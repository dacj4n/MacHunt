import AppKit

struct ApplicationGroup: Identifiable, Hashable {
    static let all = ApplicationGroup(id: "", name: "All Applications", extensions: [], itemCount: 0)

    let id: String
    let name: String
    let extensions: Set<String>
    let itemCount: Int
}

@MainActor
enum ApplicationCatalog {
    private static var applicationNameCache: [String: String] = [:]
    private static let associationLookupLimit = 256

    static func groups(from extensionCounts: [String: Int]) -> [ApplicationGroup] {
        var grouped: [String: (extensions: Set<String>, count: Int)] = [:]
        let rankedExtensions = extensionCounts
            .filter { !$0.key.isEmpty }
            .sorted {
                if $0.value != $1.value { return $0.value > $1.value }
                return $0.key < $1.key
            }

        for (index, entry) in rankedExtensions.enumerated() {
            let fileExtension = entry.key
            let count = entry.value
            let applicationName = index < associationLookupLimit
                ? cachedApplicationName(for: fileExtension)
                : "Other"
            grouped[applicationName, default: ([], 0)].extensions.insert(fileExtension)
            grouped[applicationName, default: ([], 0)].count += count
        }
        return [.all] + grouped.map { name, value in
            ApplicationGroup(id: name, name: name, extensions: value.extensions, itemCount: value.count)
        }.sorted {
            if $0.itemCount != $1.itemCount { return $0.itemCount > $1.itemCount }
            return $0.name.localizedStandardCompare($1.name) == .orderedAscending
        }
    }

    private static func cachedApplicationName(for fileExtension: String) -> String {
        if let cached = applicationNameCache[fileExtension] { return cached }
        let value = defaultApplicationName(for: fileExtension)
        applicationNameCache[fileExtension] = value
        return value
    }

    private static func defaultApplicationName(for fileExtension: String) -> String {
        let sample = URL(fileURLWithPath: NSTemporaryDirectory())
            .appending(path: "MacHunt-Association.\(fileExtension)")
        guard let applicationURL = NSWorkspace.shared.urlForApplication(toOpen: sample) else {
            return "Other"
        }
        return FileManager.default.displayName(atPath: applicationURL.path)
            .replacingOccurrences(of: ".app", with: "")
    }
}
