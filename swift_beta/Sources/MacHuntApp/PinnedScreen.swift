import MacHuntCore
import SwiftUI

struct PinnedScreen: View {
    @EnvironmentObject private var pinned: PinnedStore
    @EnvironmentObject private var searchStore: SearchStore
    @EnvironmentObject private var preferences: AppPreferences
    @State private var selection: Set<SearchResult.ID> = []
    @State private var sort: SearchSort = .name
    @State private var ascending = true
    @State private var pendingTrash: [SearchResult] = []
    @State private var confirmTrash = false

    private var sortedItems: [SearchResult] {
        pinned.items.sorted { first, second in
            let comparison: ComparisonResult = switch sort {
            case .name: first.name.localizedStandardCompare(second.name)
            case .path: first.parent.localizedStandardCompare(second.parent)
            case .type:
                firstType(first).localizedStandardCompare(firstType(second))
            case .size:
                compare(first.sizeBytes ?? -1, second.sizeBytes ?? -1)
            case .modified:
                compare(first.modifiedAt ?? .distantPast, second.modifiedAt ?? .distantPast)
            case .added:
                compare(first.addedAt ?? .distantPast, second.addedAt ?? .distantPast)
            case .cloudStatus:
                (first.cloudStatus?.rawValue ?? "").localizedStandardCompare(
                    second.cloudStatus?.rawValue ?? ""
                )
            case .tags:
                first.tags.joined(separator: ", ").localizedStandardCompare(
                    second.tags.joined(separator: ", ")
                )
            }
            return ascending ? comparison == .orderedAscending : comparison == .orderedDescending
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Pinned").font(.largeTitle.bold())
                Text("Favorite files and folders are stored locally.").foregroundStyle(.secondary)
            }

            if pinned.items.isEmpty {
                ContentUnavailableView(
                    "No Pinned Items",
                    systemImage: "star",
                    description: Text("Pin a search result to keep it here.")
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                NativeResultsTable(
                    items: sortedItems,
                    selection: $selection,
                    autosaveName: "MacHuntPinnedResultsTable",
                    isPinned: pinned.contains,
                    onTogglePin: pinned.toggle,
                    onOpen: FileActions.open,
                    onQuickLook: { QuickLookController.shared.toggle(items: $0) },
                    onReveal: FileActions.reveal,
                    onCopyPaths: FileActions.copyPaths,
                    onCopyFiles: FileActions.copyFileObjects,
                    onTrash: requestTrash,
                    onSort: { sort, ascending in
                        self.sort = sort
                        self.ascending = ascending
                    },
                    sort: sort,
                    ascending: ascending,
                    visibleColumnIDs: Set(preferences.visibleResultColumns),
                    onToggleColumn: { columnID in
                        guard let column = ResultTableColumn(rawValue: columnID) else { return }
                        preferences.setResultColumn(
                            column,
                            visible: !preferences.isResultColumnVisible(column)
                        )
                    }
                )

                HStack {
                    Text("\(pinned.items.count.formatted()) pinned items")
                    Spacer()
                    if !selection.isEmpty { Text("\(selection.count) selected") }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 28)
        .padding(.bottom, 18)
        .onAppear { pinned.refreshLocations() }
        .alert("Move selected items to Trash?", isPresented: $confirmTrash) {
            Button("Move to Trash", role: .destructive) { trash(pendingTrash) }
            Button("Cancel", role: .cancel) { pendingTrash = [] }
        } message: {
            Text("This moves \(pendingTrash.count) item(s) to the Trash.")
        }
    }

    private func requestTrash(_ items: [SearchResult]) {
        guard !items.isEmpty else { return }
        if preferences.confirmBeforeTrash {
            pendingTrash = items
            confirmTrash = true
        } else {
            trash(items)
        }
    }

    private func trash(_ items: [SearchResult]) {
        FileActions.moveToTrash(items) { success in
            guard success else { return }
            searchStore.removeFromIndex(items)
            items.forEach(pinned.removeIfPresent)
        }
        pendingTrash = []
    }

    private func firstType(_ item: SearchResult) -> String {
        item.isDirectory ? "" : URL(fileURLWithPath: item.name).pathExtension.lowercased()
    }

    private func compare<T: Comparable>(_ first: T, _ second: T) -> ComparisonResult {
        if first < second { return .orderedAscending }
        if first > second { return .orderedDescending }
        return .orderedSame
    }
}
