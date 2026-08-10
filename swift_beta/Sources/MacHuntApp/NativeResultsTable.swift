import AppKit
import MacHuntCore
import SwiftUI

struct NativeResultsTable: NSViewRepresentable {
    let items: [SearchResult]
    @Binding var selection: Set<SearchResult.ID>
    let autosaveName: String
    let isPinned: (SearchResult) -> Bool
    let onTogglePin: (SearchResult) -> Void
    let onOpen: (SearchResult) -> Void
    let onQuickLook: ([SearchResult]) -> Void
    let onReveal: ([SearchResult]) -> Void
    let onCopyPaths: ([SearchResult]) -> Void
    let onCopyFiles: ([SearchResult]) -> Void
    let onTrash: ([SearchResult]) -> Void
    let onSort: (SearchSort, Bool) -> Void
    let sort: SearchSort
    let ascending: Bool
    let visibleColumnIDs: Set<String>
    let onToggleColumn: (String) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSScrollView {
        let table = KeyboardTableView()
        table.delegate = context.coordinator
        table.dataSource = context.coordinator
        table.allowsMultipleSelection = true
        table.allowsEmptySelection = true
        table.allowsColumnReordering = true
        table.allowsColumnResizing = true
        table.usesAlternatingRowBackgroundColors = true
        table.rowHeight = 24
        table.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle
        table.autosaveName = autosaveName
        table.autosaveTableColumns = true
        table.doubleAction = #selector(Coordinator.openSelected)
        table.target = context.coordinator
        table.menu = NSMenu()
        table.menu?.delegate = context.coordinator
        table.onSpace = { [weak coordinator = context.coordinator] in coordinator?.quickLookSelected() }
        table.onReturn = { [weak coordinator = context.coordinator] in coordinator?.openSelected() }
        table.onDelete = { [weak coordinator = context.coordinator] in coordinator?.trashSelected() }

        addColumn("pin", title: "", width: 30, min: 30, max: 30, to: table)
        addColumn("name", title: "Name", width: 330, min: 180, sort: .name, to: table)
        addColumn("path", title: "Path", width: 330, min: 160, sort: .path, to: table)
        addColumn("type", title: "Type", width: 90, min: 65, sort: .type, to: table)
        addColumn("size", title: "Size", width: 100, min: 70, sort: .size, to: table)
        addColumn("modified", title: "Modified", width: 170, min: 120, sort: .modified, to: table)
        addColumn("added", title: "Date Added", width: 170, min: 120, sort: .added, to: table)
        addColumn("cloudStatus", title: "Cloud Status", width: 130, min: 100, sort: .cloudStatus, to: table)
        addColumn("tags", title: "Tags", width: 180, min: 100, sort: .tags, to: table)

        context.coordinator.captureColumns(from: table)
        context.coordinator.updateColumnVisibility(in: table)
        let headerMenu = NSMenu(title: "Columns")
        headerMenu.delegate = context.coordinator
        table.headerView?.menu = headerMenu
        context.coordinator.headerMenu = headerMenu

        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.autohidesScrollers = true
        return scroll
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let table = scrollView.documentView as? NSTableView else { return }
        let coordinator = context.coordinator
        let pinnedIDs = Set(items.lazy.filter(isPinned).map(\.id))
        let rowsChanged = coordinator.lastItems != items || coordinator.lastPinnedIDs != pinnedIDs
        coordinator.parent = self
        coordinator.updateColumnVisibility(in: table)
        if rowsChanged {
            coordinator.lastItems = items
            coordinator.lastPinnedIDs = pinnedIDs
            table.reloadData()
        }
        let indexes = IndexSet(items.indices.filter { selection.contains(items[$0].id) })
        if table.selectedRowIndexes != indexes {
            table.selectRowIndexes(indexes, byExtendingSelection: false)
        }
        let key = sort.rawValue
        if table.sortDescriptors.first?.key != key || table.sortDescriptors.first?.ascending != ascending {
            table.sortDescriptors = [NSSortDescriptor(key: key, ascending: ascending)]
        }
    }

