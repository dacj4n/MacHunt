import Foundation

public struct SearchResult: Identifiable, Codable, Hashable, Sendable {
    public var id: String { path }

    public let name: String
    public let path: String
    public let parent: String
    public let isDirectory: Bool
    public let sizeBytes: Int64?
    public let modifiedAt: Date?
    public let addedAt: Date?
    public let cloudStatus: CloudFileStatus?
    public let tags: [String]

    public init(
        name: String,
        path: String,
        parent: String,
        isDirectory: Bool,
        sizeBytes: Int64?,
        modifiedAt: Date?,
        addedAt: Date? = nil,
        cloudStatus: CloudFileStatus? = nil,
        tags: [String] = []
    ) {
        self.name = name
        self.path = path
        self.parent = parent
        self.isDirectory = isDirectory
        self.sizeBytes = sizeBytes
        self.modifiedAt = modifiedAt
        self.addedAt = addedAt
        self.cloudStatus = cloudStatus
        self.tags = tags
    }

    private enum CodingKeys: String, CodingKey {
        case name, path, parent, isDirectory, sizeBytes, modifiedAt, addedAt, cloudStatus, tags
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        name = try values.decode(String.self, forKey: .name)
        path = try values.decode(String.self, forKey: .path)
        parent = try values.decode(String.self, forKey: .parent)
        isDirectory = try values.decode(Bool.self, forKey: .isDirectory)
        sizeBytes = try values.decodeIfPresent(Int64.self, forKey: .sizeBytes)
        modifiedAt = try values.decodeIfPresent(Date.self, forKey: .modifiedAt)
        addedAt = try values.decodeIfPresent(Date.self, forKey: .addedAt)
        cloudStatus = try values.decodeIfPresent(CloudFileStatus.self, forKey: .cloudStatus)
        tags = try values.decodeIfPresent([String].self, forKey: .tags) ?? []
    }
}

public enum CloudFileStatus: String, Codable, Hashable, Sendable {
    case inCloud
    case downloading
    case downloaded
    case uploading
    case synced
    case conflict
}

public enum SearchMode: String, Codable, CaseIterable, Sendable {
    case substring
    case pattern
    case fuzzy
}

public enum ResultCategory: String, Codable, CaseIterable, Identifiable, Sendable {
    case all, files, folders, documents, images, media, code, archives, custom

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .all: "All"
        case .files: "Files"
        case .folders: "Folders"
        case .documents: "Documents"
        case .images: "Images"
        case .media: "Media"
        case .code: "Code"
        case .archives: "Archives"
        case .custom: "Custom"
        }
    }

    public var systemImage: String {
        switch self {
        case .all: "square.grid.2x2"
        case .files: "doc"
        case .folders: "folder"
        case .documents: "doc.text"
        case .images: "photo"
        case .media: "play.rectangle"
        case .code: "chevron.left.forwardslash.chevron.right"
        case .archives: "archivebox"
        case .custom: "slider.horizontal.3"
        }
    }

    public var defaultExtensions: Set<String>? {
        switch self {
        case .all, .files, .folders, .custom: nil
        case .documents: ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "rtf", "pages", "numbers", "key"]
        case .images: ["png", "jpg", "jpeg", "gif", "webp", "svg", "heic", "bmp", "tif", "tiff", "raw", "dng", "avif"]
        case .media: ["mp3", "m4a", "wav", "flac", "aac", "ogg", "mp4", "mov", "avi", "mkv", "webm", "aiff"]
        case .code: ["rs", "swift", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml", "py", "go", "java", "c", "cpp", "h", "hpp", "html", "css", "sql", "sh", "zsh"]
        case .archives: ["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "tgz", "dmg", "iso"]
        }
    }

    public var supportsEditableExtensions: Bool {
        switch self {
        case .documents, .images, .media, .code, .archives, .custom: true
        default: false
        }
    }

    public static var editableCases: [ResultCategory] {
        [.documents, .images, .media, .code, .archives]
    }

    func effectiveExtensions(override: Set<String>?) -> Set<String>? {
        guard supportsEditableExtensions else { return nil }
        if let override { return override }
        return self == .custom ? [] : defaultExtensions
    }

    func includes(_ item: SearchResult, categoryExtensions: Set<String>?) -> Bool {
        switch self {
        case .all: return true
        case .files: return !item.isDirectory
        case .folders: return item.isDirectory
        default:
            guard !item.isDirectory, let extensions = effectiveExtensions(override: categoryExtensions) else { return false }
            return extensions.contains(URL(fileURLWithPath: item.name).pathExtension.lowercased())
        }
    }
}

