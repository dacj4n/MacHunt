import AppKit
import CoreServices
import Foundation
import MacHuntCore

@MainActor
final class SearchStore: ObservableObject {
    @Published var query = "" { didSet { scheduleSearch() } }
    @Published var pathPrefix = "" { didSet { scheduleSearch() } }
    @Published var mode: SearchMode = .substring { didSet { scheduleSearch() } }
    @Published var caseSensitive = false { didSet { scheduleSearch() } }
    @Published var selectedCategoryID = ResultCategory.all.rawValue { didSet { scheduleSearch() } }
    @Published var selectedApplicationID = "" { didSet { scheduleSearch() } }
    @Published private(set) var applicationGroups: [ApplicationGroup] = [.all]
    @Published private(set) var pathSuggestions: [String] = []
    @Published var sort: SearchSort = .name { didSet { scheduleSearch(immediate: true) } }
    @Published var ascending = true { didSet { scheduleSearch(immediate: true) } }
    @Published var minimumSize: Int64? { didSet { scheduleSearch() } }
    @Published var maximumSize: Int64? { didSet { scheduleSearch() } }
    @Published var modifiedAfter: Date? { didSet { scheduleSearch() } }
    @Published var modifiedBefore: Date? { didSet { scheduleSearch() } }

    @Published private(set) var results: [SearchResult] = []
    @Published var selection: Set<SearchResult.ID> = []
    @Published private(set) var indexedItemCount = 0
    @Published private(set) var rebuildItemCount = 0
    @Published private(set) var isSearching = false
    @Published private(set) var isBuilding = false
    @Published private(set) var isFirstRun = false
    @Published private(set) var isMonitoring = false
    @Published private(set) var status = "Opening index…"
    @Published var errorMessage: String?

    private var database: SearchDatabase?
    private var searchTask: Task<Void, Never>?
    private var indexBuildTask: Task<Void, Never>?
    private var startupTask: Task<Void, Never>?
    private var applicationGroupReloadTask: Task<Void, Never>?
    private var generation = 0
    private let preferences: AppPreferences
    private let fileSystemMonitor = FileSystemMonitor()
    private let volumeMonitor = VolumeMonitor()

    init(preferences: AppPreferences) {
        self.preferences = preferences
        volumeMonitor.onMount = { [weak self] url in self?.volumeDidMount(url) }
        volumeMonitor.onUnmount = { [weak self] url in self?.volumeDidUnmount(url) }
        volumeMonitor.start()
        startupTask = Task { await openDatabase() }
    }

    var selectedResults: [SearchResult] {
        results.filter { selection.contains($0.id) }
    }

    func submitSearch() {
        scheduleSearch(immediate: true)
    }

    func toggleSortDirection() {
        ascending.toggle()
    }

    func refreshPathSuggestions() {
        guard let database else { return }
        do {
            pathSuggestions = try database.pathSuggestions(matching: pathPrefix, limit: 30)
        } catch {
            pathSuggestions = []
        }
    }

    func choosePathPrefix() {
        let panel = NSOpenPanel()
        panel.title = "Choose a folder to search"
        panel.prompt = "Choose"
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        pathPrefix = url.standardizedFileURL.path
    }

    func setSort(_ sort: SearchSort, ascending: Bool) {
        self.sort = sort
        self.ascending = ascending
    }

    func removeFromIndex(_ items: [SearchResult]) {
        guard let database else { return }
        Task {
            do {
                for item in items { _ = try await database.removeIndexedTree(at: item.path) }
                indexedItemCount = await Self.itemCount(in: database) ?? indexedItemCount
                await reloadApplicationGroups(using: database)
                scheduleSearch(immediate: true)
            } catch { errorMessage = error.localizedDescription }
        }
    }

    func chooseFolderAndRebuild() {
        let panel = NSOpenPanel()
        panel.title = "Choose a folder to index"
        panel.prompt = "Index"
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        rebuildIndex(at: url)
    }