    private func addColumn(
        _ identifier: String,
        title: String,
        width: CGFloat,
        min: CGFloat,
        max: CGFloat = 10_000,
        sort: SearchSort? = nil,
        to table: NSTableView
    ) {
        let column = NSTableColumn(identifier: .init(identifier))
        column.title = title
        column.width = width
        column.minWidth = min
        column.maxWidth = max
        if let sort { column.sortDescriptorPrototype = NSSortDescriptor(key: sort.rawValue, ascending: true) }
        table.addTableColumn(column)
    }

    static func dismantleNSView(_ nsView: NSScrollView, coordinator: Coordinator) {
        guard let table = nsView.documentView as? KeyboardTableView else { return }
        table.delegate = nil
        table.dataSource = nil
        table.menu?.delegate = nil
        table.onSpace = nil
        table.onReturn = nil
        table.onDelete = nil
    }

    @MainActor
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate, NSMenuDelegate {
        var parent: NativeResultsTable
        private weak var tableView: NSTableView?
        weak var headerMenu: NSMenu?
        var lastItems: [SearchResult] = []
        var lastPinnedIDs: Set<SearchResult.ID> = []
        private var availableColumns: [String: NSTableColumn] = [:]
        private let iconCache = NSCache<NSString, NSImage>()

        init(_ parent: NativeResultsTable) { self.parent = parent }

        func captureColumns(from table: NSTableView) {
            availableColumns = Dictionary(uniqueKeysWithValues: table.tableColumns.map {
                ($0.identifier.rawValue, $0)
            })
        }

        func updateColumnVisibility(in table: NSTableView) {
            let visible = parent.visibleColumnIDs.union([ResultTableColumn.name.rawValue])
            for column in table.tableColumns where !visible.contains(column.identifier.rawValue) {
                table.removeTableColumn(column)
            }
            let ordered = ResultTableColumn.allCases.map(\.rawValue).filter(visible.contains)
            for (targetIndex, identifier) in ordered.enumerated() {
                guard let column = availableColumns[identifier] else { continue }
                guard !table.tableColumns.contains(where: { $0 === column }) else { continue }
                table.addTableColumn(column)
                let currentIndex = table.tableColumns.count - 1
                let destination = min(targetIndex, currentIndex)
                if currentIndex != destination {
                    table.moveColumn(currentIndex, toColumn: destination)
                }
            }
        }

        func numberOfRows(in tableView: NSTableView) -> Int {
            self.tableView = tableView
            return parent.items.count
        }

        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
            guard parent.items.indices.contains(row), let identifier = tableColumn?.identifier else { return nil }
            let item = parent.items[row]
            if identifier.rawValue == "pin" {
                let button = tableView.makeView(withIdentifier: identifier, owner: nil) as? NSButton
                    ?? NSButton(image: NSImage(), target: self, action: #selector(togglePin(_:)))
                button.identifier = identifier
                button.tag = row
                button.isBordered = false
                button.image = NSImage(
                    systemSymbolName: parent.isPinned(item) ? "star.fill" : "star",
                    accessibilityDescription: parent.isPinned(item) ? "Unpin" : "Pin"
                )
                button.contentTintColor = parent.isPinned(item) ? .systemYellow : .secondaryLabelColor
                return button
            }

            let cell = tableView.makeView(withIdentifier: identifier, owner: nil) as? NSTableCellView
                ?? makeCell(identifier: identifier, includesIcon: identifier.rawValue == "name")
            cell.textField?.stringValue = value(for: identifier.rawValue, item: item)
            cell.textField?.toolTip = identifier.rawValue == "path" ? item.parent : nil
            if identifier.rawValue == "name" {
                let cacheKey = item.path as NSString
                if let cached = iconCache.object(forKey: cacheKey) {
                    cell.imageView?.image = cached
                } else {
                    let icon = NSWorkspace.shared.icon(forFile: item.path)
                    iconCache.setObject(icon, forKey: cacheKey)
                    cell.imageView?.image = icon
                }
            }
            return cell
        }

        func tableViewSelectionDidChange(_ notification: Notification) {
            guard let table = notification.object as? NSTableView else { return }
            parent.selection = Set(table.selectedRowIndexes.compactMap { index in
                parent.items.indices.contains(index) ? parent.items[index].id : nil
            })
            QuickLookController.shared.refresh(items: targets)
        }

        func tableView(_ tableView: NSTableView, sortDescriptorsDidChange oldDescriptors: [NSSortDescriptor]) {
            guard let descriptor = tableView.sortDescriptors.first,
                  let key = descriptor.key,
                  let sort = SearchSort(rawValue: key) else { return }
            parent.onSort(sort, descriptor.ascending)
        }

        func menuNeedsUpdate(_ menu: NSMenu) {
            menu.removeAllItems()
            if menu === headerMenu {
                for column in ResultTableColumn.allCases {
                    let item = add(
                        NSLocalizedString(column.title, comment: "Results table column"),
                        #selector(toggleColumn(_:)),
                        to: menu
                    )
                    item.representedObject = column.rawValue
                    item.image = NSImage(systemSymbolName: column.systemImage, accessibilityDescription: nil)
                    item.state = parent.visibleColumnIDs.contains(column.rawValue) || column.isRequired ? .on : .off
                    item.isEnabled = !column.isRequired
                }
                return
            }
            guard !targets.isEmpty else { return }
            add(localized("Open"), #selector(openSelected), to: menu)
            let openWithItem = NSMenuItem(title: localized("Open With"), action: nil, keyEquivalent: "")
            let openWithMenu = NSMenu(title: localized("Open With"))
            add(localized("Finder"), #selector(revealSelected), to: openWithMenu)
            add(localized("Terminal"), #selector(openInTerminal), to: openWithMenu)
            let applications = parent.items.indices.contains(tableView?.clickedRow ?? -1)
                ? FileActions.applications(for: parent.items[tableView?.clickedRow ?? 0])
                : targets.first.map(FileActions.applications) ?? []
            if !applications.isEmpty { openWithMenu.addItem(.separator()) }
            for application in applications {
                let title = FileManager.default.displayName(atPath: application.path)
                    .replacingOccurrences(of: ".app", with: "")
                let item = add(title, #selector(openWithApplication(_:)), to: openWithMenu)
                item.representedObject = application
                item.image = NSWorkspace.shared.icon(forFile: application.path)
                item.image?.size = NSSize(width: 16, height: 16)
            }
            openWithItem.submenu = openWithMenu
            menu.addItem(openWithItem)
            add(localized("Quick Look"), #selector(quickLookSelected), to: menu)
            add(localized("Reveal in Finder"), #selector(revealSelected), to: menu)
            menu.addItem(.separator())
            add(localized(targets.count > 1 ? "Copy Names" : "Copy Name"), #selector(copyNames), to: menu)
            add(localized(targets.count > 1 ? "Copy Paths" : "Copy Path"), #selector(copyPaths), to: menu)
            add(localized("Copy as File Objects"), #selector(copyFiles), to: menu)
            add(localized(parent.isPinned(targets[0]) ? "Unpin" : "Pin"), #selector(toggleTargetsPin), to: menu)
            menu.addItem(.separator())
            add(localized("Move to Trash"), #selector(trashSelected), to: menu).isAlternate = false
        }

        private var targets: [SearchResult] {
            guard let tableView else { return [] }
            var indexes = tableView.selectedRowIndexes
            if tableView.clickedRow >= 0, !indexes.contains(tableView.clickedRow) {
                indexes = IndexSet(integer: tableView.clickedRow)
            }
            return indexes.compactMap { parent.items.indices.contains($0) ? parent.items[$0] : nil }
        }

        @discardableResult
        private func add(_ title: String, _ action: Selector, to menu: NSMenu) -> NSMenuItem {
            let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
            item.target = self
            menu.addItem(item)
            return item
        }

        private func localized(_ key: String) -> String {
            NSLocalizedString(key, comment: "Results table context menu")
        }

        @objc func openSelected() { if let item = targets.first { parent.onOpen(item) } }
        @objc func quickLookSelected() { let value = targets; if !value.isEmpty { parent.onQuickLook(value) } }
        @objc func revealSelected() { let value = targets; if !value.isEmpty { parent.onReveal(value) } }
        @objc func openInTerminal() { FileActions.openInTerminal(targets) }
        @objc func openWithApplication(_ sender: NSMenuItem) {
            guard let application = sender.representedObject as? URL else { return }
            FileActions.open(targets, with: application)
        }
        @objc func copyNames() { let value = targets; if !value.isEmpty { FileActions.copyNames(value) } }
        @objc func copyPaths() { let value = targets; if !value.isEmpty { parent.onCopyPaths(value) } }
        @objc func copyFiles() { let value = targets; if !value.isEmpty { parent.onCopyFiles(value) } }
        @objc func trashSelected() { let value = targets; if !value.isEmpty { parent.onTrash(value) } }
        @objc func toggleTargetsPin() { targets.forEach(parent.onTogglePin) }
        @objc func toggleColumn(_ sender: NSMenuItem) {
            guard let identifier = sender.representedObject as? String else { return }
            parent.onToggleColumn(identifier)
        }
        @objc func togglePin(_ sender: NSButton) {
            guard parent.items.indices.contains(sender.tag) else { return }
            parent.onTogglePin(parent.items[sender.tag])
        }

        private func makeCell(identifier: NSUserInterfaceItemIdentifier, includesIcon: Bool) -> NSTableCellView {
            let cell = NSTableCellView()
            cell.identifier = identifier
            let text = NSTextField(labelWithString: "")
            text.lineBreakMode = .byTruncatingMiddle
            text.translatesAutoresizingMaskIntoConstraints = false
            cell.textField = text
            cell.addSubview(text)
            if includesIcon {
                let image = NSImageView()
                image.imageScaling = .scaleProportionallyDown
                image.translatesAutoresizingMaskIntoConstraints = false
                cell.imageView = image
                cell.addSubview(image)
                NSLayoutConstraint.activate([
                    image.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 4),
                    image.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
                    image.widthAnchor.constraint(equalToConstant: 18),
                    image.heightAnchor.constraint(equalToConstant: 18),
                    text.leadingAnchor.constraint(equalTo: image.trailingAnchor, constant: 6),
                ])
            } else {
                text.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 4).isActive = true
            }
            NSLayoutConstraint.activate([
                text.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -4),
                text.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            ])
            return cell
        }

        private func value(for column: String, item: SearchResult) -> String {
            switch column {
            case "name": item.name
            case "path": item.parent
            case "type": item.isDirectory ? "Folder" : URL(fileURLWithPath: item.name).pathExtension.uppercased()
            case "size": item.sizeBytes.map { ByteCountFormatter.string(fromByteCount: $0, countStyle: .file) } ?? "—"
            case "modified": item.modifiedAt?.formatted(date: .numeric, time: .shortened) ?? "—"
            case "added": item.addedAt?.formatted(date: .numeric, time: .shortened) ?? "—"
            case "cloudStatus": cloudStatusTitle(item.cloudStatus)
            case "tags": item.tags.joined(separator: ", ")
            default: ""
            }
        }

        private func cloudStatusTitle(_ status: CloudFileStatus?) -> String {
            guard let status else { return "—" }
            let key = switch status {
            case .inCloud: "In Cloud"
            case .downloading: "Downloading"
            case .downloaded: "Downloaded"
            case .uploading: "Uploading"
            case .synced: "Synced"
            case .conflict: "Conflict"
            }
            return NSLocalizedString(key, comment: "Cloud file status")
        }
    }
}

private final class KeyboardTableView: NSTableView {
    var onSpace: (() -> Void)?
    var onReturn: (() -> Void)?
    var onDelete: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 49: onSpace?()
        case 36, 76: onReturn?()
        case 51, 117: onDelete?()
        default: super.keyDown(with: event)
        }
    }
}