public enum SearchSort: String, Codable, CaseIterable, Sendable {
    case name, path, type, size, modified, added, cloudStatus, tags
}

public enum ApplicationFilter: String, Codable, CaseIterable, Identifiable, Sendable {
    case all, preview, office, xcode, visualStudioCode, media

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .all: "All Applications"
        case .preview: "Preview"
        case .office: "Office"
        case .xcode: "Xcode"
        case .visualStudioCode: "Visual Studio Code"
        case .media: "Music & Video"
        }
    }

    var extensions: Set<String>? {
        switch self {
        case .all: nil
        case .preview: ["pdf", "png", "jpg", "jpeg", "gif", "heic", "tif", "tiff", "svg", "webp"]
        case .office: ["doc", "docx", "xls", "xlsx", "ppt", "pptx", "pages", "numbers", "key", "csv"]
        case .xcode: ["swift", "m", "mm", "c", "cpp", "h", "hpp", "xcodeproj", "xcworkspace", "plist"]
        case .visualStudioCode: ["rs", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml", "py", "go", "java", "rb", "php", "sql", "css", "html"]
        case .media: ["mp3", "m4a", "wav", "flac", "aac", "mp4", "mov", "avi", "mkv", "webm"]
        }
    }

    func includes(_ item: SearchResult) -> Bool {
        guard let extensions else { return true }
        guard !item.isDirectory else { return false }
        return extensions.contains(URL(fileURLWithPath: item.name).pathExtension.lowercased())
    }
}

public struct SearchRequest: Sendable {
    public var query: String
    public var mode: SearchMode
    public var caseSensitive: Bool
    public var pathPrefix: String?
    public var category: ResultCategory
    public var application: ApplicationFilter
    public var applicationExtensions: Set<String>?
    public var categoryExtensions: Set<String>?
    public var minimumSize: Int64?
    public var maximumSize: Int64?
    public var modifiedAfter: Date?
    public var modifiedBefore: Date?
    public var sort: SearchSort
    public var ascending: Bool
    public var limit: Int

    public init(
        query: String = "",
        mode: SearchMode = .substring,
        caseSensitive: Bool = false,
        pathPrefix: String? = nil,
        category: ResultCategory = .all,
        application: ApplicationFilter = .all,
        applicationExtensions: Set<String>? = nil,
        categoryExtensions: Set<String>? = nil,
        minimumSize: Int64? = nil,
        maximumSize: Int64? = nil,
        modifiedAfter: Date? = nil,
        modifiedBefore: Date? = nil,
        sort: SearchSort = .name,
        ascending: Bool = true,
        limit: Int = 500
    ) {
        self.query = query
        self.mode = mode
        self.caseSensitive = caseSensitive
        self.pathPrefix = pathPrefix
        self.category = category
        self.application = application
        self.applicationExtensions = applicationExtensions
        self.categoryExtensions = categoryExtensions
        self.minimumSize = minimumSize
        self.maximumSize = maximumSize
        self.modifiedAfter = modifiedAfter
        self.modifiedBefore = modifiedBefore
        self.sort = sort
        self.ascending = ascending
        self.limit = limit
    }
}

public struct IndexSummary: Sendable {
    public let itemCount: Int
    public let elapsed: Duration

    public init(itemCount: Int, elapsed: Duration) {
        self.itemCount = itemCount
        self.elapsed = elapsed
    }
}

public enum IndexBuildPhase: String, Sendable {
    case scanning
    case buildingSearchIndex
    case optimizing
    case complete
}

public struct IndexBuildProgress: Sendable {
    public let phase: IndexBuildPhase
    public let itemCount: Int

    public init(phase: IndexBuildPhase, itemCount: Int) {
        self.phase = phase
        self.itemCount = itemCount
    }
}
