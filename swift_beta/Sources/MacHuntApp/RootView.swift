import SwiftUI

enum AppSection: String, CaseIterable, Identifiable, Hashable {
    case search, pinned
    var id: String { rawValue }

    var title: String {
        switch self {
        case .search: "Search"
        case .pinned: "Pinned"
        }
    }

    var icon: String {
        switch self {
        case .search: "magnifyingglass"
        case .pinned: "star"
        }
    }
}

struct RootView: View {
    @EnvironmentObject private var searchStore: SearchStore
    @EnvironmentObject private var pinnedStore: PinnedStore
    @State private var section: AppSection? = .search

    var body: some View {
        NavigationSplitView {
            List(AppSection.allCases, selection: $section) { item in
                NavigationLink(value: item) {
                    Label(item.title, systemImage: item.icon)
                }
                .keyboardShortcut(shortcut(for: item), modifiers: .command)
            }
            .navigationTitle("MacHunt")
            .listStyle(.sidebar)
            .safeAreaInset(edge: .bottom) {
                VStack(alignment: .leading, spacing: 4) {
                    if searchStore.isBuilding {
                        ProgressView().controlSize(.small)
                    }
                    Text(searchStore.status)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(3)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            }
        } detail: {
            Group {
                switch section ?? .search {
                case .search:
                    SearchScreen()
                        .navigationTitle("Search")
                case .pinned:
                    PinnedScreen()
                        .navigationTitle("Pinned")
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        }
        .navigationSplitViewStyle(.balanced)
        .onReceive(NotificationCenter.default.publisher(for: .macHuntNavigate)) { notification in
            guard let rawValue = notification.object as? String,
                  let destination = AppSection(rawValue: rawValue) else { return }
            section = destination
        }
    }

    private func shortcut(for section: AppSection) -> KeyEquivalent {
        switch section {
        case .search: "1"
        case .pinned: "2"
        }
    }
}
