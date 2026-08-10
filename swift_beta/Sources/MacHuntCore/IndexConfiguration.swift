import Foundation

public struct IndexConfiguration: Codable, Equatable, Sendable {
    public var roots: [String]
    public var excludedDirectories: [String]
    public var excludedDirectoryPatterns: [String]
    public var excludeHiddenItems: Bool
    public var excludedFilePatterns: [String]
    public var indexExternalVolumes: Bool

    public init(
        roots: [String],
        excludedDirectories: [String] = [],
        excludedDirectoryPatterns: [String] = [
            "/System/**", "/private/var/**", "/private/tmp/**", "/.Spotlight-V100/**",
            "/.fseventsd/**", "/dev/**", "/proc/**", "*/Library/Caches/*", "*/.Trash/*",
        ],
        excludeHiddenItems: Bool = false,
        excludedFilePatterns: [String] = [".DS_Store", "*.tmp"],
        indexExternalVolumes: Bool = true
    ) {
        self.roots = roots
        self.excludedDirectories = excludedDirectories
        self.excludedDirectoryPatterns = excludedDirectoryPatterns
        self.excludeHiddenItems = excludeHiddenItems
        self.excludedFilePatterns = excludedFilePatterns
        self.indexExternalVolumes = indexExternalVolumes
    }

    public func includes(path: String, isDirectory: Bool, isHidden: Bool) -> Bool {
        let standardized = URL(fileURLWithPath: path).standardizedFileURL.path
        guard roots.isEmpty || roots.contains(where: { Self.contains(standardized, root: $0) }) else {
            return false
        }
        if excludeHiddenItems && isHidden { return false }
        if excludedDirectories.contains(where: { standardized == $0 || standardized.hasPrefix($0 + "/") }) {
            return false
        }
        if excludedDirectoryPatterns.contains(where: { Self.glob($0, matches: standardized) }) {
            return false
        }
        if !isDirectory {
            let name = URL(fileURLWithPath: standardized).lastPathComponent
            if excludedFilePatterns.contains(where: { Self.glob($0, matches: name) }) { return false }
        }
        return true
    }

    public var signature: String {
        var normalized = self
        normalized.roots = roots.map { URL(fileURLWithPath: $0).standardizedFileURL.path }.sorted()
        normalized.excludedDirectories = excludedDirectories.map {
            URL(fileURLWithPath: $0).standardizedFileURL.path
        }.sorted()
        normalized.excludedDirectoryPatterns.sort()
        normalized.excludedFilePatterns.sort()
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return (try? encoder.encode(normalized)).flatMap { String(data: $0, encoding: .utf8) } ?? ""
    }

    private static func contains(_ path: String, root: String) -> Bool {
        let normalizedRoot = URL(fileURLWithPath: root).standardizedFileURL.path
        if normalizedRoot == "/" { return path.hasPrefix("/") }
        return path == normalizedRoot || path.hasPrefix(normalizedRoot + "/")
    }

    private static func glob(_ pattern: String, matches value: String) -> Bool {
        let expression = "^" + NSRegularExpression.escapedPattern(for: pattern)
            .replacingOccurrences(of: "\\*\\*", with: ".*")
            .replacingOccurrences(of: "\\*", with: ".*")
            .replacingOccurrences(of: "\\?", with: ".") + "$"
        return value.range(of: expression, options: [.regularExpression, .caseInsensitive]) != nil
    }
}
