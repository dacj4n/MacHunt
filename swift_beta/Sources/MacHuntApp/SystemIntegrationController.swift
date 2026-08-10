import AppKit
import Carbon
import ServiceManagement

@MainActor
final class SystemIntegrationController: NSObject {
    static let shared = SystemIntegrationController()

    private var preferences: AppPreferences?
    private weak var searchStore: SearchStore?
    private var statusItem: NSStatusItem?
    private var hotKey: EventHotKeyRef?
    private var hotKeyHandler: EventHandlerRef?
    private var preferenceObserver: NSObjectProtocol?
    private var lastLaunchAtLoginPreference: Bool?

    func configure(preferences: AppPreferences, searchStore: SearchStore) {
        self.preferences = preferences
        self.searchStore = searchStore
        if lastLaunchAtLoginPreference == nil {
            lastLaunchAtLoginPreference = preferences.launchAtLogin
        }
        if preferenceObserver == nil {
            preferenceObserver = NotificationCenter.default.addObserver(
                forName: .macHuntSystemPreferencesChanged,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self, let preferences = self.preferences else { return }
                    let loginSettingChanged = self.lastLaunchAtLoginPreference != preferences.launchAtLogin
                    self.lastLaunchAtLoginPreference = preferences.launchAtLogin
                    self.applyPreferences(updateLoginItem: loginSettingChanged)
                }
            }
        }
        applyPreferences(updateLoginItem: false)
    }

    func showMainWindow(section: AppSection = .search) {
        NotificationCenter.default.post(name: .macHuntNavigate, object: section.rawValue)
        NSApp.setActivationPolicy(preferences?.showDockIcon == false ? .accessory : .regular)
        NSApp.activate(ignoringOtherApps: true)
        if let window = NSApp.windows.first(where: { $0.canBecomeMain && $0.title != "Settings" }) {
            window.makeKeyAndOrderFront(nil)
        }
    }

    func toggleMainWindow() {
        if let window = NSApp.windows.first(where: { $0.canBecomeMain && $0.title != "Settings" }), window.isVisible {
            window.orderOut(nil)
        } else {
            showMainWindow()
        }
    }

    private func applyPreferences(updateLoginItem: Bool) {
        guard let preferences else { return }
        NSApp.setActivationPolicy(preferences.showDockIcon ? .regular : .accessory)
        configureStatusItem(visible: preferences.showMenuBarItem)
        configureHotKey(preferences.globalShortcut)
        if updateLoginItem { configureLoginItem(enabled: preferences.launchAtLogin) }
    }

    private func configureStatusItem(visible: Bool) {
        if !visible {
            if let statusItem { NSStatusBar.system.removeStatusItem(statusItem) }
            statusItem = nil
            return
        }
        if statusItem == nil {
            statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
            statusItem?.button?.image = NSImage(
                systemSymbolName: "bolt.fill",
                accessibilityDescription: "MacHunt"
            )
        }
        let menu = NSMenu()
        menu.addItem(menuItem("Show Search", action: #selector(showSearch), key: ""))
        menu.addItem(menuItem("Pinned", action: #selector(showPinned), key: ""))
        menu.addItem(menuItem("Settings…", action: #selector(showSettings), key: ","))
        menu.addItem(.separator())
        menu.addItem(menuItem(
            preferences?.monitoringPaused == true ? "Resume Index Monitoring" : "Pause Index Monitoring",
            action: #selector(toggleMonitoring),
            key: ""
        ))
        menu.addItem(menuItem("Rebuild Index", action: #selector(rebuildIndex), key: ""))
        menu.addItem(.separator())
        menu.addItem(menuItem("Quit MacHunt", action: #selector(quit), key: "q"))
        statusItem?.menu = menu
    }

    private func menuItem(_ title: String, action: Selector, key: String) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    private func configureHotKey(_ choice: GlobalShortcutChoice) {
        if let hotKey { UnregisterEventHotKey(hotKey); self.hotKey = nil }
        if hotKeyHandler == nil {
            var type = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
            InstallEventHandler(
                GetApplicationEventTarget(),
                { _, _, userData in
                    guard let userData else { return OSStatus(eventNotHandledErr) }
                    let controller = Unmanaged<SystemIntegrationController>.fromOpaque(userData).takeUnretainedValue()
                    Task { @MainActor in controller.toggleMainWindow() }
                    return noErr
                },
                1,
                &type,
                Unmanaged.passUnretained(self).toOpaque(),
                &hotKeyHandler
            )
        }
        let modifiers: UInt32 = switch choice {
        case .optionSpace: UInt32(optionKey)
        case .commandShiftSpace: UInt32(cmdKey | shiftKey)
        case .controlSpace: UInt32(controlKey)
        }
        let id = EventHotKeyID(signature: OSType(0x4D_48_4E_54), id: 1) // MHNT
        let result = RegisterEventHotKey(
            UInt32(kVK_Space), modifiers, id, GetApplicationEventTarget(), 0, &hotKey
        )
        if result != noErr { searchStore?.errorMessage = "The selected global shortcut is already in use." }
    }

    private func configureLoginItem(enabled: Bool) {
        do {
            let service = SMAppService.mainApp
            if enabled, service.status == .notRegistered { try service.register() }
            if !enabled, service.status != .notRegistered { try service.unregister() }
        } catch {
            searchStore?.errorMessage = "Launch at Login could not be changed. Install MacHunt.app in the Applications folder and try again."
        }
    }

    @objc private func showSearch() { showMainWindow(section: .search) }
    @objc private func showPinned() { showMainWindow(section: .pinned) }
    @objc private func showSettings() {
        NSApp.activate(ignoringOtherApps: true)
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
    }
    @objc private func toggleMonitoring() {
        guard let preferences else { return }
        searchStore?.setMonitoringPaused(!preferences.monitoringPaused)
        configureStatusItem(visible: preferences.showMenuBarItem)
    }
    @objc private func rebuildIndex() { searchStore?.rebuildConfiguredIndex() }
    @objc private func quit() { NSApp.terminate(nil) }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { SystemIntegrationController.shared.showMainWindow() }
        return true
    }
}

extension Notification.Name {
    static let macHuntNavigate = Notification.Name("MacHuntNavigate")
}
