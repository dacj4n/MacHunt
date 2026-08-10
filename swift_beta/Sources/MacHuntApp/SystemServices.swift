import AppKit
import Foundation
import MacHuntCore
@preconcurrency import QuickLookUI

@MainActor
enum FileActions {
    static func open(_ item: SearchResult) {
        NSWorkspace.shared.open(URL(fileURLWithPath: item.path))
    }

    static func reveal(_ items: [SearchResult]) {
        NSWorkspace.shared.activateFileViewerSelecting(items.map { URL(fileURLWithPath: $0.path) })
    }

    static func copyPaths(_ items: [SearchResult]) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(items.map(\.path).joined(separator: "\n"), forType: .string)
    }

    static func copyNames(_ items: [SearchResult]) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(items.map(\.name).joined(separator: "\n"), forType: .string)
    }

    static func copyFileObjects(_ items: [SearchResult]) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.writeObjects(items.map { URL(fileURLWithPath: $0.path) as NSURL })
    }

    static func applications(for item: SearchResult) -> [URL] {
        let target = URL(fileURLWithPath: item.path)
        return NSWorkspace.shared.urlsForApplications(toOpen: target)
            .reduce(into: [String: URL]()) { result, url in result[url.path] = url }
            .values
            .sorted {
                FileManager.default.displayName(atPath: $0.path)
                    .localizedStandardCompare(FileManager.default.displayName(atPath: $1.path)) == .orderedAscending
            }
    }

    static func open(_ items: [SearchResult], with applicationURL: URL) {
        guard !items.isEmpty else { return }
        NSWorkspace.shared.open(
            items.map { URL(fileURLWithPath: $0.path) },
            withApplicationAt: applicationURL,
            configuration: NSWorkspace.OpenConfiguration()
        )
    }

    static func openInTerminal(_ items: [SearchResult]) {
        guard let terminal = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.Terminal") else {
            NSSound.beep()
            return
        }
        let targets = items.map { item in
            let url = URL(fileURLWithPath: item.path)
            return item.isDirectory ? url : url.deletingLastPathComponent()
        }
        NSWorkspace.shared.open(
            targets,
            withApplicationAt: terminal,
            configuration: NSWorkspace.OpenConfiguration()
        )
    }

    static func moveToTrash(
        _ items: [SearchResult],
        completion: @escaping @MainActor @Sendable (Bool) -> Void = { _ in }
    ) {
        NSWorkspace.shared.recycle(items.map { URL(fileURLWithPath: $0.path) }) { mappings, error in
            if error != nil { NSSound.beep() }
            Task { @MainActor in
                if error == nil, !mappings.isEmpty,
                   let undoManager = NSApp.keyWindow?.undoManager ?? NSApp.mainWindow?.undoManager {
                    undoManager.registerUndo(withTarget: TrashUndoController.shared) { target in
                        target.restore(mappings)
                    }
                    undoManager.setActionName("Move to Trash")
                }
                completion(error == nil)
            }
        }
    }
}

@MainActor
private final class TrashUndoController: NSObject {
    static let shared = TrashUndoController()

    func restore(_ mappings: [URL: URL]) {
        for (original, trashed) in mappings {
            do {
                try FileManager.default.createDirectory(
                    at: original.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                try FileManager.default.moveItem(at: trashed, to: original)
            } catch {
                NSSound.beep()
            }
        }
    }
}

@MainActor
final class QuickLookController: NSObject {
    static let shared = QuickLookController()
    private var urls: [NSURL] = []

    func toggle(items: [SearchResult]) {
        guard !items.isEmpty else { return }
        guard let panel = QLPreviewPanel.shared() else { return }
        if panel.isVisible {
            panel.orderOut(nil)
            return
        }
        urls = items.map { URL(fileURLWithPath: $0.path) as NSURL }
        panel.dataSource = self
        panel.delegate = self
        panel.currentPreviewItemIndex = 0
        panel.reloadData()
        panel.makeKeyAndOrderFront(nil)
    }

    func refresh(items: [SearchResult]) {
        guard let panel = QLPreviewPanel.shared(), panel.isVisible else { return }
        urls = items.map { URL(fileURLWithPath: $0.path) as NSURL }
        panel.reloadData()
    }

}

extension QuickLookController: @MainActor QLPreviewPanelDataSource, @MainActor QLPreviewPanelDelegate {
    func numberOfPreviewItems(in panel: QLPreviewPanel!) -> Int { urls.count }

    func previewPanel(_ panel: QLPreviewPanel!, previewItemAt index: Int) -> (any QLPreviewItem)! {
        guard urls.indices.contains(index) else { return nil }
        return urls[index]
    }
}