    func rebuildIndex(at url: URL) {
        guard let database else { return }
        searchTask?.cancel()
        applicationGroupReloadTask?.cancel()
        indexBuildTask?.cancel()
        stopMonitoring()
        isBuilding = true
        errorMessage = nil
        status = "Indexing \(url.path)…"
        rebuildItemCount = 0
        indexBuildTask = Task {
            let progressTask = indexProgressTask(
                using: database,
                statusPrefix: "Indexing \(url.lastPathComponent)"
            )
            defer {
                progressTask.cancel()
                indexBuildTask = nil
            }
            do {
                let summary = try await database.rebuildIndex(at: url)
                try await database.saveLastEventID(UInt64(FSEventsGetCurrentEventId()))
                indexedItemCount = summary.itemCount
                rebuildItemCount = summary.itemCount
                await reloadApplicationGroups(using: database)
                status = "Indexed \(summary.itemCount.formatted()) items"
                isBuilding = false
                startMonitoring()
                scheduleSearch(immediate: true)
            } catch is CancellationError {
                status = "Index build cancelled"
                isBuilding = false
                rebuildItemCount = 0
                startMonitoring()
            } catch {
                errorMessage = error.localizedDescription
                status = "Index build failed"
                isBuilding = false
                rebuildItemCount = 0
                startMonitoring()
            }
        }
    }

    func rebuildConfiguredIndex() {
        guard let database else { return }
        searchTask?.cancel()
        applicationGroupReloadTask?.cancel()
        indexBuildTask?.cancel()
        stopMonitoring()
        isBuilding = true
        errorMessage = nil
        status = "Rebuilding configured index…"
        rebuildItemCount = 0
        let configuration = activeConfiguration
        indexBuildTask = Task {
            let progressTask = indexProgressTask(
                using: database,
                statusPrefix: "Rebuilding index"
            )
            defer {
                progressTask.cancel()
                indexBuildTask = nil
            }
            do {
                let summary = try await database.rebuildIndex(configuration: configuration)
                try await database.saveLastEventID(UInt64(FSEventsGetCurrentEventId()))
                indexedItemCount = summary.itemCount
                rebuildItemCount = summary.itemCount
                await reloadApplicationGroups(using: database)
                status = "Indexed \(summary.itemCount.formatted()) items"
                isBuilding = false
                startMonitoring()
                scheduleSearch(immediate: true)
            } catch is CancellationError {
                status = "Index build cancelled"
                isBuilding = false
                rebuildItemCount = 0
                startMonitoring()
            } catch {
                errorMessage = error.localizedDescription
                status = "Index build failed"
                isBuilding = false
                rebuildItemCount = 0
                startMonitoring()
            }
        }
    }

    func cancelIndexBuild() {
        startupTask?.cancel()
        startupTask = nil
        indexBuildTask?.cancel()
        if isBuilding {
            status = "Cancelling index build…"
        }
    }

    func clearIndex() {
        guard let database else { return }
        cancelIndexBuild()
        stopMonitoring()
        Task {
            do {
                try await database.clearIndex()
                indexedItemCount = 0
                results = []
                selection = []
                status = "Index cleared"
            } catch { errorMessage = error.localizedDescription }
        }
    }

    func setMonitoringPaused(_ paused: Bool) {
        preferences.monitoringPaused = paused
        paused ? stopMonitoring() : startMonitoring()
        status = paused ? "Index monitoring paused" : "Index monitoring active"
    }

    private func openDatabase() async {
        do {
            let url = try SearchDatabase.defaultURL()
            let database = try await Task.detached(priority: .userInitiated) {
                try SearchDatabase(url: url)
            }.value
            self.database = database
            indexedItemCount = await Self.itemCount(in: database) ?? 0
            let indexIsComplete = try database.indexIsComplete()
            let needsBootstrap = IndexBootstrapPolicy.shouldBuildIndex(
                itemCount: indexedItemCount,
                indexIsComplete: indexIsComplete
            )
            let configurationMatches = try database.indexMatches(configuration: activeConfiguration)
            if needsBootstrap {
                await buildInitialIndex(using: database)
            } else if !indexIsComplete || !configurationMatches {
                status = indexIsComplete
                    ? "Index settings changed — rebuild when convenient"
                    : "Partial index — \(indexedItemCount.formatted()) items available; rebuild when convenient"
                startMonitoring()
                results = []
                scheduleApplicationGroupReload(using: database, delay: .zero)
            } else {
                status = "Indexed \(indexedItemCount.formatted()) items"
                startMonitoring()
                results = []
                scheduleApplicationGroupReload(using: database, delay: .zero)
                Task(priority: .utility) {
                    do {
                        try await database.optimizeForSearching()
                        status = "Indexed \(indexedItemCount.formatted()) items — optimized"
                    } catch { errorMessage = error.localizedDescription }
                }
            }
        } catch {
            errorMessage = error.localizedDescription
            status = "Unable to open index"
        }
    }

