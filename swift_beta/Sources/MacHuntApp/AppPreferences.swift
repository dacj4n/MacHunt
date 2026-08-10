import AppKit
import Foundation
import MacHuntCore

enum AppAppearance: String, CaseIterable, Codable, Identifiable {
    case system, light, dark
    var id: String { rawValue }
    var title: String { rawValue.capitalized }
    var colorScheme: NSAppearance.Name? {
        switch self {
        case .system: nil
        case .light: .aqua
        case .dark: .darkAqua
        }
    }
}

enum AppLanguage: String, CaseIterable, Codable, Identifiable {
    case system, english, chinese
    var id: String { rawValue }
    var title: String {
        switch self {
        case .system: "System"
        case .english: "English"
        case .chinese: "中文"
        }
    }
    var localeIdentifier: String? {
        switch self {
        case .system: nil
        case .english: "en"
        case .chinese: "zh-Hans"
        }
    }
}

enum GlobalShortcutChoice: String, CaseIterable, Codable, Identifiable {
    case optionSpace, commandShiftSpace, controlSpace
    var id: String { rawValue }
    var title: String {
        switch self {
        case .optionSpace: "⌥ Space"
        case .commandShiftSpace: "⇧⌘ Space"
        case .controlSpace: "⌃ Space"
        }
    }
}

struct CustomFileTypeCategory: Identifiable, Codable, Equatable {
    var id: UUID
    var name: String
    var extensions: [String]
    var isVisible: Bool
    var systemImage: String?
    var emoji: String?
}

struct SearchCategoryOption: Identifiable, Equatable {
    let id: String
    let title: String
    let systemImage: String
    let emoji: String?
    let category: ResultCategory
    let extensions: Set<String>?
}

@MainActor
final class AppPreferences: ObservableObject {
    @Published var appearance: AppAppearance { didSet { persistAndApply() } }
    @Published var language: AppLanguage { didSet { persist() } }
    @Published var watchRoots: [String] { didSet { persist() } }
    @Published var excludedDirectories: [String] { didSet { persist() } }
    @Published var excludedDirectoryPatterns: [String] { didSet { persist() } }
    @Published var excludeHiddenItems: Bool { didSet { persist() } }
    @Published var excludedFilePatterns: [String] { didSet { persist() } }
    @Published var indexExternalVolumes: Bool { didSet { persist() } }
    @Published var maximumResults: Int { didSet { persist() } }
    @Published private(set) var enabledFileTypeCategories: [ResultCategory] { didSet { persist() } }
    @Published private(set) var fileTypeExtensions: [String: [String]] { didSet { persist() } }
    @Published private(set) var customFileTypeCategories: [CustomFileTypeCategory] { didSet { persist() } }
    @Published private(set) var visibleResultColumns: [String] { didSet { persist() } }
    @Published var monitoringPaused: Bool { didSet { persist() } }
    @Published var showMenuBarItem: Bool { didSet { persistAndNotifySystem() } }
    @Published var showDockIcon: Bool { didSet { persistAndNotifySystem() } }
    @Published var launchAtLogin: Bool { didSet { persistAndNotifySystem() } }
    @Published var globalShortcut: GlobalShortcutChoice { didSet { persistAndNotifySystem() } }
    @Published var confirmBeforeTrash: Bool { didSet { persist() } }

