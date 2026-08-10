import AppKit
import MacHuntCore
import SwiftUI

struct SearchScreen: View {
    @EnvironmentObject private var store: SearchStore
    @EnvironmentObject private var pinned: PinnedStore
    @EnvironmentObject private var preferences: AppPreferences
    @State private var pendingTrash: [SearchResult] = []
    @State private var confirmTrash = false
    @State private var showPathSuggestions = false
    @State private var showCustomDate = false
    @State private var customDateFrom = Calendar.current.date(byAdding: .month, value: -1, to: .now) ?? .now
    @State private var customDateTo = Date.now
    @State private var showCustomSize = false
    @State private var customMinimumSize = ""
    @State private var customMaximumSize = ""
    @State private var customMinimumUnit = SizeUnit.megabytes
    @State private var customMaximumUnit = SizeUnit.megabytes

    var body: some View {
        VStack(spacing: 0) {
            controls
            Divider()
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            Divider()
            statusBar
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .onChange(of: preferences.fileTypeExtensions) { _, _ in
            if selectedCategoryOption?.category.supportsEditableExtensions == true { store.submitSearch() }
        }
        .onChange(of: preferences.enabledFileTypeCategories) { _, _ in
            reconcileSelectedCategory()
        }
        .onChange(of: preferences.customFileTypeCategories) { _, _ in
            reconcileSelectedCategory()
        }
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    store.rebuildConfiguredIndex()
                } label: {
                    Label("Rebuild Index", systemImage: "arrow.clockwise")
                }
                .disabled(store.isBuilding)

                categoryFilterMenu

                NativeSearchField(
                    text: $store.query,
                    prompt: String(localized: "Search files and folders"),
                    onSubmit: store.submitSearch
                )
                .frame(width: 300)
            }
        }
        .alert("MacHunt", isPresented: Binding(
            get: { store.errorMessage != nil },
            set: { if !$0 { store.errorMessage = nil } }
        )) {
            Button("OK") { store.errorMessage = nil }
        } message: {
            Text(store.errorMessage ?? "")
        }
        .alert("Move selected items to Trash?", isPresented: $confirmTrash) {
            Button("Move to Trash", role: .destructive) { trash(pendingTrash) }
            Button("Cancel", role: .cancel) { pendingTrash = [] }
        } message: {
            Text("This moves \(pendingTrash.count) item(s) to the Trash.")
        }
    }

    private var controls: some View {
        HStack(spacing: 8) {
            pathFilter

            Toggle("Regex", isOn: Binding(
                get: { store.mode == .pattern },
                set: { store.mode = $0 ? .pattern : .substring }
            ))
            .toggleStyle(.button)

            Toggle("Fuzzy", isOn: Binding(
                get: { store.mode == .fuzzy },
                set: { store.mode = $0 ? .fuzzy : .substring }
            ))
            .toggleStyle(.button)

            Toggle("Case Sensitive", isOn: $store.caseSensitive)
                .toggleStyle(.button)

            dateMenu
            sizeMenu

            Picker("Application", selection: $store.selectedApplicationID) {
                ForEach(store.applicationGroups) { item in
                    Text(item.itemCount > 0 ? "\(item.name) (\(item.itemCount.formatted()))" : item.name)
                        .tag(item.id)
                }
            }
            .labelsHidden()
            .frame(width: 150)

            Picker("Sort", selection: $store.sort) {
                Text("Name").tag(SearchSort.name)
                Text("Path").tag(SearchSort.path)
                Text("Type").tag(SearchSort.type)
                Text("Size").tag(SearchSort.size)
                Text("Modified").tag(SearchSort.modified)
                Text("Date Added").tag(SearchSort.added)
                Text("Cloud Status").tag(SearchSort.cloudStatus)
                Text("Tags").tag(SearchSort.tags)
            }
            .labelsHidden()
            .frame(width: 110)

            columnsMenu

            Button {
                store.toggleSortDirection()
            } label: {
                Label(store.ascending ? "Ascending" : "Descending", systemImage: store.ascending ? "arrow.up" : "arrow.down")
                    .labelStyle(.iconOnly)
            }
        }
        .controlSize(.small)
        .padding()
        .disabled(store.isBuilding && store.indexedItemCount == 0)
    }

    private var columnsMenu: some View {
        Menu {
            ForEach(ResultTableColumn.allCases) { column in
                Button {
                    preferences.setResultColumn(
                        column,
                        visible: !preferences.isResultColumnVisible(column)
                    )
                } label: {
                    Label {
                        HStack {
                            Text(LocalizedStringKey(column.title))
                            if preferences.isResultColumnVisible(column) {
                                Image(systemName: "checkmark")
                            }
                        }
                    } icon: {
                        Image(systemName: column.systemImage)
                    }
                }
                .disabled(column.isRequired)
            }
        } label: {
            Label("Choose Columns", systemImage: "rectangle.3.group")
                .labelStyle(.iconOnly)
        }
        .help("Choose Columns")
    }

    private var categoryFilterMenu: some View {
        Menu {
            Section("Built-in Categories") {
                ForEach(preferences.builtInSearchCategoryOptions) { option in
                    categoryMenuButton(option)
                }
            }
            if !preferences.customSearchCategoryOptions.isEmpty {
                Section("Custom Categories") {
                    ForEach(preferences.customSearchCategoryOptions) { option in
                        categoryMenuButton(option)
                    }
                }
            }
        } label: {
            Label {
                if selectedCategoryOption?.category == .custom {
                    Text(selectedCategoryOption?.title ?? "All")
                } else {
                    Text(LocalizedStringKey(selectedCategoryOption?.title ?? "All"))
                }
            } icon: {
                categoryIcon(selectedCategoryOption)
            }
        }
        .help("Filter by file type category")
        .accessibilityLabel("File Type Category")
    }

    private func categoryMenuButton(_ option: SearchCategoryOption) -> some View {
        Button {
            store.selectedCategoryID = option.id
        } label: {
            Label {
                HStack(spacing: 6) {
                    if option.category == .custom {
                        Text(option.title)
                    } else {
                        Text(LocalizedStringKey(option.title))
                    }
                    if store.selectedCategoryID == option.id {
                        Image(systemName: "checkmark")
                    }
                }
            } icon: {
                categoryIcon(option)
            }
        }
    }

    @ViewBuilder
    private func categoryIcon(_ option: SearchCategoryOption?) -> some View {
        if let emoji = option?.emoji, !emoji.isEmpty {
            Text(emoji)
        } else {
            Image(systemName: option?.systemImage ?? "square.grid.2x2")
        }
    }

    private var pathFilter: some View {
        HStack(spacing: 4) {
            TextField("Path prefix", text: $store.pathPrefix)
                .textFieldStyle(.roundedBorder)
                .onSubmit { store.submitSearch() }

            Button {
                store.refreshPathSuggestions()
                showPathSuggestions.toggle()
            } label: {
                Label("Path Suggestions", systemImage: "chevron.down")
                    .labelStyle(.iconOnly)
            }
            .help("Choose an indexed folder")
            .popover(isPresented: $showPathSuggestions, arrowEdge: .bottom) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Indexed Folders").font(.headline)
                    if store.pathSuggestions.isEmpty {
                        ContentUnavailableView("No Matching Folders", systemImage: "folder")
                    } else {
                        ScrollView {
                            LazyVStack(alignment: .leading, spacing: 2) {
                                ForEach(store.pathSuggestions, id: \.self) { path in
                                    Button {
                                        store.pathPrefix = path
                                        showPathSuggestions = false
                                    } label: {
                                        Label(path, systemImage: "folder")
                                            .lineLimit(1)
                                            .truncationMode(.middle)
                                            .frame(maxWidth: .infinity, alignment: .leading)
                                    }
                                    .buttonStyle(.plain)
                                    .padding(.vertical, 4)
                                }
                            }
                        }
                    }
                }
                .padding()
                .frame(width: 440, height: 280)
            }

            Button { store.choosePathPrefix() } label: {
                Label("Choose Folder…", systemImage: "folder")
                    .labelStyle(.iconOnly)
            }
            .help("Choose a folder in Finder")
        }
    }

    @ViewBuilder
    private var content: some View {
        if store.isBuilding && store.indexedItemCount == 0 {
            ContentUnavailableView {
                Label("Building Search Index", systemImage: "externaldrive.badge.timemachine")
            } description: {
                Text("MacHunt is indexing this Mac and mounted volumes. Search becomes available while indexing continues.")
            } actions: {
                ProgressView()
                    .controlSize(.regular)
                    .padding(.top, 8)
            }
        } else if selectedCategoryOption?.category.supportsEditableExtensions == true,
                  selectedCategoryOption?.extensions?.isEmpty == true {
            ContentUnavailableView {
                Label("No File Types Configured", systemImage: "slider.horizontal.3")
            } description: {
                Text("Add file extensions in Settings to use this category.")
            } actions: {
                SettingsLink {
                    Text("Open Settings…")
                }
            }
        } else if store.results.isEmpty {
            ContentUnavailableView.search(text: store.query)
        } else {
            resultTable
        }
    }

    private var selectedCategoryOption: SearchCategoryOption? {
        preferences.categoryOption(id: store.selectedCategoryID)
    }

    private func reconcileSelectedCategory() {
        if selectedCategoryOption == nil {
            store.selectedCategoryID = ResultCategory.all.rawValue
        } else {
            store.submitSearch()
        }
    }


    private var resultTable: some View {
        NativeResultsTable(
            items: store.results,
            selection: $store.selection,
            autosaveName: "MacHuntSearchResultsTable",
            isPinned: pinned.contains,
            onTogglePin: pinned.toggle,
            onOpen: FileActions.open,
            onQuickLook: { QuickLookController.shared.toggle(items: $0) },
            onReveal: FileActions.reveal,
            onCopyPaths: FileActions.copyPaths,
            onCopyFiles: FileActions.copyFileObjects,
            onTrash: requestTrash,
            onSort: store.setSort,
            sort: store.sort,
            ascending: store.ascending,
            visibleColumnIDs: Set(preferences.visibleResultColumns),
            onToggleColumn: { columnID in
                guard let column = ResultTableColumn(rawValue: columnID) else { return }
                preferences.setResultColumn(
                    column,
                    visible: !preferences.isResultColumnVisible(column)
                )
            }
        )
    }

    private var dateMenu: some View {
        Menu {
            Button("All Time") { store.modifiedAfter = nil; store.modifiedBefore = nil }
            Button("Today") { store.modifiedAfter = Calendar.current.startOfDay(for: .now); store.modifiedBefore = nil }
            Button("Past Week") { store.modifiedAfter = Calendar.current.date(byAdding: .day, value: -7, to: .now); store.modifiedBefore = nil }
            Button("Past Month") { store.modifiedAfter = Calendar.current.date(byAdding: .month, value: -1, to: .now); store.modifiedBefore = nil }
            Button("Past Year") { store.modifiedAfter = Calendar.current.date(byAdding: .year, value: -1, to: .now); store.modifiedBefore = nil }
            Divider()
            Button("Custom Range…") { showCustomDate = true }
        } label: {
            Label(store.modifiedAfter == nil ? "All Time" : "Date", systemImage: "calendar")
        }
        .popover(isPresented: $showCustomDate, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 12) {
                Text("Custom Date Range").font(.headline)
                DatePicker("From", selection: $customDateFrom, displayedComponents: .date)
                DatePicker("To", selection: $customDateTo, in: customDateFrom..., displayedComponents: .date)
                HStack {
                    Button("Clear") {
                        store.modifiedAfter = nil
                        store.modifiedBefore = nil
                        showCustomDate = false
                    }
                    Spacer()
                    Button("Apply") {
                        store.modifiedAfter = Calendar.current.startOfDay(for: customDateFrom)
                        store.modifiedBefore = Calendar.current.date(
                            byAdding: .day,
                            value: 1,
                            to: Calendar.current.startOfDay(for: customDateTo)
                        )
                        showCustomDate = false
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            .padding()
            .frame(width: 300)
        }
    }

    private var sizeMenu: some View {
        Menu {
            Button("All Sizes") { store.minimumSize = nil; store.maximumSize = nil }
            Button("Less Than 1 MB") { store.minimumSize = nil; store.maximumSize = 1_048_575 }
            Button("1–100 MB") { store.minimumSize = 1_048_576; store.maximumSize = 104_857_600 }
            Button("100 MB or Larger") { store.minimumSize = 104_857_600; store.maximumSize = nil }
            Divider()
            Button("Custom Size…") { showCustomSize = true }
        } label: {
            Label(store.minimumSize == nil && store.maximumSize == nil ? "All Sizes" : "Size", systemImage: "internaldrive")
        }
        .popover(isPresented: $showCustomSize, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 12) {
                Text("Custom Size Range").font(.headline)
                HStack {
                    TextField("Minimum", text: $customMinimumSize)
                    Picker("Minimum Unit", selection: $customMinimumUnit) {
                        ForEach(SizeUnit.allCases) { Text($0.title).tag($0) }
                    }
                    .labelsHidden()
                    .frame(width: 90)
                }
                HStack {
                    TextField("Maximum", text: $customMaximumSize)
                    Picker("Maximum Unit", selection: $customMaximumUnit) {
                        ForEach(SizeUnit.allCases) { Text($0.title).tag($0) }
                    }
                    .labelsHidden()
                    .frame(width: 90)
                }
                HStack {
                    Button("Clear") {
                        store.minimumSize = nil
                        store.maximumSize = nil
                        customMinimumSize = ""
                        customMaximumSize = ""
                        showCustomSize = false
                    }
                    Spacer()
                    Button("Apply") { applyCustomSize() }
                        .keyboardShortcut(.defaultAction)
                }
            }
            .padding()
            .frame(width: 320)
        }
    }

    private func applyCustomSize() {
        let minimum = customMinimumUnit.bytes(from: customMinimumSize)
        let maximum = customMaximumUnit.bytes(from: customMaximumSize)
        if let minimum, let maximum, minimum > maximum {
            store.errorMessage = "Minimum size must not exceed maximum size."
            return
        }
        store.minimumSize = minimum
        store.maximumSize = maximum
        showCustomSize = false
    }

    @ViewBuilder
    private func rowMenu(for item: SearchResult) -> some View {
        let targets = store.selection.contains(item.id) ? store.selectedResults : [item]
        Button("Open") { FileActions.open(item) }
        Button("Quick Look") { QuickLookController.shared.toggle(items: targets) }
        Button("Reveal in Finder") { FileActions.reveal(targets) }
        Divider()
        Button("Copy Path") { FileActions.copyPaths(targets) }
        Button("Copy as File Objects") { FileActions.copyFileObjects(targets) }
        Button(pinned.contains(item) ? "Unpin" : "Pin") { pinned.toggle(item) }
        Divider()
        Button("Move to Trash", role: .destructive) { requestTrash(targets) }
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
            if success { store.removeFromIndex(items); items.forEach(pinned.removeIfPresent) }
        }
        pendingTrash = []
    }

    private var statusBar: some View {
        HStack {
            if store.isSearching { ProgressView().controlSize(.small) }
            Text(store.status)
            Spacer()
            if !store.selection.isEmpty { Text("\(store.selection.count) selected") }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(.horizontal)
        .frame(height: 28)
    }
}

private enum SizeUnit: Int64, CaseIterable, Identifiable {
    case bytes = 1
    case kilobytes = 1_024
    case megabytes = 1_048_576
    case gigabytes = 1_073_741_824

    var id: Int64 { rawValue }
    var title: String {
        switch self {
        case .bytes: "B"
        case .kilobytes: "KB"
        case .megabytes: "MB"
        case .gigabytes: "GB"
        }
    }

    func bytes(from text: String) -> Int64? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let number = Double(trimmed), number >= 0 else { return nil }
        let value = number * Double(rawValue)
        guard value <= Double(Int64.max) else { return nil }
        return Int64(value)
    }
}

private func formatBytes(_ bytes: Int64?) -> String {
    guard let bytes else { return "—" }
    return ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
}