    private func buildInitialIndex(using database: SearchDatabase) async {
        isFirstRun = true
        isBuilding = true
        let roots = activeConfiguration.roots.map { URL(fileURLWithPath: $0, isDirectory: true) }
        status = roots.contains { $0.path == "/" }
            ? "Building the full Mac index…"
            : "Building the configured search index…"
        let progressTask = indexProgressTask(
            using: database,
            statusPrefix: "Building first index"
        )
        defer { progressTask.cancel() }

        do {
            let summary = try await database.rebuildIndex(configuration: activeConfiguration)
            try await database.saveLastEventID(UInt64(FSEventsGetCurrentEventId()))
            indexedItemCount = summary.itemCount
            rebuildItemCount = summary.itemCount
            await reloadApplicationGroups(using: database)
            status = "Indexed \(summary.itemCount.formatted()) items"
            isBuilding = false
            isFirstRun = false
            startMonitoring()
            scheduleSearch(immediate: true)
        } catch is CancellationError {
            status = "Index build cancelled"
            isBuilding = false
            isFirstRun = false
            rebuildItemCount = 0
            startMonitoring()
        } catch {
            errorMessage = error.localizedDescription
            status = "First index build failed"
            isBuilding = false
            isFirstRun = false
            rebuildItemCount = 0
            startMonitoring()
        }
    }

