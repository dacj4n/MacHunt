import Foundation
import SQLite3
import Testing
@testable import MacHuntCore

@Suite("Search database")
struct SearchDatabaseTests {
    @Test("Only an empty index requires automatic first-run bootstrap")
    func emptyIndexBootstrap() {
        #expect(IndexBootstrapPolicy.shouldBuildIndex(itemCount: 0, indexIsComplete: false))
        #expect(!IndexBootstrapPolicy.shouldBuildIndex(itemCount: 1, indexIsComplete: false))
        #expect(!IndexBootstrapPolicy.shouldBuildIndex(itemCount: 1, indexIsComplete: true))
    }

    @Test("Pinned items saved before metadata columns were added still decode")
    func legacySearchResultDecoding() throws {
        let data = Data(#"{"name":"Old","path":"/tmp/Old","parent":"/tmp","isDirectory":false,"sizeBytes":null,"modifiedAt":null}"#.utf8)
        let item = try JSONDecoder().decode(SearchResult.self, from: data)

        #expect(item.addedAt == nil)
        #expect(item.cloudStatus == nil)
        #expect(item.tags.isEmpty)
    }

    @Test("A rebuild atomically replaces the index and removes staging files")
    func atomicRebuildCleanup() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)
        try Data().write(to: fixture.rootURL.appending(path: "Added Later.txt"))

        _ = try await database.rebuildIndex(at: fixture.rootURL)