    private let defaults: UserDefaults
    private var isLoading = true
    private static let key = "appPreferences.v2"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let saved = defaults.data(forKey: Self.key).flatMap { try? JSONDecoder().decode(Snapshot.self, from: $0) }
        var initial = saved ?? Snapshot.default
        var migratedLegacyScope = false
        let oldDefaultPatterns = ["*/Library/Caches/*", "*/.Trash/*"]
        if initial.watchRoots == [FileManager.default.homeDirectoryForCurrentUser.path] {
            initial.watchRoots = ["/"]
            migratedLegacyScope = true
        }
        if initial.excludedDirectoryPatterns == oldDefaultPatterns {
            initial.excludedDirectoryPatterns = Snapshot.default.excludedDirectoryPatterns
            migratedLegacyScope = true
        }
        appearance = initial.appearance
        language = initial.language
        watchRoots = initial.watchRoots
        excludedDirectories = initial.excludedDirectories
        excludedDirectoryPatterns = initial.excludedDirectoryPatterns
        excludeHiddenItems = initial.excludeHiddenItems
        excludedFilePatterns = initial.excludedFilePatterns
        indexExternalVolumes = initial.indexExternalVolumes
        maximumResults = initial.maximumResults
        enabledFileTypeCategories = (initial.enabledFileTypeCategories ?? ResultCategory.editableCases)
            .filter { ResultCategory.editableCases.contains($0) }
        let savedExtensions = initial.fileTypeExtensions ?? Self.defaultFileTypeExtensions
        fileTypeExtensions = savedExtensions
        if let savedCustomCategories = initial.customFileTypeCategories {
            customFileTypeCategories = savedCustomCategories
        } else {
            let legacyExtensions = initial.customFileExtensions
                ?? savedExtensions[ResultCategory.custom.rawValue]
                ?? []
            let legacyWasVisible = initial.enabledFileTypeCategories?.contains(.custom) == true
            customFileTypeCategories = legacyWasVisible || !legacyExtensions.isEmpty
                ? [CustomFileTypeCategory(
                    id: UUID(),
                    name: "Custom",
                    extensions: legacyExtensions,
                    isVisible: true,
                    systemImage: "tag",
                    emoji: nil
                )]
                : []
        }
        visibleResultColumns = initial.visibleResultColumns ?? ResultTableColumn.defaultIDs
        monitoringPaused = initial.monitoringPaused
        showMenuBarItem = initial.showMenuBarItem
        showDockIcon = initial.showDockIcon
        launchAtLogin = initial.launchAtLogin
        globalShortcut = initial.globalShortcut
        confirmBeforeTrash = initial.confirmBeforeTrash
        isLoading = false
        applyAppearance()
        if migratedLegacyScope { persist() }
    }

    var indexConfiguration: IndexConfiguration {
        IndexConfiguration(
            roots: watchRoots,
            excludedDirectories: excludedDirectories,
            excludedDirectoryPatterns: excludedDirectoryPatterns,
            excludeHiddenItems: excludeHiddenItems,
            excludedFilePatterns: excludedFilePatterns,
            indexExternalVolumes: indexExternalVolumes
        )
    }

    func resetIndexRules() {
        excludedDirectories = []
        excludedDirectoryPatterns = Snapshot.default.excludedDirectoryPatterns
        excludeHiddenItems = false
        excludedFilePatterns = Snapshot.default.excludedFilePatterns
    }

    var builtInSearchCategoryOptions: [SearchCategoryOption] {
        ResultCategory.allCases
            .filter { $0 != .custom && isCategoryVisible($0) }
            .map { category in
                SearchCategoryOption(
                    id: category.rawValue,
                    title: category.title,
                    systemImage: category.systemImage,
                    emoji: nil,
                    category: category,
                    extensions: extensions(for: category)
                )
            }
    }

    var customSearchCategoryOptions: [SearchCategoryOption] {
        customFileTypeCategories.filter(\.isVisible).map { item in
            SearchCategoryOption(
                id: "custom:\(item.id.uuidString)",
                title: item.name,
                systemImage: item.systemImage ?? "tag",
                emoji: item.emoji,
                category: .custom,
                extensions: Set(item.extensions)
            )
        }
    }

    var searchCategoryOptions: [SearchCategoryOption] {
        builtInSearchCategoryOptions + customSearchCategoryOptions
    }

    func categoryOption(id: String) -> SearchCategoryOption? {
        searchCategoryOptions.first { $0.id == id }
    }

    func isCategoryVisible(_ category: ResultCategory) -> Bool {
        category == .all || category == .files || category == .folders
            || enabledFileTypeCategories.contains(category)
    }

    func setCategory(_ category: ResultCategory, visible: Bool) {
        guard category.supportsEditableExtensions else { return }
        if visible {
            if !enabledFileTypeCategories.contains(category) {
                enabledFileTypeCategories.append(category)
                enabledFileTypeCategories.sort { $0.rawValue < $1.rawValue }
            }
        } else {
            enabledFileTypeCategories.removeAll { $0 == category }
        }
    }

    func extensions(for category: ResultCategory) -> Set<String>? {
        guard category.supportsEditableExtensions else { return nil }
        if let saved = fileTypeExtensions[category.rawValue] { return Set(saved) }
        return category.defaultExtensions ?? []
    }

    func setExtensions(for category: ResultCategory, from input: String) {
        guard category.supportsEditableExtensions else { return }
        fileTypeExtensions[category.rawValue] = Self.parseExtensions(input)
    }

    func resetExtensions(for category: ResultCategory) {
        guard category.supportsEditableExtensions else { return }
        fileTypeExtensions[category.rawValue] = Array(category.defaultExtensions ?? []).sorted()
    }

    @discardableResult
    func addCustomFileTypeCategory() -> UUID {
        let existingNames = Set(customFileTypeCategories.map { $0.name.lowercased() })
        var number = customFileTypeCategories.count + 1
        var name = "Custom \(number)"
        while existingNames.contains(name.lowercased()) {
            number += 1
            name = "Custom \(number)"
        }
        let item = CustomFileTypeCategory(
            id: UUID(),
            name: name,
            extensions: [],
            isVisible: true,
            systemImage: "tag",
            emoji: nil
        )
        customFileTypeCategories.append(item)
        return item.id
    }

    func updateCustomFileTypeCategory(
        id: UUID,
        name: String,
        extensions input: String,
        systemImage: String,
        emoji: String?
    ) {
        guard let index = customFileTypeCategories.firstIndex(where: { $0.id == id }) else { return }
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        customFileTypeCategories[index].name = trimmedName.isEmpty ? "Custom" : trimmedName
        customFileTypeCategories[index].extensions = Self.parseExtensions(input)
        customFileTypeCategories[index].systemImage = systemImage
        customFileTypeCategories[index].emoji = emoji
    }

    func setCustomFileTypeCategory(id: UUID, visible: Bool) {
        guard let index = customFileTypeCategories.firstIndex(where: { $0.id == id }) else { return }
        customFileTypeCategories[index].isVisible = visible
    }

    func removeCustomFileTypeCategory(id: UUID) {
        customFileTypeCategories.removeAll { $0.id == id }
    }

    func customFileTypeCategory(id: UUID) -> CustomFileTypeCategory? {
        customFileTypeCategories.first { $0.id == id }
    }

    func isResultColumnVisible(_ column: ResultTableColumn) -> Bool {
        column.isRequired || visibleResultColumns.contains(column.rawValue)
    }

    func setResultColumn(_ column: ResultTableColumn, visible: Bool) {
        guard !column.isRequired else { return }
        if visible {
            if !visibleResultColumns.contains(column.rawValue) {
                visibleResultColumns.append(column.rawValue)
            }
        } else {
            visibleResultColumns.removeAll { $0 == column.rawValue }
        }
        visibleResultColumns = ResultTableColumn.allCases
            .map(\.rawValue)
            .filter(visibleResultColumns.contains)
    }

    private static func normalizeExtension(_ input: String) -> String? {
        var value = input.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        while value.hasPrefix("*.") { value.removeFirst(2) }
        while value.hasPrefix(".") { value.removeFirst() }
        guard !value.isEmpty, !value.contains("/") else { return nil }
        return value
    }

    private static func parseExtensions(_ input: String) -> [String] {
        let separators = CharacterSet(charactersIn: ",; \n\t")
        return Array(Set(input.components(separatedBy: separators).compactMap(normalizeExtension))).sorted()
    }

    private func persistAndApply() { persist(); applyAppearance() }
    private func persistAndNotifySystem() {
        persist()
        NotificationCenter.default.post(name: .macHuntSystemPreferencesChanged, object: nil)
    }

    private func persist() {
        guard !isLoading else { return }
        let snapshot = Snapshot(
            appearance: appearance, language: language, watchRoots: watchRoots,
            excludedDirectories: excludedDirectories, excludedDirectoryPatterns: excludedDirectoryPatterns,
            excludeHiddenItems: excludeHiddenItems, excludedFilePatterns: excludedFilePatterns,
            indexExternalVolumes: indexExternalVolumes, maximumResults: maximumResults,
            customFileExtensions: nil,
            enabledFileTypeCategories: enabledFileTypeCategories,
            fileTypeExtensions: fileTypeExtensions,
            customFileTypeCategories: customFileTypeCategories,
            visibleResultColumns: visibleResultColumns,
            monitoringPaused: monitoringPaused, showMenuBarItem: showMenuBarItem,
            showDockIcon: showDockIcon, launchAtLogin: launchAtLogin,
            globalShortcut: globalShortcut, confirmBeforeTrash: confirmBeforeTrash
        )
        if let data = try? JSONEncoder().encode(snapshot) { defaults.set(data, forKey: Self.key) }
    }

    private func applyAppearance() {
        NSApp?.appearance = appearance.colorScheme.map(NSAppearance.init(named:)) ?? nil
    }

    private struct Snapshot: Codable {
        var appearance: AppAppearance
        var language: AppLanguage
        var watchRoots: [String]
        var excludedDirectories: [String]
        var excludedDirectoryPatterns: [String]
        var excludeHiddenItems: Bool
        var excludedFilePatterns: [String]
        var indexExternalVolumes: Bool
        var maximumResults: Int
        var customFileExtensions: [String]?
        var enabledFileTypeCategories: [ResultCategory]?
        var fileTypeExtensions: [String: [String]]?
        var customFileTypeCategories: [CustomFileTypeCategory]?
        var visibleResultColumns: [String]?
        var monitoringPaused: Bool
        var showMenuBarItem: Bool
        var showDockIcon: Bool
        var launchAtLogin: Bool
        var globalShortcut: GlobalShortcutChoice
        var confirmBeforeTrash: Bool

        static var `default`: Snapshot {
            Snapshot(
                appearance: .system,
                language: .system,
                watchRoots: ["/"],
                excludedDirectories: [],
                excludedDirectoryPatterns: IndexConfiguration(roots: ["/"]).excludedDirectoryPatterns,
                excludeHiddenItems: false,
                excludedFilePatterns: [".DS_Store", "*.tmp"],
                indexExternalVolumes: true,
                maximumResults: 500,
                customFileExtensions: [],
                enabledFileTypeCategories: ResultCategory.editableCases,
                fileTypeExtensions: AppPreferences.defaultFileTypeExtensions,
                customFileTypeCategories: [],
                visibleResultColumns: ResultTableColumn.defaultIDs,
                monitoringPaused: false,
                showMenuBarItem: true,
                showDockIcon: true,
                launchAtLogin: false,
                globalShortcut: .optionSpace,
                confirmBeforeTrash: true
            )
        }
    }

    nonisolated private static var defaultFileTypeExtensions: [String: [String]] {
        Dictionary(uniqueKeysWithValues: ResultCategory.editableCases.map { category in
            (category.rawValue, Array(category.defaultExtensions ?? []).sorted())
        })
    }
}

extension Notification.Name {
    static let macHuntSystemPreferencesChanged = Notification.Name("MacHuntSystemPreferencesChanged")
}
