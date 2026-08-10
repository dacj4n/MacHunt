import AppKit
import MacHuntCore
import SwiftUI

struct SettingsScreen: View {
    @EnvironmentObject private var store: SearchStore
    @EnvironmentObject private var preferences: AppPreferences
    @State private var newDirectoryPattern = ""
    @State private var newFilePattern = ""
    @State private var editingFileTypeID = ResultCategory.documents.rawValue
    @State private var categoryNameDraft = ""
    @State private var extensionDraft = ""
    @State private var categoryIconDraft = "tag"
    @State private var categoryEmojiDraft = ""
    @State private var showingIconPicker = false
    @State private var confirmClear = false
    @FocusState private var emojiFieldFocused: Bool

    var body: some View {
        Form {
            Section("Appearance") {
                Picker("Theme", selection: $preferences.appearance) {
                    ForEach(AppAppearance.allCases) { Text($0.title).tag($0) }
                }
                Picker("Language", selection: $preferences.language) {
                    ForEach(AppLanguage.allCases) { Text($0.title).tag($0) }
                }
            }

            Section("Search Index") {
                LabeledContent("Indexed Items", value: store.indexedItemCount.formatted())
                LabeledContent("Status", value: store.status)
                LabeledContent("Live Monitoring", value: store.isMonitoring ? "Active" : "Stopped")
                Toggle("Pause live index monitoring", isOn: $preferences.monitoringPaused)
                    .onChange(of: preferences.monitoringPaused) { _, paused in store.setMonitoringPaused(paused) }
                Stepper("Maximum results: \(preferences.maximumResults)", value: $preferences.maximumResults, in: 100...5_000, step: 100)

                HStack {
                    Button("Rebuild Index") { store.rebuildConfiguredIndex() }
                        .disabled(store.isBuilding)
                    if store.isBuilding {
                        Button("Cancel", role: .cancel) { store.cancelIndexBuild() }
                    }
                    Button("Clear Index", role: .destructive) { confirmClear = true }
                }
            }

            fileTypeCategoriesSection

            Section("Indexed Locations") {
                pathList(paths: $preferences.watchRoots)
                HStack {
                    Button("Add Folder…") { addFolder(to: $preferences.watchRoots) }
                    Spacer()
                    Toggle("Automatically index external volumes", isOn: $preferences.indexExternalVolumes)
                }
                Text("Changing locations or exclusions takes effect after rebuilding the index.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Exclusions") {
                Toggle("Exclude hidden files and folders", isOn: $preferences.excludeHiddenItems)
                Text("Excluded folders").font(.headline)
                pathList(paths: $preferences.excludedDirectories)
                Button("Add Excluded Folder…") { addFolder(to: $preferences.excludedDirectories) }
                patternEditor(
                    title: "Folder patterns",
                    values: $preferences.excludedDirectoryPatterns,
                    draft: $newDirectoryPattern,
                    prompt: "*/Library/Caches/*"
                )
                patternEditor(
                    title: "File patterns",
                    values: $preferences.excludedFilePatterns,
                    draft: $newFilePattern,
                    prompt: "*.tmp"
                )
                Button("Restore Default Exclusions") { preferences.resetIndexRules() }
            }

            Section("System Integration") {
                Toggle("Show menu bar item", isOn: $preferences.showMenuBarItem)
                Toggle("Show Dock icon", isOn: $preferences.showDockIcon)
                Toggle("Launch at login", isOn: $preferences.launchAtLogin)
                Picker("Global show/hide shortcut", selection: $preferences.globalShortcut) {
                    ForEach(GlobalShortcutChoice.allCases) { Text($0.title).tag($0) }
                }
                Toggle("Confirm before moving items to Trash", isOn: $preferences.confirmBeforeTrash)
            }
        }
        .formStyle(.grouped)
        .padding()
        .onAppear(perform: loadExtensionDraft)
        .onChange(of: editingFileTypeID) { _, _ in loadExtensionDraft() }
        .alert("Clear the search index?", isPresented: $confirmClear) {
            Button("Clear Index", role: .destructive) { store.clearIndex() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Search results will be empty until you rebuild the index.")
        }
    }

    private var fileTypeCategoriesSection: some View {
        Section("File Type Categories") {
            Text("Choose which categories appear in Search.")
                .font(.caption)
                .foregroundStyle(.secondary)

            Text("Built-in Categories").font(.headline)
            ForEach(ResultCategory.editableCases) { category in
                Toggle(isOn: Binding(
                    get: { preferences.isCategoryVisible(category) },
                    set: { preferences.setCategory(category, visible: $0) }
                )) {
                    Label {
                        Text(LocalizedStringKey(category.title))
                    } icon: {
                        Image(systemName: category.systemImage)
                    }
                }
            }

            HStack {
                Text("Custom Categories").font(.headline)
                Spacer()
                Button("Add Category", systemImage: "plus") { addCustomCategory() }
            }
            ForEach(preferences.customFileTypeCategories) { item in
                HStack {
                    Toggle(item.name, isOn: Binding(
                        get: { item.isVisible },
                        set: { preferences.setCustomFileTypeCategory(id: item.id, visible: $0) }
                    ))
                    Spacer()
                    Button("Edit", systemImage: "pencil") {
                        editingFileTypeID = customEditorID(item.id)
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.borderless)
                    Button("Delete", systemImage: "minus.circle", role: .destructive) {
                        deleteCustomCategory(item.id)
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.borderless)
                }
            }

            Divider()

            Picker("Edit file type", selection: $editingFileTypeID) {
                Section("Built-in Categories") {
                    ForEach(ResultCategory.editableCases) { category in
                        Text(LocalizedStringKey(category.title)).tag(category.rawValue)
                    }
                }
                if !preferences.customFileTypeCategories.isEmpty {
                    Section("Custom Categories") {
                        ForEach(preferences.customFileTypeCategories) { item in
                            Text(item.name).tag(customEditorID(item.id))
                        }
                    }
                }
            }

            if editingCustomCategoryID != nil {
                alignedInput("Category Name") {
                    TextField("Category Name", text: $categoryNameDraft)
                        .labelsHidden()
                        .textFieldStyle(.roundedBorder)
                        .multilineTextAlignment(.leading)
                }

                alignedInput("Icon") {
                    HStack(alignment: .center, spacing: 8) {
                        Button {
                            showingIconPicker = true
                        } label: {
                            Label("SF Symbols", systemImage: categoryIconDraft)
                        }
                        .popover(isPresented: $showingIconPicker, arrowEdge: .bottom) {
                            iconPicker
                        }

                        Divider()
                            .frame(height: 22)

                        TextField("", text: $categoryEmojiDraft, prompt: Text("🙂"))
                            .labelsHidden()
                            .textFieldStyle(.roundedBorder)
                            .multilineTextAlignment(.center)
                            .font(.title3)
                            .frame(width: 44)
                            .focused($emojiFieldFocused)
                            .onChange(of: categoryEmojiDraft) { _, value in
                                let normalized = normalizedEmoji(value)
                                if value != normalized { categoryEmojiDraft = normalized }
                            }

                        Button {
                            emojiFieldFocused = true
                            DispatchQueue.main.async {
                                NSApp.orderFrontCharacterPalette(nil)
                            }
                        } label: {
                            Label("Choose Emoji…", systemImage: "face.smiling")
                        }
                        .help("Open the macOS Emoji and Symbols viewer")
                    }

                    Text("Choose a common SF Symbol or one Emoji.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            alignedInput("Extensions") {
                TextField("Extensions", text: $extensionDraft, axis: .vertical)
                    .labelsHidden()
                    .textFieldStyle(.roundedBorder)
                    .font(.system(.body, design: .monospaced))
                    .multilineTextAlignment(.leading)
                    .lineLimit(2...4)
            }

            Text("Separate extensions with commas or spaces. Dots are optional.")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack {
                if let category = editingBuiltInCategory {
                    Button("Restore Defaults") {
                        preferences.resetExtensions(for: category)
                        loadExtensionDraft()
                    }
                } else if let id = editingCustomCategoryID {
                    Button("Delete Category", role: .destructive) {
                        deleteCustomCategory(id)
                    }
                }
                Spacer()
                Text("\(parsedExtensionCount) types")
                    .foregroundStyle(.secondary)
                Button("Save") {
                    saveFileTypeEditor()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
    }

    private func alignedInput<Content: View>(
        _ title: LocalizedStringKey,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
            content()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var iconPicker: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Choose Icon")
                .font(.headline)

            LazyVGrid(
                columns: Array(repeating: GridItem(.fixed(32), spacing: 8), count: 7),
                spacing: 8
            ) {
                ForEach(Self.customCategoryIcons, id: \.self) { symbol in
                    iconChoiceButton(symbol)
                }
            }
        }
        .padding()
        .frame(width: 316)
    }

    @ViewBuilder
    private func iconChoiceButton(_ symbol: String) -> some View {
        let button = Button {
            categoryIconDraft = symbol
            categoryEmojiDraft = ""
            showingIconPicker = false
        } label: {
            Image(systemName: symbol)
                .frame(width: 28, height: 28)
        }
        .controlSize(.small)
        .help(symbol)
        .accessibilityLabel(symbol)

        if categoryIconDraft == symbol && categoryEmojiDraft.isEmpty {
            button.buttonStyle(.borderedProminent)
        } else {
            button.buttonStyle(.bordered)
        }
    }

    private static let customCategoryIcons = [
        "tag", "doc", "folder", "doc.text", "photo", "play.rectangle", "archivebox",
        "chevron.left.forwardslash.chevron.right", "book.closed", "music.note", "film", "camera",
        "paintbrush", "cube", "shippingbox", "terminal", "hammer", "wrench.and.screwdriver",
        "graduationcap", "briefcase", "heart", "star", "bolt", "globe", "tray", "paperclip",
        "bookmark", "square.stack.3d.up"
    ]

    private func normalizedEmoji(_ input: String) -> String {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let first = trimmed.first else { return "" }
        return String(first)
    }

    @ViewBuilder
    private func pathList(paths: Binding<[String]>) -> some View {
        ForEach(Array(paths.wrappedValue.enumerated()), id: \.element) { index, path in
            HStack {
                Image(systemName: "folder")
                Text(path).lineLimit(1).truncationMode(.middle)
                Spacer()
                Button("Remove", systemImage: "minus.circle") {
                    paths.wrappedValue.remove(at: index)
                }
                .labelStyle(.iconOnly)
                .buttonStyle(.borderless)
            }
        }
    }

    private func patternEditor(
        title: String,
        values: Binding<[String]>,
        draft: Binding<String>,
        prompt: String
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.headline)
            ForEach(values.wrappedValue, id: \.self) { value in
                HStack {
                    Text(value).font(.system(.body, design: .monospaced))
                    Spacer()
                    Button("Remove", systemImage: "minus.circle") {
                        values.wrappedValue.removeAll { $0 == value }
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.borderless)
                }
            }
            HStack {
                TextField(prompt, text: draft)
                Button("Add") {
                    let value = draft.wrappedValue.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !value.isEmpty, !values.wrappedValue.contains(value) else { return }
                    values.wrappedValue.append(value)
                    draft.wrappedValue = ""
                }
            }
        }
    }

    private func addFolder(to paths: Binding<[String]>) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = true
        guard panel.runModal() == .OK else { return }
        for path in panel.urls.map(\.standardizedFileURL.path) where !paths.wrappedValue.contains(path) {
            paths.wrappedValue.append(path)
        }
    }

    private var parsedExtensionCount: Int {
        extensionDraft.components(separatedBy: CharacterSet(charactersIn: ",; \n\t"))
            .filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
            .count
    }

    private var editingBuiltInCategory: ResultCategory? {
        guard let category = ResultCategory(rawValue: editingFileTypeID),
              ResultCategory.editableCases.contains(category) else { return nil }
        return category
    }

    private var editingCustomCategoryID: UUID? {
        guard editingFileTypeID.hasPrefix("custom:") else { return nil }
        return UUID(uuidString: String(editingFileTypeID.dropFirst("custom:".count)))
    }

    private func customEditorID(_ id: UUID) -> String {
        "custom:\(id.uuidString)"
    }

    private func loadExtensionDraft() {
        if let category = editingBuiltInCategory {
            categoryNameDraft = ""
            extensionDraft = (preferences.extensions(for: category) ?? []).sorted().joined(separator: ", ")
            categoryIconDraft = "tag"
            categoryEmojiDraft = ""
        } else if let id = editingCustomCategoryID,
                  let item = preferences.customFileTypeCategory(id: id) {
            categoryNameDraft = item.name
            extensionDraft = item.extensions.joined(separator: ", ")
            categoryIconDraft = item.systemImage ?? "tag"
            categoryEmojiDraft = item.emoji ?? ""
        } else {
            editingFileTypeID = ResultCategory.documents.rawValue
        }
    }

    private func addCustomCategory() {
        let id = preferences.addCustomFileTypeCategory()
        editingFileTypeID = customEditorID(id)
        loadExtensionDraft()
    }

    private func deleteCustomCategory(_ id: UUID) {
        preferences.removeCustomFileTypeCategory(id: id)
        if editingCustomCategoryID == id {
            editingFileTypeID = ResultCategory.documents.rawValue
            loadExtensionDraft()
        }
    }

    private func saveFileTypeEditor() {
        if let category = editingBuiltInCategory {
            preferences.setExtensions(for: category, from: extensionDraft)
        } else if let id = editingCustomCategoryID {
            preferences.updateCustomFileTypeCategory(
                id: id,
                name: categoryNameDraft,
                extensions: extensionDraft,
                systemImage: categoryIconDraft,
                emoji: categoryEmojiDraft.isEmpty ? nil : categoryEmojiDraft
            )
        }
        loadExtensionDraft()
    }
}