        #expect(try database.search(SearchRequest(query: "Added Later")).count == 1)
        let progress = try database.rebuildProgress()
        #expect(progress.phase == .complete)
        #expect(progress.itemCount == 5)
        for suffix in [".new", ".new-wal", ".new-shm"] {
            #expect(!FileManager.default.fileExists(atPath: fixture.databaseURL.path + suffix))
        }
    }

    @Test("The compact schema stores paths once and keeps FTS contentless")
    func compactSchema() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        var rawDatabase: OpaquePointer?
        #expect(sqlite3_open_v2(fixture.databaseURL.path, &rawDatabase, SQLITE_OPEN_READONLY, nil) == SQLITE_OK)
        guard let rawDatabase else { return }
        defer { sqlite3_close(rawDatabase) }

        let fileColumns = try queryStrings(rawDatabase, sql: "SELECT name FROM pragma_table_info('files')")
        let tables = try queryStrings(rawDatabase, sql: "SELECT name FROM sqlite_master WHERE type = 'table'")

        #expect(fileColumns.contains("directory_id"))
        #expect(!fileColumns.contains("path"))
        #expect(!fileColumns.contains("parent"))
        #expect(!fileColumns.contains("tags"))
        #expect(tables.contains("directories"))
        #expect(tables.contains("file_tags"))
        #expect(!tables.contains("files_fts_content"))
    }

    @Test("The filesystem root includes normal system-wide locations")
    func filesystemRootScope() {
        let configuration = IndexConfiguration(roots: ["/"])

        #expect(configuration.includes(
            path: "/Applications/Utilities/Terminal.app",
            isDirectory: true,
            isHidden: false
        ))
    }

    @Test("FTS trigram finds a substring")
    func substringSearch() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        #expect(try database.indexIsComplete())

        let results = try database.search(SearchRequest(query: "library"))
        #expect(results.map(\.name) == ["MyPhotoLibraryBackup.txt"])
    }

    @Test("Fuzzy search requires every token")
    func fuzzySearch() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(query: "project final", mode: .fuzzy))
        #expect(results.map(\.name) == ["Project Final.swift"])
    }

    @Test("Category filtering distinguishes folders and files")
    func categoryFilter() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let folders = try database.search(SearchRequest(category: .folders))
        #expect(folders.contains { $0.name == "Images" })
        #expect(folders.allSatisfy { $0.isDirectory })
    }

    @Test("A dynamic application group filters by its extensions")
    func dynamicApplicationFilter() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(
            applicationExtensions: ["swift"]
        ))

        #expect(results.map(\.name) == ["Project Final.swift"])
    }

    @Test("A custom category uses user-defined file extensions")
    func customCategoryFilter() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let swiftOnly = try database.search(SearchRequest(
            category: .custom,
            categoryExtensions: ["swift"]
        ))
        let noConfiguredTypes = try database.search(SearchRequest(
            category: .custom,
            categoryExtensions: []
        ))

        #expect(swiftOnly.map(\.name) == ["Project Final.swift"])
        #expect(noConfiguredTypes.isEmpty)
    }

    @Test("Built-in category extensions can be replaced by user settings")
    func editableBuiltInCategoryFilter() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(
            category: .documents,
            categoryExtensions: ["swift"]
        ))

        #expect(results.map(\.name) == ["Project Final.swift"])
    }

    @Test("Application groups can be built from indexed extension counts")
    func extensionCounts() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let counts = try database.extensionCounts()

        #expect(counts["swift"] == 1)
        #expect(counts["png"] == 1)
        #expect(counts["txt"] == 1)
    }

    @Test("File-system changes add and remove results without rebuilding")
    func incrementalChanges() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let addedURL = fixture.rootURL.appending(path: "Added Later.txt")
        try Data().write(to: addedURL)
        _ = try await database.applyFileSystemChanges(
            paths: [fixture.rootURL.path, addedURL.path],
            configuration: .init(roots: [fixture.rootURL.path])
        )
        #expect(try database.search(SearchRequest(query: "Added Later")).count == 1)

        try FileManager.default.removeItem(at: addedURL)
        _ = try await database.applyFileSystemChanges(
            paths: [addedURL.path],
            configuration: .init(roots: [fixture.rootURL.path])
        )
        #expect(try database.search(SearchRequest(query: "Added Later")).isEmpty)
    }

    @Test("Index configuration excludes hidden and matching files")
    func configuredIndexing() async throws {
        let fixture = try Fixture()
        try Data().write(to: fixture.rootURL.appending(path: ".secret.txt"))
        try Data().write(to: fixture.rootURL.appending(path: "ignored.tmp"))
        let database = try SearchDatabase(url: fixture.databaseURL)
        let configuration = IndexConfiguration(
            roots: [fixture.rootURL.path],
            excludeHiddenItems: true,
            excludedFilePatterns: ["*.tmp"]
        )

        _ = try await database.rebuildIndex(configuration: configuration)

        #expect(try database.search(SearchRequest(query: "secret")).isEmpty)
        #expect(try database.search(SearchRequest(query: "ignored")).isEmpty)
        #expect(!(try database.search(SearchRequest(query: "Project Final"))).isEmpty)
    }

    @Test("An index records the configuration used to build it")
    func indexConfigurationIdentity() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        let configuration = IndexConfiguration(roots: [fixture.rootURL.path])

        #expect(!(try database.indexMatches(configuration: configuration)))
        _ = try await database.rebuildIndex(configuration: configuration)
        #expect(try database.indexMatches(configuration: configuration))
    }

    @Test("Pattern search scans beyond the first candidate page")
    func completePatternSearch() async throws {
        let fixture = try Fixture()
        for index in 0..<2_100 {
            try Data().write(to: fixture.rootURL.appending(path: "A\(index).txt"))
        }
        try Data().write(to: fixture.rootURL.appending(path: "ZTarget.txt"))
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(query: "ZTarget.*", mode: .pattern, limit: 10))

        #expect(results.map(\.name) == ["ZTarget.txt"])
    }

    @Test("Type sorting uses the file extension")
    func typeSorting() async throws {
        let fixture = try Fixture()
        try Data().write(to: fixture.rootURL.appending(path: "z.swift"))
        try Data().write(to: fixture.rootURL.appending(path: "a.txt"))
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(query: ".", sort: .type))
        let names = results.map(\.name)

        #expect(names.firstIndex(of: "z.swift")! < names.firstIndex(of: "a.txt")!)
    }

    @Test("FSEvents recovery position persists across database instances")
    func eventIDPersistence() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        try await database.saveLastEventID(42_4242)

        #expect(try database.lastEventID() == 42_4242)
    }

    @Test("Absolute path prefix limits results to that folder")
    func absolutePathPrefix() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(pathPrefix: fixture.rootURL.appending(path: "Images").path))

        #expect(results.map(\.name) == ["photo.png"])
    }

    @Test("The filesystem root path prefix includes every indexed item")
    func rootPathPrefix() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(pathPrefix: "/", limit: 20))

        #expect(results.count == 4)
    }

    @Test("Indexed folders provide path suggestions")
    func pathSuggestions() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let suggestions = try database.pathSuggestions(
            matching: fixture.rootURL.path,
            limit: 20
        )

        #expect(suggestions.contains { $0.hasSuffix("/root/Images") })
    }

    @Test("Fuzzy search matches ordered non-contiguous characters")
    func fuzzySubsequence() async throws {
        let fixture = try Fixture()
        let database = try SearchDatabase(url: fixture.databaseURL)
        _ = try await database.rebuildIndex(at: fixture.rootURL)

        let results = try database.search(SearchRequest(query: "prj fnl", mode: .fuzzy))

        #expect(results.map(\.name) == ["Project Final.swift"])
    }

    private struct Fixture {
        let rootURL: URL
        let databaseURL: URL

        init() throws {
            let base = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
            rootURL = base.appending(path: "root")
            databaseURL = base.appending(path: "index.db")
            try FileManager.default.createDirectory(at: rootURL.appending(path: "Images"), withIntermediateDirectories: true)
            try Data().write(to: rootURL.appending(path: "MyPhotoLibraryBackup.txt"))
            try Data().write(to: rootURL.appending(path: "Project Final.swift"))
            try Data().write(to: rootURL.appending(path: "Images/photo.png"))
        }
    }

    private func queryStrings(_ database: OpaquePointer, sql: String) throws -> [String] {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, sql, -1, &statement, nil) == SQLITE_OK,
              let statement else { return [] }
        defer { sqlite3_finalize(statement) }
        var values: [String] = []
        while sqlite3_step(statement) == SQLITE_ROW {
            if let text = sqlite3_column_text(statement, 0) { values.append(String(cString: text)) }
        }
        return values
    }
}
