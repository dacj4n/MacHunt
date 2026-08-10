import SwiftUI

enum ResultTableColumn: String, CaseIterable, Identifiable {
    case pin, name, path, type, size, modified, added, cloudStatus, tags

    var id: String { rawValue }

    var title: String {
        switch self {
        case .pin: "Pinned"
        case .name: "Name"
        case .path: "Path"
        case .type: "Type"
        case .size: "Size"
        case .modified: "Modified"
        case .added: "Date Added"
        case .cloudStatus: "Cloud Status"
        case .tags: "Tags"
        }
    }

    var systemImage: String {
        switch self {
        case .pin: "star"
        case .name: "doc"
        case .path: "folder"
        case .type: "tag"
        case .size: "internaldrive"
        case .modified: "calendar"
        case .added: "calendar.badge.plus"
        case .cloudStatus: "icloud"
        case .tags: "tag"
        }
    }

    var isRequired: Bool { self == .name }

    static let defaultIDs = [pin, name, path, type, size, modified].map(\.rawValue)
}