    private func indexProgressTask(
        using database: SearchDatabase,
        statusPrefix: String
    ) -> Task<Void, Never> {
        Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled, let self else { return }
                guard let progress = await Self.rebuildProgress(in: database) else { continue }
                self.rebuildItemCount = progress.itemCount
                self.status = switch progress.phase {
                case .scanning:
                    "\(statusPrefix) — \(progress.itemCount.formatted()) items scanned"
                case .buildingSearchIndex:
                    "Building search index — \(progress.itemCount.formatted()) items"
                case .optimizing:
                    "Optimizing database — \(progress.itemCount.formatted()) items"
                case .complete:
                    "Finishing index — \(progress.itemCount.formatted()) items"
                }
            }
        }
    }

    private func scheduleSearch(immediate: Bool = false) {
        guard let database else { return }
        searchTask?.cancel()
        if query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
           pathPrefix.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
           selectedCategoryID == ResultCategory.all.rawValue,
           selectedApplicationID.isEmpty,
           minimumSize == nil,
           maximumSize == nil,
           modifiedAfter == nil,
           modifiedBefore == nil {
            results = []
            selection = []
            isSearching = false
            if !isBuilding { status = "Indexed \(indexedItemCount.formatted()) items" }
            return
        }
        generation += 1
        let requestedGeneration = generation
        let selectedApplication = applicationGroups.first { $0.id == selectedApplicationID }
        let selectedCategory = preferences.categoryOption(id: selectedCategoryID)
            ?? preferences.categoryOption(id: ResultCategory.all.rawValue)!
        let request = SearchRequest(
            query: query,
            mode: mode,
            caseSensitive: caseSensitive,
            pathPrefix: pathPrefix.isEmpty ? nil : pathPrefix,
            category: selectedCategory.category,
            applicationExtensions: selectedApplication?.extensions.isEmpty == false
                ? selectedApplication?.extensions
                : nil,
            categoryExtensions: selectedCategory.extensions,
            minimumSize: minimumSize,
            maximumSize: maximumSize,
            modifiedAfter: modifiedAfter,
            modifiedBefore: modifiedBefore,
            sort: sort,
            ascending: ascending,
            limit: preferences.maximumResults
        )

        searchTask = Task {
            do {
                if !immediate { try await Task.sleep(for: .milliseconds(200)) }
                isSearching = true
                let next = try await Self.performSearch(request, in: database)
                try Task.checkCancellation()
                guard requestedGeneration == generation else { return }
                results = next
                selection.formIntersection(Set(next.map(\.id)))
                if !isBuilding {
                    status = "Showing \(next.count.formatted()) of \(indexedItemCount.formatted()) indexed items"
                }
                isSearching = false
            } catch is CancellationError {
                if requestedGeneration == generation { isSearching = false }
            } catch {
                guard requestedGeneration == generation else { return }
                errorMessage = error.localizedDescription
                isSearching = false
            }
        }
    }

    private var activeConfiguration: IndexConfiguration {
        preferences.indexConfiguration
    }

    private var monitoringRoots: [String] {
        var roots = activeConfiguration.roots
        if activeConfiguration.indexExternalVolumes {
            let externalRoots = IndexBootstrapPolicy.defaultRoots().dropFirst().map(\.path)
            roots.append(contentsOf: externalRoots.filter { !roots.contains($0) })
        }
        return roots
    }

    private func startMonitoring() {
        guard let database, !preferences.monitoringPaused, !isBuilding else { return }
        let eventID = try? database.lastEventID()
        isMonitoring = fileSystemMonitor.start(paths: monitoringRoots, since: eventID) { [weak self] batch in
            Task { @MainActor [weak self] in
                await self?.applyFileSystemBatch(batch)
            }
        }
    }

    private func stopMonitoring() {
        fileSystemMonitor.stop()
        isMonitoring = false
    }

    private func applyFileSystemBatch(_ batch: FileSystemMonitor.Batch) async {
        guard let database, !preferences.monitoringPaused, !isBuilding else { return }
        if batch.requiresFullRescan {
            status = "File-system history changed; rebuilding index…"
            rebuildConfiguredIndex()
            return
        }
        do {
            let changed = try await database.applyFileSystemChanges(
                paths: batch.paths,
                configuration: activeConfiguration
            )
            try await database.saveLastEventID(batch.lastEventID)
            if changed > 0 {
                indexedItemCount = await Self.itemCount(in: database) ?? indexedItemCount
                scheduleApplicationGroupReload(using: database)
                status = "Updated \(changed.formatted()) items"
                scheduleSearch(immediate: true)
            }
        } catch { errorMessage = error.localizedDescription }
    }

    private func volumeDidMount(_ url: URL) {
        guard preferences.indexExternalVolumes, !preferences.monitoringPaused, let database else { return }
        status = "Indexing mounted volume \(url.lastPathComponent)…"
        Task {
            do {
                _ = try await database.indexTree(at: url.path, configuration: activeConfiguration)
                indexedItemCount = await Self.itemCount(in: database) ?? indexedItemCount
                await reloadApplicationGroups(using: database)
                startMonitoring()
                scheduleSearch(immediate: true)
            } catch { errorMessage = error.localizedDescription }
        }
    }

    private func volumeDidUnmount(_ url: URL) {
        guard let database else { return }
        Task {
            do {
                _ = try await database.removeIndexedTree(at: url.path)
                indexedItemCount = await Self.itemCount(in: database) ?? indexedItemCount
                await reloadApplicationGroups(using: database)
                startMonitoring()
                scheduleSearch(immediate: true)
            } catch { errorMessage = error.localizedDescription }
        }
    }

    private func reloadApplicationGroups(using database: SearchDatabase) async {
        guard let counts = await Self.extensionCounts(in: database) else { return }
        applicationGroups = ApplicationCatalog.groups(from: counts)
        if !applicationGroups.contains(where: { $0.id == selectedApplicationID }) {
            selectedApplicationID = ""
        }
    }

    private func scheduleApplicationGroupReload(
        using database: SearchDatabase,
        delay: Duration = .seconds(2)
    ) {
        applicationGroupReloadTask?.cancel()
        applicationGroupReloadTask = Task { [weak self] in
            if delay > .zero { try? await Task.sleep(for: delay) }
            guard !Task.isCancelled else { return }
            await self?.reloadApplicationGroups(using: database)
        }
    }

    private nonisolated static func performSearch(
        _ request: SearchRequest,
        in database: SearchDatabase
    ) async throws -> [SearchResult] {
        try Task.checkCancellation()
        let results = try database.search(request)
        try Task.checkCancellation()
        return results
    }

    private nonisolated static func itemCount(in database: SearchDatabase) async -> Int? {
        try? database.indexedItemCount()
    }

    private nonisolated static func rebuildProgress(in database: SearchDatabase) async -> IndexBuildProgress? {
        try? database.rebuildProgress()
    }

    private nonisolated static func extensionCounts(in database: SearchDatabase) async -> [String: Int]? {
        try? database.extensionCounts()
    }
}

@MainActor
final class PinnedStore: ObservableObject {
    @Published private(set) var items: [SearchResult] = []
    private let storageURL: URL
    private var records: [PinnedRecord] = []

