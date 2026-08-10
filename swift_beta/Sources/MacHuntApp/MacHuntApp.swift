import AppKit
import SwiftUI

@main
struct MacHuntApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var preferences: AppPreferences
    @StateObject private var searchStore: SearchStore
    @StateObject private var pinnedStore = PinnedStore()

    init() {
        let preferences = AppPreferences()
        _preferences = StateObject(wrappedValue: preferences)
        _searchStore = StateObject(wrappedValue: SearchStore(preferences: preferences))
    }

    var body: some Scene {
        WindowGroup("MacHunt") {
            RootView()
                .environmentObject(searchStore)
                .environmentObject(pinnedStore)
                .environmentObject(preferences)
                .environment(\.locale, preferences.language.localeIdentifier.map(Locale.init(identifier:)) ?? .autoupdatingCurrent)
                .frame(minWidth: 980, minHeight: 660)
                .onAppear {
                    SystemIntegrationController.shared.configure(
                        preferences: preferences,
                        searchStore: searchStore
                    )
                }
        }
        .defaultSize(width: 1320, height: 860)
        .windowToolbarStyle(.unified)
        .commands {
            CommandMenu("Search") {
                Button("Show Search") {
                    SystemIntegrationController.shared.showMainWindow(section: .search)
                }
                .keyboardShortcut("1", modifiers: .command)

                Button("Show Pinned") {
                    SystemIntegrationController.shared.showMainWindow(section: .pinned)
                }
                .keyboardShortcut("2", modifiers: .command)

                Divider()

                Button("Open Selected") {
                    if let first = searchStore.selectedResults.first { FileActions.open(first) }
                }
                .disabled(searchStore.selectedResults.isEmpty)

                Button("Quick Look Selected") {
                    QuickLookController.shared.toggle(items: searchStore.selectedResults)
                }
                .disabled(searchStore.selectedResults.isEmpty)

                Button("Reveal Selected in Finder") {
                    FileActions.reveal(searchStore.selectedResults)
                }
                .disabled(searchStore.selectedResults.isEmpty)

                Button(allSelectedResultsArePinned ? "Unpin Selected" : "Pin Selected") {
                    togglePinnedSelectedResults()
                }
                .keyboardShortcut("p", modifiers: [.command, .shift])
                .disabled(searchStore.selectedResults.isEmpty)

                Divider()

                Button(preferences.monitoringPaused ? "Resume Index Monitoring" : "Pause Index Monitoring") {
                    searchStore.setMonitoringPaused(!preferences.monitoringPaused)
                }

                Button("Rebuild Index") {
                    searchStore.rebuildConfiguredIndex()
                }
                .keyboardShortcut("r", modifiers: [.command, .shift])
                .disabled(searchStore.isBuilding)
            }
        }

        Settings {
            SettingsScreen()
                .environmentObject(searchStore)
                .environmentObject(preferences)
                .frame(width: 680, height: 680)
        }
    }

    private var allSelectedResultsArePinned: Bool {
        !searchStore.selectedResults.isEmpty && searchStore.selectedResults.allSatisfy(pinnedStore.contains)
    }

    private func togglePinnedSelectedResults() {
        let selected = searchStore.selectedResults
        if selected.allSatisfy(pinnedStore.contains) {
            selected.forEach(pinnedStore.removeIfPresent)
        } else {
            selected.filter { !pinnedStore.contains($0) }.forEach(pinnedStore.toggle)
        }
    }
}
