import Darwin
import Foundation
import SQLite3

public enum SearchDatabaseError: LocalizedError, Sendable {
    case open(String)
    case execute(String)
    case prepare(String)

    public var errorDescription: String? {
        switch self {
        case .open(let message): "Unable to open search index: \(message)"
        case .execute(let message): "Search index operation failed: \(message)"
        case .prepare(let message): "Unable to prepare search query: \(message)"
        }
    }
}

public actor SearchDatabase {
    private static let schemaVersion = "2"
    private nonisolated(unsafe) var connection: OpaquePointer?
    private nonisolated let databaseURL: URL

    public init(url: URL) throws {
        databaseURL = url
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        connection = try Self.openWriter(at: url)
    }

    deinit { sqlite3_close(connection) }

    public static func defaultURL() throws -> URL {
        let caches = try FileManager.default.url(
            for: .cachesDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        return caches.appending(path: "MacHuntSwift/index.db")
    }

    public nonisolated func indexedItemCount() throws -> Int {
        try Self.indexedItemCount(at: databaseURL)
    }

    public nonisolated func rebuildIndexedItemCount() throws -> Int {
        try rebuildProgress().itemCount
    }

    public nonisolated func rebuildProgress() throws -> IndexBuildProgress {
        let stagingURL = Self.stagingURL(for: databaseURL)
        let url = FileManager.default.fileExists(atPath: stagingURL.path) ? stagingURL : databaseURL
        let reader = try Self.openReader(at: url)
        defer { sqlite3_close(reader) }
        let count = try Self.scalarInt(reader, sql: "SELECT COUNT(*) FROM files")
        let rawPhase = try Self.metadataValue("index_build_phase", in: reader)
            ?? IndexBuildPhase.scanning.rawValue
        return IndexBuildProgress(
            phase: IndexBuildPhase(rawValue: rawPhase) ?? .scanning,
            itemCount: count
        )
    }

    public nonisolated func indexIsComplete() throws -> Bool {
        let reader = try Self.openReader(at: databaseURL)
        defer { sqlite3_close(reader) }
        return try Self.metadataValue("index_build_complete", in: reader) == "1"
    }

    public nonisolated func indexMatches(configuration: IndexConfiguration) throws -> Bool {
        let reader = try Self.openReader(at: databaseURL)
        defer { sqlite3_close(reader) }
        return try Self.metadataValue("index_configuration", in: reader) == configuration.signature
    }

    public nonisolated func pathSuggestions(matching prefix: String, limit: Int = 30) throws -> [String] {
        let reader = try Self.openReader(at: databaseURL)
        defer { sqlite3_close(reader) }
        let trimmed = prefix.trimmingCharacters(in: .whitespacesAndNewlines)
        let canonicalPrefix = trimmed.isEmpty ? "" : Self.canonicalPath(trimmed)
        let statement = try Self.prepare(reader, sql: """
            SELECT path FROM directories
            WHERE path LIKE ? ESCAPE '\\'
            ORDER BY length(path), path COLLATE NOCASE
            LIMIT ?
            """)
        defer { sqlite3_finalize(statement) }
        Self.bind(canonicalPrefix.isEmpty ? "%" : "\(Self.escapeLike(canonicalPrefix))%", to: 1, in: statement)
        sqlite3_bind_int(statement, 2, Int32(max(1, limit)))
        var paths: [String] = []
        while sqlite3_step(statement) == SQLITE_ROW { paths.append(Self.text(statement, column: 0)) }
        return paths
    }

    public nonisolated func extensionCounts() throws -> [String: Int] {
        let reader = try Self.openReader(at: databaseURL)
        defer { sqlite3_close(reader) }
        let statement = try Self.prepare(reader, sql: """
            SELECT extension, COUNT(*) FROM files
            WHERE is_directory = 0 AND extension != ''
            GROUP BY extension
            """)
        defer { sqlite3_finalize(statement) }
        var counts: [String: Int] = [:]
        while sqlite3_step(statement) == SQLITE_ROW {
            counts[Self.text(statement, column: 0)] = Int(sqlite3_column_int64(statement, 1))
        }
        return counts
    }

    public nonisolated func lastEventID() throws -> UInt64? {
        let reader = try Self.openReader(at: databaseURL)
        defer { sqlite3_close(reader) }
        guard try Self.metadataValue("event_cursor_version", in: reader) == "1",
              let value = try Self.metadataValue("last_event_id", in: reader) else { return nil }
        return UInt64(value)
    }

    public func saveLastEventID(_ eventID: UInt64) throws {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        try Self.upsertMetadata(key: "last_event_id", value: String(eventID), connection: connection)
        try Self.upsertMetadata(key: "event_cursor_version", value: "1", connection: connection)
    }

    public func clearIndex() throws {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        try Self.resetSchema(connection)
        try Self.execute(connection, "VACUUM")
    }

    public func removeIndexedTree(at path: String) throws -> Int {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        try Self.execute(connection, "BEGIN IMMEDIATE")
        do {
            let removed = try Self.remove(path: path, from: connection)
            try Self.execute(connection, "COMMIT")
            return removed
        } catch {
            try? Self.execute(connection, "ROLLBACK")
            throw error
        }
    }

    public func optimizeForSearching() throws {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        try Self.createSearchStructures(connection)
        try Self.execute(connection, "PRAGMA optimize")
    }

    public func rebuildIndex(at rootURL: URL) async throws -> IndexSummary {
        try await rebuildIndex(at: [rootURL])
    }

    public func rebuildIndex(configuration: IndexConfiguration) async throws -> IndexSummary {
        try await rebuildIndexAtomically(
            at: configuration.roots.map { URL(fileURLWithPath: $0, isDirectory: true) },
            configuration: configuration
        )
    }

    public func rebuildIndex(at rootURLs: [URL]) async throws -> IndexSummary {
        let configuration = IndexConfiguration(
            roots: rootURLs.map(\.path),
            excludedDirectoryPatterns: [],
            excludedFilePatterns: []
        )
        return try await rebuildIndexAtomically(at: rootURLs, configuration: configuration)
    }

    public func applyFileSystemChanges(paths: [String], configuration: IndexConfiguration) throws -> Int {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        try Self.execute(connection, "BEGIN IMMEDIATE")
        do {
            var changed = 0
            for path in Self.uniquePaths(paths) {
                if FileManager.default.fileExists(atPath: path) {
                    changed += try indexChangedItem(
                        at: URL(fileURLWithPath: path),
                        configuration: configuration,
                        recursive: false,
                        connection: connection
                    )
                } else {
                    changed += try Self.remove(path: path, from: connection)
                }
            }
            try Self.execute(connection, "COMMIT")
            return changed
        } catch {
            try? Self.execute(connection, "ROLLBACK")
            throw error
        }
    }

    public func indexTree(at path: String, configuration: IndexConfiguration) throws -> Int {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        try Self.execute(connection, "BEGIN IMMEDIATE")
        do {
            let changed = try indexChangedItem(
                at: URL(fileURLWithPath: path),
                configuration: configuration,
                recursive: true,
                connection: connection
            )
            try Self.execute(connection, "COMMIT")
            return changed
        } catch {
            try? Self.execute(connection, "ROLLBACK")
            throw error
        }
    }

    private func rebuildIndexAtomically(
        at rootURLs: [URL],
        configuration: IndexConfiguration
    ) async throws -> IndexSummary {
        let stagingURL = Self.stagingURL(for: databaseURL)
        try Self.removeDatabaseFiles(at: stagingURL)
        let stagingDatabase = try SearchDatabase(url: stagingURL)
        do {
            let summary = try await stagingDatabase.rebuildIndexInPlace(
                at: rootURLs,
                configuration: configuration
            )
            try await stagingDatabase.closeDatabase()
            try replaceDatabase(with: stagingURL)
            return summary
        } catch {
            try? await stagingDatabase.closeDatabase()
            try? Self.removeDatabaseFiles(at: stagingURL)
            throw error
        }
    }

    private func rebuildIndexInPlace(
        at rootURLs: [URL],
        configuration: IndexConfiguration
    ) async throws -> IndexSummary {
        guard let connection else { throw SearchDatabaseError.open("database is closed") }
        let clock = ContinuousClock()
        let started = clock.now
        try Self.execute(connection, "PRAGMA synchronous=OFF")
        try Self.execute(connection, "PRAGMA wal_autocheckpoint=4096")
        try Self.resetSchema(connection)
        try Self.setBuildPhase(.scanning, connection: connection)
        try Self.execute(connection, "BEGIN IMMEDIATE")

        let insertDirectory = try Self.prepare(connection, sql: "INSERT OR IGNORE INTO directories(path) VALUES (?)")
        let selectDirectory = try Self.prepare(connection, sql: "SELECT id FROM directories WHERE path = ?")
        let insertFile = try Self.prepare(connection, sql: """
            INSERT INTO files(
                directory_id, name, extension, is_directory, size_bytes,
                modified_ms, added_ms, cloud_status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """)
        let insertTag = try Self.prepare(connection, sql: "INSERT OR IGNORE INTO tags(name) VALUES (?)")
        let selectTag = try Self.prepare(connection, sql: "SELECT id FROM tags WHERE name = ?")
        let insertFileTag = try Self.prepare(connection, sql: "INSERT OR IGNORE INTO file_tags(file_id, tag_id) VALUES (?, ?)")
        defer {
            sqlite3_finalize(insertDirectory)
            sqlite3_finalize(selectDirectory)
            sqlite3_finalize(insertFile)
            sqlite3_finalize(insertTag)
            sqlite3_finalize(selectTag)
            sqlite3_finalize(insertFileTag)
        }

        var directoryIDs: [String: Int64] = [:]
        var tagIDs: [String: Int64] = [:]
        let keys = Self.indexedResourceKeys
        let traversalKeys = Self.traversalResourceKeys
        let options: FileManager.DirectoryEnumerationOptions = [.skipsPackageDescendants]
        let batchSize = 1_024
        var count = 0

        do {
            for rootURL in rootURLs {
                guard let enumerator = FileManager.default.enumerator(
                    at: rootURL,
                    includingPropertiesForKeys: Array(traversalKeys),
                    options: options,
                    errorHandler: { _, _ in true }
                ) else { continue }

                while true {
                    if Task.isCancelled { throw CancellationError() }
                    let batch = Self.nextAcceptedBatch(
                        from: enumerator,
                        limit: batchSize,
                        traversalKeys: traversalKeys,
                        configuration: configuration
                    )
                    guard !batch.isEmpty else { break }
                    let resources = await Self.loadResources(for: batch, keys: keys)
                    try Self.writeScannedResources(
                        resources,
                        configuration: configuration,
                        connection: connection,
                        insertDirectory: insertDirectory,
                        selectDirectory: selectDirectory,
                        insertFile: insertFile,
                        insertTag: insertTag,
                        selectTag: selectTag,
                        insertFileTag: insertFileTag,
                        directoryIDs: &directoryIDs,
                        tagIDs: &tagIDs,
                        count: &count
                    )
                }
            }
            try Self.execute(connection, "COMMIT")

            try Self.setBuildPhase(.buildingSearchIndex, connection: connection)
            try Self.execute(connection, "INSERT INTO files_fts(files_fts) VALUES ('rebuild')")

            try Self.setBuildPhase(.optimizing, connection: connection)
            try Self.createSearchStructures(connection)
            try Self.upsertMetadata(key: "index_configuration", value: configuration.signature, connection: connection)
            try Self.upsertMetadata(key: "index_build_complete", value: "1", connection: connection)
            try Self.setBuildPhase(.complete, connection: connection)
            try Self.execute(connection, "PRAGMA optimize")
            sqlite3_wal_checkpoint_v2(connection, nil, SQLITE_CHECKPOINT_TRUNCATE, nil, nil)
            try Self.execute(connection, "PRAGMA synchronous=NORMAL")
            return IndexSummary(itemCount: count, elapsed: started.duration(to: clock.now))
        } catch {
            try? Self.execute(connection, "ROLLBACK")
            throw error
        }
    }

    private func closeDatabase() throws {
        guard let connection else { return }
        sqlite3_wal_checkpoint_v2(connection, nil, SQLITE_CHECKPOINT_TRUNCATE, nil, nil)
        guard sqlite3_close_v2(connection) == SQLITE_OK else {
            throw SearchDatabaseError.execute(Self.message(connection))
        }
        self.connection = nil
    }

    private func replaceDatabase(with stagingURL: URL) throws {
        try closeDatabase()
        try Self.removeSidecars(at: databaseURL)
        guard Darwin.rename(stagingURL.path, databaseURL.path) == 0 else {
            let replacementError = SearchDatabaseError.execute(
                "Unable to replace search index: \(String(cString: strerror(errno)))"
            )
            connection = try? Self.openWriter(at: databaseURL)
            throw replacementError
        }
        try Self.removeSidecars(at: stagingURL)
        connection = try Self.openWriter(at: databaseURL)
    }

    public nonisolated func search(_ request: SearchRequest) throws -> [SearchResult] {
        let connection = try Self.openReader(at: databaseURL)
        defer { sqlite3_close(connection) }
        var bindings: [SQLBinding] = []
        var clauses: [String] = []
        let trimmedQuery = request.query.trimmingCharacters(in: .whitespacesAndNewlines)
        let canUseTrigram = request.mode == .substring
            && trimmedQuery.count >= 3
            && trimmedQuery.unicodeScalars.allSatisfy(\.isASCII)
        let fullPath = Self.fullPathSQL(fileAlias: "f", directoryAlias: "d")
        let tagsSQL = """
            coalesce((
                SELECT group_concat(tag_name, char(31)) FROM (
                    SELECT t.name AS tag_name
                    FROM file_tags ft JOIN tags t ON t.id = ft.tag_id
                    WHERE ft.file_id = f.id ORDER BY t.name COLLATE NOCASE
                )
            ), '')
            """
        var sql = """
            SELECT f.name, \(fullPath), d.path, f.is_directory, f.size_bytes, f.modified_ms,
                   f.added_ms, f.cloud_status, \(tagsSQL)
            FROM files f
            JOIN directories d ON d.id = f.directory_id
            """

        if canUseTrigram {
            sql += " JOIN files_fts ON files_fts.rowid = f.id"
            clauses.append("files_fts MATCH ?")
            bindings.append(.text(Self.quotedFTS(trimmedQuery)))
            if request.caseSensitive {
                clauses.append("instr(f.name, ?) > 0")
                bindings.append(.text(trimmedQuery))
            }
        } else if request.mode == .substring, !trimmedQuery.isEmpty {
            clauses.append(request.caseSensitive
                ? "instr(f.name, ?) > 0"
                : "lower(f.name) LIKE lower(?) ESCAPE '\\'")
            bindings.append(.text(request.caseSensitive ? trimmedQuery : "%\(Self.escapeLike(trimmedQuery))%"))
        } else if request.mode == .fuzzy, !trimmedQuery.isEmpty {
            for token in trimmedQuery.split(whereSeparator: \.isWhitespace) {
                clauses.append("lower(f.name) LIKE lower(?) ESCAPE '\\'")
                let characters = String(token).map { Self.escapeLike(String($0)) }.joined(separator: "%")
                bindings.append(.text("%\(characters)%"))
            }
        }

        if let prefix = request.pathPrefix?.trimmingCharacters(in: .whitespacesAndNewlines), !prefix.isEmpty {
            let standardized = Self.canonicalPath(prefix)
            clauses.append("(d.path = ? OR d.path LIKE ? ESCAPE '\\')")
            bindings.append(.text(standardized))
            bindings.append(.text(standardized == "/" ? "/%" : "\(Self.escapeLike(standardized))/%"))
        }
        if request.category == .files { clauses.append("f.is_directory = 0") }
        if request.category == .folders { clauses.append("f.is_directory = 1") }
        let categoryExtensions = request.category.effectiveExtensions(override: request.categoryExtensions)
        if request.category.supportsEditableExtensions, categoryExtensions?.isEmpty != false {
            clauses.append("0")
        } else if let extensions = categoryExtensions {
            clauses.append("f.is_directory = 0 AND f.extension IN (\(Array(repeating: "?", count: extensions.count).joined(separator: ",")))")
            bindings.append(contentsOf: extensions.sorted().map(SQLBinding.text))
        }
        if let extensions = request.applicationExtensions ?? request.application.extensions {
            clauses.append("f.is_directory = 0 AND f.extension IN (\(Array(repeating: "?", count: extensions.count).joined(separator: ",")))")
            bindings.append(contentsOf: extensions.sorted().map(SQLBinding.text))
        }
        if let value = request.minimumSize { clauses.append("f.size_bytes >= ?"); bindings.append(.integer(value)) }
        if let value = request.maximumSize { clauses.append("f.size_bytes <= ?"); bindings.append(.integer(value)) }
        if let date = request.modifiedAfter {
            clauses.append("f.modified_ms >= ?")
            bindings.append(.integer(Int64(date.timeIntervalSince1970 * 1_000)))
        }
        if let date = request.modifiedBefore {
            clauses.append("f.modified_ms <= ?")
            bindings.append(.integer(Int64(date.timeIntervalSince1970 * 1_000)))
        }
        if !clauses.isEmpty { sql += " WHERE " + clauses.joined(separator: " AND ") }

        let sortColumn = switch request.sort {
        case .name: "f.name COLLATE NOCASE"
        case .path: "d.path COLLATE NOCASE, f.name COLLATE NOCASE"
        case .type: "f.extension, f.name COLLATE NOCASE"
        case .size: "coalesce(f.size_bytes, 0)"
        case .modified: "coalesce(f.modified_ms, 0)"
        case .added: "coalesce(f.added_ms, 0)"
        case .cloudStatus: "coalesce(f.cloud_status, 0)"
        case .tags: tagsSQL + " COLLATE NOCASE"
        }
        sql += " ORDER BY \(sortColumn) \(request.ascending ? "ASC" : "DESC")"
        let requiresPostFiltering = request.mode == .pattern || request.mode == .fuzzy
        if !requiresPostFiltering {
            sql += " LIMIT ?"
            bindings.append(.integer(Int64(request.limit)))
        }

        let statement = try Self.prepare(connection, sql: sql)
        defer { sqlite3_finalize(statement) }
        for (offset, binding) in bindings.enumerated() {
            Self.bind(binding, to: Int32(offset + 1), in: statement)
        }

        var results: [SearchResult] = []
        while true {
            if Task.isCancelled { throw CancellationError() }
            guard sqlite3_step(statement) == SQLITE_ROW else { break }
            let item = SearchResult(
                name: Self.text(statement, column: 0),
                path: Self.text(statement, column: 1),
                parent: Self.text(statement, column: 2),
                isDirectory: sqlite3_column_int(statement, 3) != 0,
                sizeBytes: Self.optionalInt64(statement, column: 4),
                modifiedAt: Self.date(statement, column: 5),
                addedAt: Self.date(statement, column: 6),
                cloudStatus: Self.cloudStatus(code: Self.optionalInt64(statement, column: 7)),
                tags: Self.decodeTags(Self.text(statement, column: 8))
            )
            guard request.category.includes(item, categoryExtensions: request.categoryExtensions) else { continue }
            if request.applicationExtensions == nil, !request.application.includes(item) { continue }
            if request.mode == .pattern, !trimmedQuery.isEmpty,
               !Self.pattern(trimmedQuery, caseSensitive: request.caseSensitive, matches: item.name) { continue }
            if request.mode == .fuzzy, !trimmedQuery.isEmpty,
               !Self.fuzzy(trimmedQuery, caseSensitive: request.caseSensitive, matches: item.name) { continue }
            results.append(item)
            if results.count >= request.limit { break }
        }
        return results
    }

    private func indexChangedItem(
        at url: URL,
        configuration: IndexConfiguration,
        recursive: Bool,
        connection: OpaquePointer
    ) throws -> Int {
        let keys = Self.indexedResourceKeys
        guard let values = try? url.resourceValues(forKeys: keys), values.isSymbolicLink != true else { return 0 }
        let isDirectory = values.isDirectory == true
        guard isDirectory || values.isRegularFile == true else { return 0 }
        guard configuration.includes(path: url.path, isDirectory: isDirectory, isHidden: values.isHidden == true) else {
            return try Self.remove(path: url.path, from: connection)
        }

        var changed = try Self.upsert(url: url, values: values, connection: connection)
        guard recursive, isDirectory, let enumerator = FileManager.default.enumerator(
            at: url,
            includingPropertiesForKeys: Array(keys),
            options: [.skipsPackageDescendants],
            errorHandler: { _, _ in true }
        ) else { return changed }

        for case let child as URL in enumerator {
            if Task.isCancelled { throw CancellationError() }
            let outcome = try autoreleasepool {
                try Self.indexIncrementalItem(
                    child,
                    resourceKeys: keys,
                    configuration: configuration,
                    connection: connection
                )
            }
            if outcome == .skipDescendants { enumerator.skipDescendants() }
            if outcome == .indexed { changed += 1 }
        }
        return changed
    }

    private static func upsert(url: URL, values: URLResourceValues, connection: OpaquePointer) throws -> Int {
        let parent = url.deletingLastPathComponent().path
        let parentDirectoryID = try directoryID(for: parent, connection: connection)
        if values.isDirectory == true { _ = try directoryID(for: url.path, connection: connection) }
        let statement = try prepare(connection, sql: """
            INSERT INTO files(
                directory_id, name, extension, is_directory, size_bytes,
                modified_ms, added_ms, cloud_status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(directory_id, name) DO UPDATE SET
                extension=excluded.extension,
                is_directory=excluded.is_directory,
                size_bytes=excluded.size_bytes,
                modified_ms=excluded.modified_ms,
                added_ms=excluded.added_ms,
                cloud_status=excluded.cloud_status
            RETURNING id
            """)
        defer { sqlite3_finalize(statement) }
        let fileID = try upsertIndexedItem(
            url: url,
            values: values,
            directoryID: parentDirectoryID,
            statement: statement,
            connection: connection
        )
        let deleteTags = try prepare(connection, sql: "DELETE FROM file_tags WHERE file_id = ?")
        sqlite3_bind_int64(deleteTags, 1, fileID)
        guard sqlite3_step(deleteTags) == SQLITE_DONE else {
            sqlite3_finalize(deleteTags)
            throw SearchDatabaseError.execute(message(connection))
        }
        sqlite3_finalize(deleteTags)
        try insertTags(values.tagNames ?? [], fileID: fileID, connection: connection)
        return 1
    }

    private static func remove(path: String, from connection: OpaquePointer) throws -> Int {
        let url = URL(fileURLWithPath: path).standardizedFileURL
        let target = url.path
        let parent = url.deletingLastPathComponent().path
        let name = url.lastPathComponent
        let descendantPattern = target == "/" ? "/%" : "\(escapeLike(target))/%"
        let countStatement = try prepare(connection, sql: """
            SELECT COUNT(*)
            FROM files f JOIN directories d ON d.id = f.directory_id
            WHERE (d.path = ? AND f.name = ?) OR d.path = ? OR d.path LIKE ? ESCAPE '\\'
            """)
        bind(parent, to: 1, in: countStatement)
        bind(name, to: 2, in: countStatement)
        bind(target, to: 3, in: countStatement)
        bind(descendantPattern, to: 4, in: countStatement)
        guard sqlite3_step(countStatement) == SQLITE_ROW else {
            sqlite3_finalize(countStatement)
            throw SearchDatabaseError.execute(message(connection))
        }
        let count = Int(sqlite3_column_int64(countStatement, 0))
        sqlite3_finalize(countStatement)

        let deleteFiles = try prepare(connection, sql: """
            DELETE FROM files WHERE id IN (
                SELECT f.id
                FROM files f JOIN directories d ON d.id = f.directory_id
                WHERE (d.path = ? AND f.name = ?) OR d.path = ? OR d.path LIKE ? ESCAPE '\\'
            )
            """)
        bind(parent, to: 1, in: deleteFiles)
        bind(name, to: 2, in: deleteFiles)
        bind(target, to: 3, in: deleteFiles)
        bind(descendantPattern, to: 4, in: deleteFiles)
        guard sqlite3_step(deleteFiles) == SQLITE_DONE else {
            sqlite3_finalize(deleteFiles)
            throw SearchDatabaseError.execute(message(connection))
        }
        sqlite3_finalize(deleteFiles)

        let deleteDirectories = try prepare(connection, sql: "DELETE FROM directories WHERE path = ? OR path LIKE ? ESCAPE '\\'")
        bind(target, to: 1, in: deleteDirectories)
        bind(descendantPattern, to: 2, in: deleteDirectories)
        guard sqlite3_step(deleteDirectories) == SQLITE_DONE else {
            sqlite3_finalize(deleteDirectories)
            throw SearchDatabaseError.execute(message(connection))
        }
        sqlite3_finalize(deleteDirectories)
        try execute(connection, "DELETE FROM tags WHERE NOT EXISTS (SELECT 1 FROM file_tags WHERE tag_id = tags.id)")
        return count
    }

    private static let indexedResourceKeys: Set<URLResourceKey> = [
        .isDirectoryKey, .isRegularFileKey, .fileSizeKey, .contentModificationDateKey,
        .addedToDirectoryDateKey, .isSymbolicLinkKey, .isHiddenKey, .tagNamesKey,
        .isUbiquitousItemKey,
    ]

    private static let traversalResourceKeys: Set<URLResourceKey> = [
        .isDirectoryKey, .isRegularFileKey, .isSymbolicLinkKey, .isHiddenKey,
    ]

    private static let cloudResourceKeys: Set<URLResourceKey> = [
        .isUbiquitousItemKey, .ubiquitousItemHasUnresolvedConflictsKey,
        .ubiquitousItemIsDownloadingKey, .ubiquitousItemIsUploadedKey,
        .ubiquitousItemIsUploadingKey, .ubiquitousItemDownloadingStatusKey,
    ]

    private struct ScannedResource: @unchecked Sendable {
        let url: URL
        let values: URLResourceValues
    }

    private static func nextAcceptedBatch(
        from enumerator: FileManager.DirectoryEnumerator,
        limit: Int,
        traversalKeys: Set<URLResourceKey>,
        configuration: IndexConfiguration
    ) -> [URL] {
        var urls: [URL] = []
        urls.reserveCapacity(limit)
        while urls.count < limit, let fileURL = enumerator.nextObject() as? URL {
            guard let values = try? fileURL.resourceValues(forKeys: traversalKeys),
                  values.isSymbolicLink != true else { continue }
            let isDirectory = values.isDirectory == true
            guard isDirectory || values.isRegularFile == true else { continue }
            guard configuration.includes(
                path: fileURL.path,
                isDirectory: isDirectory,
                isHidden: values.isHidden == true
            ) else {
                if isDirectory { enumerator.skipDescendants() }
                continue
            }
            urls.append(fileURL)
        }
        return urls
    }

    private static func loadResources(
        for urls: [URL],
        keys: Set<URLResourceKey>
    ) async -> [ScannedResource] {
        let availableWorkers = max(2, ProcessInfo.processInfo.activeProcessorCount - 1)
        let workerCount = min(8, availableWorkers, urls.count)
        guard workerCount > 0 else { return [] }

        return await withTaskGroup(of: (Int, ScannedResource?).self) { group in
            var nextIndex = 0
            for _ in 0..<workerCount {
                let index = nextIndex
                nextIndex += 1
                group.addTask {
                    let resource = autoreleasepool {
                        (try? urls[index].resourceValues(forKeys: keys)).map {
                            ScannedResource(url: urls[index], values: $0)
                        }
                    }
                    return (index, resource)
                }
            }

            var ordered = Array<ScannedResource?>(repeating: nil, count: urls.count)
            for await (index, resource) in group {
                ordered[index] = resource
                if nextIndex < urls.count {
                    let newIndex = nextIndex
                    nextIndex += 1
                    group.addTask {
                        let resource = autoreleasepool {
                            (try? urls[newIndex].resourceValues(forKeys: keys)).map {
                                ScannedResource(url: urls[newIndex], values: $0)
                            }
                        }
                        return (newIndex, resource)
                    }
                }
            }
            return ordered.compactMap { $0 }
        }
    }

    private static func writeScannedResources(
        _ resources: [ScannedResource],
        configuration: IndexConfiguration,
        connection: OpaquePointer,
        insertDirectory: OpaquePointer,
        selectDirectory: OpaquePointer,
        insertFile: OpaquePointer,
        insertTag: OpaquePointer,
        selectTag: OpaquePointer,
        insertFileTag: OpaquePointer,
        directoryIDs: inout [String: Int64],
        tagIDs: inout [String: Int64],
        count: inout Int
    ) throws {
        for resource in resources {
            let outcome = try autoreleasepool {
                try indexScannedItem(
                    resource.url,
                    values: resource.values,
                    configuration: configuration,
                    connection: connection,
                    insertDirectory: insertDirectory,
                    selectDirectory: selectDirectory,
                    insertFile: insertFile,
                    insertTag: insertTag,
                    selectTag: selectTag,
                    insertFileTag: insertFileTag,
                    directoryIDs: &directoryIDs,
                    tagIDs: &tagIDs
                )
            }
            guard outcome == .indexed else { continue }
            count += 1
            if count.isMultiple(of: 25_000) {
                try execute(connection, "COMMIT")
                try execute(connection, "BEGIN IMMEDIATE")
                directoryIDs.removeAll(keepingCapacity: false)
            }
        }
    }

    private enum ScannedItemOutcome { case indexed, skipped, skipDescendants }

    private static func indexScannedItem(
        _ fileURL: URL,
        values: URLResourceValues,
        configuration: IndexConfiguration,
        connection: OpaquePointer,
        insertDirectory: OpaquePointer,
        selectDirectory: OpaquePointer,
        insertFile: OpaquePointer,
        insertTag: OpaquePointer,
        selectTag: OpaquePointer,
        insertFileTag: OpaquePointer,
        directoryIDs: inout [String: Int64],
        tagIDs: inout [String: Int64]
    ) throws -> ScannedItemOutcome {
        guard values.isSymbolicLink != true else { return .skipped }
        let isDirectory = values.isDirectory == true
        guard isDirectory || values.isRegularFile == true else { return .skipped }
        guard configuration.includes(
            path: fileURL.path,
            isDirectory: isDirectory,
            isHidden: values.isHidden == true
        ) else { return isDirectory ? .skipDescendants : .skipped }

        let parent = fileURL.deletingLastPathComponent().path
        let parentDirectoryID = try directoryID(
            for: parent,
            connection: connection,
            insert: insertDirectory,
            select: selectDirectory,
            cache: &directoryIDs
        )
        if isDirectory {
            _ = try directoryID(
                for: fileURL.path,
                connection: connection,
                insert: insertDirectory,
                select: selectDirectory,
                cache: &directoryIDs
            )
        }
        let fileID = try insertScannedItem(
            url: fileURL,
            values: values,
            directoryID: parentDirectoryID,
            statement: insertFile,
            connection: connection
        )
        try insertTags(
            values.tagNames ?? [],
            fileID: fileID,
            connection: connection,
            insertTag: insertTag,
            selectTag: selectTag,
            insertFileTag: insertFileTag,
            cache: &tagIDs
        )
        return .indexed
    }

    private static func indexIncrementalItem(
        _ fileURL: URL,
        resourceKeys: Set<URLResourceKey>,
        configuration: IndexConfiguration,
        connection: OpaquePointer
    ) throws -> ScannedItemOutcome {
        guard let values = try? fileURL.resourceValues(forKeys: resourceKeys),
              values.isSymbolicLink != true else { return .skipped }
        let isDirectory = values.isDirectory == true
        guard isDirectory || values.isRegularFile == true else { return .skipped }
        guard configuration.includes(
            path: fileURL.path,
            isDirectory: isDirectory,
            isHidden: values.isHidden == true
        ) else { return isDirectory ? .skipDescendants : .skipped }
        _ = try upsert(url: fileURL, values: values, connection: connection)
        return .indexed
    }

    private static func insertScannedItem(
        url: URL,
        values: URLResourceValues,
        directoryID: Int64,
        statement: OpaquePointer,
        connection: OpaquePointer
    ) throws -> Int64 {
        try bindIndexedItem(
            url: url,
            values: values,
            directoryID: directoryID,
            statement: statement
        )
        guard sqlite3_step(statement) == SQLITE_DONE else {
            throw SearchDatabaseError.execute(message(connection))
        }
        return sqlite3_last_insert_rowid(connection)
    }

    private static func upsertIndexedItem(
        url: URL,
        values: URLResourceValues,
        directoryID: Int64,
        statement: OpaquePointer,
        connection: OpaquePointer
    ) throws -> Int64 {
        try bindIndexedItem(
            url: url,
            values: values,
            directoryID: directoryID,
            statement: statement
        )
        guard sqlite3_step(statement) == SQLITE_ROW else {
            throw SearchDatabaseError.execute(message(connection))
        }
        let id = sqlite3_column_int64(statement, 0)
        guard sqlite3_step(statement) == SQLITE_DONE else {
            throw SearchDatabaseError.execute(message(connection))
        }
        return id
    }

    private static func bindIndexedItem(
        url: URL,
        values: URLResourceValues,
        directoryID: Int64,
        statement: OpaquePointer
    ) throws {
        let isDirectory = values.isDirectory == true
        sqlite3_reset(statement)
        sqlite3_clear_bindings(statement)
        sqlite3_bind_int64(statement, 1, directoryID)
        bind(url.lastPathComponent, to: 2, in: statement)
        bind(isDirectory ? "" : url.pathExtension.lowercased(), to: 3, in: statement)
        sqlite3_bind_int(statement, 4, isDirectory ? 1 : 0)
        if !isDirectory, let size = values.fileSize { sqlite3_bind_int64(statement, 5, Int64(size)) }
        else { sqlite3_bind_null(statement, 5) }
        bind(values.contentModificationDate, to: 6, in: statement)
        bind(values.addedToDirectoryDate, to: 7, in: statement)
        if let cloudCode = resolvedCloudCode(for: url, baseValues: values) {
            sqlite3_bind_int(statement, 8, cloudCode)
        }
        else { sqlite3_bind_null(statement, 8) }
    }

    private static func directoryID(for path: String, connection: OpaquePointer) throws -> Int64 {
        var cache: [String: Int64] = [:]
        let insert = try prepare(connection, sql: "INSERT OR IGNORE INTO directories(path) VALUES (?)")
        let select = try prepare(connection, sql: "SELECT id FROM directories WHERE path = ?")
        defer { sqlite3_finalize(insert); sqlite3_finalize(select) }
        return try directoryID(
            for: path,
            connection: connection,
            insert: insert,
            select: select,
            cache: &cache
        )
    }

    private static func directoryID(
        for path: String,
        connection: OpaquePointer,
        insert: OpaquePointer,
        select: OpaquePointer,
        cache: inout [String: Int64]
    ) throws -> Int64 {
        if let cached = cache[path] { return cached }
        sqlite3_reset(insert)
        sqlite3_clear_bindings(insert)
        bind(path, to: 1, in: insert)
        guard sqlite3_step(insert) == SQLITE_DONE else { throw SearchDatabaseError.execute(message(connection)) }
        if sqlite3_changes(connection) > 0 {
            let id = sqlite3_last_insert_rowid(connection)
            cache[path] = id
            return id
        }
        sqlite3_reset(select)
        sqlite3_clear_bindings(select)
        bind(path, to: 1, in: select)
        guard sqlite3_step(select) == SQLITE_ROW else { throw SearchDatabaseError.execute(message(connection)) }
        let id = sqlite3_column_int64(select, 0)
        guard sqlite3_step(select) == SQLITE_DONE else { throw SearchDatabaseError.execute(message(connection)) }
        cache[path] = id
        return id
    }

    private static func insertTags(_ tags: [String], fileID: Int64, connection: OpaquePointer) throws {
        var cache: [String: Int64] = [:]
        let insertTag = try prepare(connection, sql: "INSERT OR IGNORE INTO tags(name) VALUES (?)")
        let selectTag = try prepare(connection, sql: "SELECT id FROM tags WHERE name = ?")
        let insertFileTag = try prepare(connection, sql: "INSERT OR IGNORE INTO file_tags(file_id, tag_id) VALUES (?, ?)")
        defer { sqlite3_finalize(insertTag); sqlite3_finalize(selectTag); sqlite3_finalize(insertFileTag) }
        try insertTags(
            tags,
            fileID: fileID,
            connection: connection,
            insertTag: insertTag,
            selectTag: selectTag,
            insertFileTag: insertFileTag,
            cache: &cache
        )
    }

    private static func insertTags(
        _ tags: [String],
        fileID: Int64,
        connection: OpaquePointer,
        insertTag: OpaquePointer,
        selectTag: OpaquePointer,
        insertFileTag: OpaquePointer,
        cache: inout [String: Int64]
    ) throws {
        for name in Set(tags).sorted() where !name.isEmpty {
            let tagID: Int64
            if let cached = cache[name] {
                tagID = cached
            } else {
                sqlite3_reset(insertTag)
                sqlite3_clear_bindings(insertTag)
                bind(name, to: 1, in: insertTag)
                guard sqlite3_step(insertTag) == SQLITE_DONE else {
                    throw SearchDatabaseError.execute(message(connection))
                }
                sqlite3_reset(selectTag)
                sqlite3_clear_bindings(selectTag)
                bind(name, to: 1, in: selectTag)
                guard sqlite3_step(selectTag) == SQLITE_ROW else {
                    throw SearchDatabaseError.execute(message(connection))
                }
                tagID = sqlite3_column_int64(selectTag, 0)
                guard sqlite3_step(selectTag) == SQLITE_DONE else {
                    throw SearchDatabaseError.execute(message(connection))
                }
                cache[name] = tagID
            }
            sqlite3_reset(insertFileTag)
            sqlite3_clear_bindings(insertFileTag)
            sqlite3_bind_int64(insertFileTag, 1, fileID)
            sqlite3_bind_int64(insertFileTag, 2, tagID)
            guard sqlite3_step(insertFileTag) == SQLITE_DONE else {
                throw SearchDatabaseError.execute(message(connection))
            }
        }
    }

    private static func createSchema(_ database: OpaquePointer) throws {
        try execute(database, """
            CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            """)
        let version = try metadataValue("schema_version", in: database)
        if version != schemaVersion { try resetSchema(database) }
    }

    private static func resetSchema(_ database: OpaquePointer) throws {
        try execute(database, """
            DROP TRIGGER IF EXISTS files_ai;
            DROP TRIGGER IF EXISTS files_ad;
            DROP TRIGGER IF EXISTS files_au;
            DROP TABLE IF EXISTS files_fts;
            DROP TABLE IF EXISTS file_tags;
            DROP TABLE IF EXISTS tags;
            DROP TABLE IF EXISTS files;
            DROP TABLE IF EXISTS directories;
            DROP TABLE IF EXISTS metadata;

            CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE directories(
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE
            );
            CREATE TABLE files(
                id INTEGER PRIMARY KEY,
                directory_id INTEGER NOT NULL REFERENCES directories(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                extension TEXT NOT NULL DEFAULT '',
                is_directory INTEGER NOT NULL,
                size_bytes INTEGER,
                modified_ms INTEGER,
                added_ms INTEGER,
                cloud_status INTEGER,
                UNIQUE(directory_id, name)
            );
            CREATE TABLE tags(
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE file_tags(
                file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY(file_id, tag_id)
            ) WITHOUT ROWID;
            CREATE VIRTUAL TABLE files_fts USING fts5(
                name,
                content='files',
                content_rowid='id',
                tokenize='trigram'
            );
            """)
        try upsertMetadata(key: "schema_version", value: schemaVersion, connection: database)
        try upsertMetadata(key: "index_build_complete", value: "0", connection: database)
        try setBuildPhase(.scanning, connection: database)
    }

    private static func createSearchStructures(_ database: OpaquePointer) throws {
        try execute(database, """
            CREATE INDEX IF NOT EXISTS files_name_nocase_idx ON files(name COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS files_extension_idx ON files(extension, name COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS files_modified_idx ON files(modified_ms);
            CREATE INDEX IF NOT EXISTS files_added_idx ON files(added_ms);

            CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
                INSERT INTO files_fts(rowid, name) VALUES (new.id, new.name);
            END;
            CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
                INSERT INTO files_fts(files_fts, rowid, name) VALUES ('delete', old.id, old.name);
            END;
            CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE OF name ON files BEGIN
                INSERT INTO files_fts(files_fts, rowid, name) VALUES ('delete', old.id, old.name);
                INSERT INTO files_fts(rowid, name) VALUES (new.id, new.name);
            END;
            """)
    }

    private static func setBuildPhase(_ phase: IndexBuildPhase, connection: OpaquePointer) throws {
        try upsertMetadata(key: "index_build_phase", value: phase.rawValue, connection: connection)
    }

    private static func configure(_ database: OpaquePointer) throws {
        try execute(database, "PRAGMA journal_mode=WAL")
        try execute(database, "PRAGMA synchronous=NORMAL")
        try execute(database, "PRAGMA temp_store=MEMORY")
        try execute(database, "PRAGMA mmap_size=33554432")
        try execute(database, "PRAGMA foreign_keys=ON")
        sqlite3_busy_timeout(database, 5_000)
    }

    private static func openReader(at url: URL) throws -> OpaquePointer {
        var database: OpaquePointer?
        let result = sqlite3_open_v2(url.path, &database, SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, nil)
        guard result == SQLITE_OK, let database else {
            let error = database.map { String(cString: sqlite3_errmsg($0)) } ?? "unknown SQLite error"
            sqlite3_close(database)
            throw SearchDatabaseError.open(error)
        }
        sqlite3_busy_timeout(database, 5_000)
        return database
    }

    private static func openWriter(at url: URL) throws -> OpaquePointer {
        var database: OpaquePointer?
        let result = sqlite3_open_v2(
            url.path,
            &database,
            SQLITE_OPEN_CREATE | SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX,
            nil
        )
        guard result == SQLITE_OK, let database else {
            let error = database.map { String(cString: sqlite3_errmsg($0)) } ?? "unknown SQLite error"
            sqlite3_close(database)
            throw SearchDatabaseError.open(error)
        }
        do {
            try configure(database)
            try createSchema(database)
            return database
        } catch {
            sqlite3_close(database)
            throw error
        }
    }

    private nonisolated static func stagingURL(for databaseURL: URL) -> URL {
        URL(fileURLWithPath: databaseURL.path + ".new")
    }

    private static func removeDatabaseFiles(at url: URL) throws {
        let manager = FileManager.default
        for candidate in [url, URL(fileURLWithPath: url.path + "-wal"), URL(fileURLWithPath: url.path + "-shm")] {
            if manager.fileExists(atPath: candidate.path) { try manager.removeItem(at: candidate) }
        }
    }

    private static func removeSidecars(at url: URL) throws {
        let manager = FileManager.default
        for candidate in [URL(fileURLWithPath: url.path + "-wal"), URL(fileURLWithPath: url.path + "-shm")] {
            if manager.fileExists(atPath: candidate.path) { try manager.removeItem(at: candidate) }
        }
    }

    private static func indexedItemCount(at url: URL) throws -> Int {
        let reader = try openReader(at: url)
        defer { sqlite3_close(reader) }
        return try scalarInt(reader, sql: "SELECT COUNT(*) FROM files")
    }

    private static func scalarInt(_ database: OpaquePointer, sql: String) throws -> Int {
        let statement = try prepare(database, sql: sql)
        defer { sqlite3_finalize(statement) }
        guard sqlite3_step(statement) == SQLITE_ROW else { throw SearchDatabaseError.execute(message(database)) }
        return Int(sqlite3_column_int64(statement, 0))
    }

    private static func metadataValue(_ key: String, in database: OpaquePointer) throws -> String? {
        let statement = try prepare(database, sql: "SELECT value FROM metadata WHERE key = ?")
        defer { sqlite3_finalize(statement) }
        bind(key, to: 1, in: statement)
        let result = sqlite3_step(statement)
        if result == SQLITE_DONE { return nil }
        guard result == SQLITE_ROW else { throw SearchDatabaseError.execute(message(database)) }
        return text(statement, column: 0)
    }

    private static func upsertMetadata(key: String, value: String, connection: OpaquePointer) throws {
        let statement = try prepare(connection, sql: """
            INSERT INTO metadata(key, value) VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """)
        defer { sqlite3_finalize(statement) }
        bind(key, to: 1, in: statement)
        bind(value, to: 2, in: statement)
        guard sqlite3_step(statement) == SQLITE_DONE else { throw SearchDatabaseError.execute(message(connection)) }
    }

    private static func uniquePaths(_ paths: [String]) -> [String] {
        Set(paths.map { URL(fileURLWithPath: $0).standardizedFileURL.path }).sorted()
    }

    private static func fullPathSQL(fileAlias: String, directoryAlias: String) -> String {
        "CASE WHEN \(directoryAlias).path = '/' THEN '/' || \(fileAlias).name ELSE \(directoryAlias).path || '/' || \(fileAlias).name END"
    }

    private static func cloudCode(from values: URLResourceValues) -> Int32? {
        guard values.isUbiquitousItem == true else { return nil }
        if values.ubiquitousItemHasUnresolvedConflicts == true { return 6 }
        if values.ubiquitousItemIsDownloading == true { return 2 }
        if values.ubiquitousItemIsUploading == true { return 4 }
        switch values.ubiquitousItemDownloadingStatus {
        case .notDownloaded: return 1
        case .downloaded: return 3
        case .current: return values.ubiquitousItemIsUploaded == true ? 5 : 3
        default: return values.ubiquitousItemIsUploaded == true ? 5 : 1
        }
    }

    private static func resolvedCloudCode(for url: URL, baseValues: URLResourceValues) -> Int32? {
        guard baseValues.isUbiquitousItem == true else { return nil }
        guard let values = try? url.resourceValues(forKeys: cloudResourceKeys) else { return 1 }
        return cloudCode(from: values)
    }

    private static func cloudStatus(code: Int64?) -> CloudFileStatus? {
        switch code {
        case 1: .inCloud
        case 2: .downloading
        case 3: .downloaded
        case 4: .uploading
        case 5: .synced
        case 6: .conflict
        default: nil
        }
    }

    private static func decodeTags(_ value: String) -> [String] {
        value.isEmpty ? [] : value.split(separator: "\u{1F}").map(String.init)
    }

    private static func optionalInt64(_ statement: OpaquePointer, column: Int32) -> Int64? {
        sqlite3_column_type(statement, column) == SQLITE_NULL ? nil : sqlite3_column_int64(statement, column)
    }

    private static func date(_ statement: OpaquePointer, column: Int32) -> Date? {
        optionalInt64(statement, column: column).map { Date(timeIntervalSince1970: Double($0) / 1_000) }
    }

    private enum SQLBinding { case text(String), integer(Int64) }
    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    private static func bind(_ value: SQLBinding, to index: Int32, in statement: OpaquePointer) {
        switch value {
        case .text(let text): bind(text, to: index, in: statement)
        case .integer(let integer): sqlite3_bind_int64(statement, index, integer)
        }
    }

    private static func bind(_ value: String, to index: Int32, in statement: OpaquePointer) {
        sqlite3_bind_text(statement, index, value, -1, transient)
    }

    private static func bind(_ value: Date?, to index: Int32, in statement: OpaquePointer) {
        if let value { sqlite3_bind_int64(statement, index, Int64(value.timeIntervalSince1970 * 1_000)) }
        else { sqlite3_bind_null(statement, index) }
    }

    private static func text(_ statement: OpaquePointer, column: Int32) -> String {
        guard let value = sqlite3_column_text(statement, column) else { return "" }
        return String(cString: value)
    }

    private static func execute(_ database: OpaquePointer, _ sql: String) throws {
        var error: UnsafeMutablePointer<CChar>?
        guard sqlite3_exec(database, sql, nil, nil, &error) == SQLITE_OK else {
            let value = error.map { String(cString: $0) } ?? message(database)
            sqlite3_free(error)
            throw SearchDatabaseError.execute(value)
        }
    }

    private static func prepare(_ database: OpaquePointer, sql: String) throws -> OpaquePointer {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, sql, -1, &statement, nil) == SQLITE_OK, let statement else {
            throw SearchDatabaseError.prepare(message(database))
        }
        return statement
    }

    private static func message(_ database: OpaquePointer) -> String { String(cString: sqlite3_errmsg(database)) }
    private static func quotedFTS(_ query: String) -> String { "\"\(query.replacingOccurrences(of: "\"", with: "\"\""))\"" }
    private static func escapeLike(_ value: String) -> String {
        value.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "%", with: "\\%")
            .replacingOccurrences(of: "_", with: "\\_")
    }

    private static func canonicalPath(_ path: String) -> String {
        guard let pointer = realpath(path, nil) else { return URL(fileURLWithPath: path).standardizedFileURL.path }
        defer { free(pointer) }
        return String(cString: pointer)
    }

    private static func pattern(_ pattern: String, caseSensitive: Bool, matches value: String) -> Bool {
        let expression: String
        if (try? NSRegularExpression(pattern: pattern)) != nil {
            expression = pattern
        } else {
            expression = "^" + NSRegularExpression.escapedPattern(for: pattern)
                .replacingOccurrences(of: "\\*\\*", with: ".*")
                .replacingOccurrences(of: "\\*", with: "[^/]*")
                .replacingOccurrences(of: "\\?", with: "[^/]") + "$"
        }
        let options: NSRegularExpression.Options = caseSensitive ? [] : [.caseInsensitive]
        guard let regex = try? NSRegularExpression(pattern: expression, options: options) else { return false }
        return regex.firstMatch(in: value, range: NSRange(value.startIndex..., in: value)) != nil
    }

    private static func fuzzy(_ query: String, caseSensitive: Bool, matches value: String) -> Bool {
        let haystack = caseSensitive ? value : value.lowercased()
        return query.split(whereSeparator: \.isWhitespace).allSatisfy { rawToken in
            let token = caseSensitive ? String(rawToken) : String(rawToken).lowercased()
            var position = haystack.startIndex
            for character in token {
                guard let match = haystack[position...].firstIndex(of: character) else { return false }
                position = haystack.index(after: match)
            }
            return true
        }
    }
}