    init() {
        let base = (try? FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )) ?? FileManager.default.temporaryDirectory
        storageURL = base.appending(path: "MacHuntSwift/pinned.json")
        load()
    }

    func contains(_ item: SearchResult) -> Bool { items.contains { $0.path == item.path } }

    func toggle(_ item: SearchResult) {
        if let index = records.firstIndex(where: { $0.item.path == item.path }) {
            records.remove(at: index)
        } else {
            let url = URL(fileURLWithPath: item.path)
            let bookmark = try? url.bookmarkData(options: .minimalBookmark, includingResourceValuesForKeys: nil, relativeTo: nil)
            records.append(PinnedRecord(item: item, bookmark: bookmark))
        }
        items = records.map(\.item)
        save()
    }

    func removeIfPresent(_ item: SearchResult) {
        let originalCount = records.count
        records.removeAll { $0.item.path == item.path }
        guard records.count != originalCount else { return }
        items = records.map(\.item)
        save()
    }

    func refreshLocations() {
        records = records.compactMap { record in
            guard let bookmark = record.bookmark else {
                return FileManager.default.fileExists(atPath: record.item.path) ? record : nil
            }
            var stale = false
            guard let url = try? URL(
                resolvingBookmarkData: bookmark,
                options: [.withoutUI, .withoutMounting],
                relativeTo: nil,
                bookmarkDataIsStale: &stale
            ), FileManager.default.fileExists(atPath: url.path) else { return nil }
            let refreshed = Self.result(at: url) ?? record.item
            let nextBookmark = stale
                ? (try? url.bookmarkData(options: .minimalBookmark, includingResourceValuesForKeys: nil, relativeTo: nil))
                : bookmark
            return PinnedRecord(item: refreshed, bookmark: nextBookmark)
        }
        items = records.map(\.item)
        save()
    }

    private func load() {
        guard let data = try? Data(contentsOf: storageURL) else { return }
        if let decoded = try? JSONDecoder().decode([PinnedRecord].self, from: data) {
            records = decoded
        } else if let legacy = try? JSONDecoder().decode([SearchResult].self, from: data) {
            records = legacy.map { PinnedRecord(item: $0, bookmark: nil) }
        }
        refreshLocations()
    }

    private func save() {
        do {
            try FileManager.default.createDirectory(at: storageURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            let data = try JSONEncoder().encode(records)
            try data.write(to: storageURL, options: .atomic)
        } catch {
            NSSound.beep()
        }
    }

    private static func result(at url: URL) -> SearchResult? {
        let keys: Set<URLResourceKey> = [
            .isDirectoryKey, .fileSizeKey, .contentModificationDateKey, .addedToDirectoryDateKey,
            .isUbiquitousItemKey, .ubiquitousItemHasUnresolvedConflictsKey,
            .ubiquitousItemIsDownloadingKey, .ubiquitousItemIsUploadedKey,
            .ubiquitousItemIsUploadingKey, .ubiquitousItemDownloadingStatusKey,
            .tagNamesKey,
        ]
        guard let values = try? url.resourceValues(forKeys: keys) else { return nil }
        return SearchResult(
            name: url.lastPathComponent,
            path: url.path,
            parent: url.deletingLastPathComponent().path,
            isDirectory: values.isDirectory == true,
            sizeBytes: values.isDirectory == true ? nil : values.fileSize.map(Int64.init),
            modifiedAt: values.contentModificationDate,
            addedAt: values.addedToDirectoryDate,
            cloudStatus: Self.cloudStatus(from: values),
            tags: values.tagNames ?? []
        )
    }

    private static func cloudStatus(from values: URLResourceValues) -> CloudFileStatus? {
        guard values.isUbiquitousItem == true else { return nil }
        if values.ubiquitousItemHasUnresolvedConflicts == true { return .conflict }
        if values.ubiquitousItemIsDownloading == true { return .downloading }
        if values.ubiquitousItemIsUploading == true { return .uploading }
        switch values.ubiquitousItemDownloadingStatus {
        case .notDownloaded: return .inCloud
        case .downloaded: return .downloaded
        case .current: return values.ubiquitousItemIsUploaded == true ? .synced : .downloaded
        default: return values.ubiquitousItemIsUploaded == true ? .synced : .inCloud
        }
    }

    private struct PinnedRecord: Codable {
        var item: SearchResult
        var bookmark: Data?
    }
}
