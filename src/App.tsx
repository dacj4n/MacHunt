import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type TabId = "all" | "files" | "folders" | "documents" | "images" | "media" | "code" | "archives";
type SortKey = "name" | "path" | "type" | "size" | "modified";
type ColumnKey = "name" | "path" | "type" | "size" | "modified";
type ThemeMode = "system" | "light" | "dark";
type ViewMode = "search" | "settings";
type Language = "zh" | "en";

const DEFAULT_COLUMN_WIDTHS: Record<ColumnKey, number> = {
  name: 500,
  path: 420,
  type: 140,
  size: 160,
  modified: 300
};

const MIN_COLUMN_WIDTHS: Record<ColumnKey, number> = {
  name: 300,
  path: 220,
  type: 110,
  size: 90,
  modified: 170
};

const THEME_STORAGE_KEY = "machunt.theme.mode";
const LANGUAGE_STORAGE_KEY = "machunt.language";
const COLUMN_WIDTHS_STORAGE_KEY = "machunt.table.column.widths";
const LEGACY_SEARCH_MODE_STORAGE_KEY = "machunt.search.mode";
const REGEX_ENABLED_STORAGE_KEY = "machunt.search.regex_enabled";
const CASE_SENSITIVE_STORAGE_KEY = "machunt.search.case_sensitive";
const EVENT_OPEN_SETTINGS = "app://open-settings";
const TAB_IDS: TabId[] = ["all", "files", "folders", "documents", "images", "media", "code", "archives"];
const PREVIEW_OPEN_MS = 240;
const PREVIEW_CLOSE_MS = 220;

const I18N = {
  zh: {
    searchPlaceholder: "搜索文件、文件夹、内容...",
    searchTag: "搜索",
    pathPlaceholder: "路径过滤",
    choosePath: "选择路径",
    menuOpen: "打开",
    menuOpenWith: "打开于 ...",
    menuFinder: "在Finder中查看",
    menuQSpace: "在QSpace Pro中查看",
    menuTerminal: "在终端中打开",
    menuWezTerm: "在 WezTerm 中打开",
    menuCopyName: "拷贝名称",
    menuCopyPath: "拷贝路径",
    menuCopyAllNames: "拷贝所有文件名",
    menuCopyAllPaths: "拷贝所有文件路径",
    menuTrash: "移到废纸篓",
    regexEnabled: "正则",
    caseSensitive: "区分大小写",
    build: "构建",
    rebuild: "重建",
    startWatch: "开始监听",
    stopWatch: "停止监听",
    starting: "启动中...",
    stopping: "停止中...",
    tab_all: "全部",
    tab_files: "文件",
    tab_folders: "文件夹",
    tab_documents: "文档",
    tab_images: "图片",
    tab_media: "音视频",
    tab_code: "代码",
    tab_archives: "压缩包",
    sort: "排序",
    sort_name: "名称",
    sort_size: "大小",
    sort_modified: "修改时间",
    header_name: "名称",
    header_path: "路径",
    header_type: "类型",
    header_size: "大小",
    header_modified: "修改日期",
    emptyTypeHint: "输入关键词开始搜索。",
    emptyNoMatch: "没有匹配结果。",
    indexedItems: "已索引 {count} 项",
    shownItems: "显示 {count} 项",
    searching: "搜索中...",
    settingsTitle: "设置",
    settingsDesc: "主题和语言配置会立即生效并自动保存。",
    backToSearch: "返回搜索",
    themeModeTitle: "主题模式",
    themeSystemTitle: "随系统变化",
    themeSystemDesc: "跟随 macOS 的深色/浅色外观自动切换。",
    themeLightTitle: "白天主题",
    themeLightDesc: "始终使用浅色主题。",
    themeDarkTitle: "夜间主题",
    themeDarkDesc: "始终使用深色主题。",
    themeCurrent: "当前生效主题",
    languageTitle: "语言",
    languageZhTitle: "中文",
    languageZhDesc: "界面使用中文。",
    languageEnTitle: "English",
    languageEnDesc: "Interface in English."
  },
  en: {
    searchPlaceholder: "Search files, folders, content...",
    searchTag: "Search",
    pathPlaceholder: "Path filter",
    choosePath: "Choose Path",
    menuOpen: "Open",
    menuOpenWith: "Open With ...",
    menuFinder: "Reveal in Finder",
    menuQSpace: "View in QSpace Pro",
    menuTerminal: "Open in Terminal",
    menuWezTerm: "Open in WezTerm",
    menuCopyName: "Copy Name",
    menuCopyPath: "Copy Path",
    menuCopyAllNames: "Copy All Names",
    menuCopyAllPaths: "Copy All Paths",
    menuTrash: "Move to Trash",
    regexEnabled: "Regex",
    caseSensitive: "Case Sensitive",
    build: "Build",
    rebuild: "Rebuild",
    startWatch: "Start Watch",
    stopWatch: "Stop Watch",
    starting: "Starting...",
    stopping: "Stopping...",
    tab_all: "All",
    tab_files: "Files",
    tab_folders: "Folders",
    tab_documents: "Documents",
    tab_images: "Images",
    tab_media: "Media",
    tab_code: "Code",
    tab_archives: "Archives",
    sort: "Sort",
    sort_name: "Name",
    sort_size: "Size",
    sort_modified: "Modified",
    header_name: "Name",
    header_path: "Path",
    header_type: "Type",
    header_size: "Size",
    header_modified: "Date Modified",
    emptyTypeHint: "Type a keyword to start searching.",
    emptyNoMatch: "No matching files found.",
    indexedItems: "{count} items indexed",
    shownItems: "{count} items shown",
    searching: "Searching...",
    settingsTitle: "Settings",
    settingsDesc: "Theme and language changes apply immediately and are saved.",
    backToSearch: "Back to Search",
    themeModeTitle: "Theme",
    themeSystemTitle: "Follow System",
    themeSystemDesc: "Switch with macOS appearance automatically.",
    themeLightTitle: "Light",
    themeLightDesc: "Always use light appearance.",
    themeDarkTitle: "Dark",
    themeDarkDesc: "Always use dark appearance.",
    themeCurrent: "Current theme",
    languageTitle: "Language",
    languageZhTitle: "中文",
    languageZhDesc: "Show interface in Chinese.",
    languageEnTitle: "English",
    languageEnDesc: "Show interface in English."
  }
} as const;

interface SearchResultItem {
  name: string;
  path: string;
  parent: string;
  isDir: boolean;
  isFile: boolean;
  sizeBytes?: number;
  modifiedUnixMs?: number;
}

interface SearchResponse {
  items: SearchResultItem[];
  total: number;
  tookMs: number;
}

interface InitResponse {
  indexed: number;
  hasIndex: boolean;
  lastEventId?: number;
}

interface BuildResponse {
  indexed: number;
  tookMs: number;
}

interface BuildEvent {
  phase: "started" | "finished";
  indexed?: number;
  tookMs?: number;
}

interface WatchResponse {
  running: boolean;
  mode: string;
  message: string;
  lastEventId?: number;
}

interface PreviewStatusEvent {
  phase: "opened" | "closed";
  sessionId: number;
}

interface ContextMenuState {
  x: number;
  y: number;
  item: SearchResultItem;
  multiSelection: boolean;
}

interface PreviewZoomState {
  id: number;
  left: number;
  top: number;
  width: number;
  height: number;
  opacity: number;
  transitionMs: number;
}

function extensionOf(name: string): string {
  const idx = name.lastIndexOf(".");
  if (idx < 0 || idx === name.length - 1) {
    return "";
  }
  return name.slice(idx + 1).toLowerCase();
}

function classifyTab(item: SearchResultItem): TabId {
  if (item.isDir) {
    return "folders";
  }

  const ext = extensionOf(item.name);
  if (["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md"].includes(ext)) {
    return "documents";
  }
  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "heic", "bmp"].includes(ext)) {
    return "images";
  }
  if (["mp3", "m4a", "wav", "flac", "aac", "mp4", "mov", "avi", "mkv"].includes(ext)) {
    return "media";
  }
  if (["rs", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml", "py", "go", "java", "c", "cpp", "h", "hpp", "html", "css"].includes(ext)) {
    return "code";
  }
  if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz"].includes(ext)) {
    return "archives";
  }
  return "all";
}

function filterByTab(items: SearchResultItem[], tab: TabId): SearchResultItem[] {
  if (tab === "all") {
    return items;
  }
  if (tab === "files") {
    return items.filter((item) => item.isFile);
  }
  if (tab === "folders") {
    return items.filter((item) => item.isDir);
  }
  return items.filter((item) => classifyTab(item) === tab);
}

function sortItems(items: SearchResultItem[], key: SortKey, ascending: boolean): SearchResultItem[] {
  const sorted = [...items];
  sorted.sort((a, b) => {
    let cmp = 0;
    if (key === "name") {
      cmp = a.name.localeCompare(b.name);
    } else if (key === "path") {
      cmp = a.parent.localeCompare(b.parent);
    } else if (key === "type") {
      cmp = typeSortKey(a).localeCompare(typeSortKey(b));
    } else if (key === "size") {
      cmp = (a.sizeBytes ?? -1) - (b.sizeBytes ?? -1);
    } else {
      cmp = (a.modifiedUnixMs ?? 0) - (b.modifiedUnixMs ?? 0);
    }
    return ascending ? cmp : -cmp;
  });
  return sorted;
}

function formatBytes(bytes?: number): string {
  if (!bytes || bytes < 0) {
    return "--";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const kb = bytes / 1024;
  if (kb < 1024) {
    return `${kb.toFixed(1)} KB`;
  }
  const mb = kb / 1024;
  if (mb < 1024) {
    return `${mb.toFixed(1)} MB`;
  }
  const gb = mb / 1024;
  return `${gb.toFixed(1)} GB`;
}

function formatDate(ms?: number): string {
  if (!ms) {
    return "--";
  }
  return new Date(ms).toLocaleString();
}

function iconToken(item: SearchResultItem): string {
  if (item.isDir) {
    return "folder";
  }
  const tab = classifyTab(item);
  if (tab === "documents") {
    return "doc";
  }
  if (tab === "images") {
    return "img";
  }
  if (tab === "media") {
    return "media";
  }
  if (tab === "code") {
    return "code";
  }
  if (tab === "archives") {
    return "archive";
  }
  return "file";
}

function typeLabel(item: SearchResultItem, language: Language): string {
  if (item.isDir) {
    return language === "zh" ? "文件夹" : "Folder";
  }

  const ext = extensionOf(item.name);
  if (ext.length > 0) {
    return ext.toUpperCase();
  }
  return language === "zh" ? "文件" : "File";
}

function typeSortKey(item: SearchResultItem): string {
  if (item.isDir) {
    return "0-folder";
  }

  const ext = extensionOf(item.name);
  if (ext.length > 0) {
    return `1-${ext.toLowerCase()}`;
  }
  return "1-file";
}

function iconGlyph(token: string): string {
  switch (token) {
    case "folder":
      return "F";
    case "doc":
      return "D";
    case "img":
      return "I";
    case "media":
      return "M";
    case "code":
      return "C";
    case "archive":
      return "A";
    default:
      return "*";
  }
}

function setCellPreviewTooltip(
  event: React.MouseEvent<HTMLElement>,
  text: string
) {
  const cell = event.currentTarget;
  const isTruncated = cell.scrollWidth > cell.clientWidth;
  if (isTruncated) {
    cell.title = text;
  } else {
    cell.removeAttribute("title");
  }
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable;
}

function blurActiveEditable(): void {
  if (typeof document === "undefined") {
    return;
  }
  const active = document.activeElement;
  if (active instanceof HTMLElement && isEditableTarget(active)) {
    active.blur();
  }
}

function buildSearchRequest(
  query: string,
  tab: TabId,
  pathPrefix: string,
  caseSensitive: boolean,
  regexEnabled: boolean
) {
  const includeFiles = tab !== "folders";
  const includeDirs = tab === "all" || tab === "folders";
  return {
    request: {
      query,
      mode: "Substring",
      regexEnabled,
      caseSensitive,
      pathPrefix: pathPrefix.trim() || null,
      includeFiles,
      includeDirs,
      limit: 2500
    }
  };
}

function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolveTheme(themeMode: ThemeMode, systemDark: boolean): "light" | "dark" {
  if (themeMode === "system") {
    return systemDark ? "dark" : "light";
  }
  return themeMode;
}

function detectDefaultLanguage(): Language {
  if (typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")) {
    return "zh";
  }
  return "en";
}

function fmt(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(vars[key] ?? ""));
}

function normalizeStoredColumnWidth(value: unknown, key: ColumnKey): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }
  const min = MIN_COLUMN_WIDTHS[key];
  const max = 2200;
  return Math.round(Math.max(min, Math.min(max, value)));
}

function loadStoredColumnWidths(): Record<ColumnKey, number> | null {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(COLUMN_WIDTHS_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as Partial<Record<ColumnKey, unknown>>;
    const name = normalizeStoredColumnWidth(parsed.name, "name");
    const path = normalizeStoredColumnWidth(parsed.path, "path");
    const type = normalizeStoredColumnWidth(parsed.type, "type") ?? DEFAULT_COLUMN_WIDTHS.type;
    const size = normalizeStoredColumnWidth(parsed.size, "size");
    const modified = normalizeStoredColumnWidth(parsed.modified, "modified");
    if (name === null || path === null || size === null || modified === null) {
      return null;
    }
    return { name, path, type, size, modified };
  } catch {
    return null;
  }
}

function loadStoredRegexEnabled(): boolean | null {
  if (typeof window === "undefined") {
    return null;
  }
  const raw = window.localStorage.getItem(REGEX_ENABLED_STORAGE_KEY);
  if (raw === "1") {
    return true;
  }
  if (raw === "0") {
    return false;
  }

  const legacy = window.localStorage.getItem(LEGACY_SEARCH_MODE_STORAGE_KEY);
  if (legacy === "Pattern") {
    return true;
  }
  if (legacy === "Substring") {
    return false;
  }
  return null;
}

function loadStoredCaseSensitive(): boolean | null {
  if (typeof window === "undefined") {
    return null;
  }
  const raw = window.localStorage.getItem(CASE_SENSITIVE_STORAGE_KEY);
  if (raw === "1") {
    return true;
  }
  if (raw === "0") {
    return false;
  }
  return null;
}

function App() {
  const [activeView, setActiveView] = useState<ViewMode>("search");
  const [themeMode, setThemeMode] = useState<ThemeMode>("system");
  const [systemDark, setSystemDark] = useState(systemPrefersDark());
  const [language, setLanguage] = useState<Language>(detectDefaultLanguage());
  const [query, setQuery] = useState("");
  const [pathPrefix, setPathPrefix] = useState("");
  const [pathSuggestions, setPathSuggestions] = useState<string[]>([]);
  const [isPathDropdownOpen, setIsPathDropdownOpen] = useState(false);
  const [activePathSuggestion, setActivePathSuggestion] = useState(-1);
  const [regexEnabled, setRegexEnabled] = useState(() => loadStoredRegexEnabled() ?? false);
  const [caseSensitive, setCaseSensitive] = useState(() => loadStoredCaseSensitive() ?? false);
  const [activeTab, setActiveTab] = useState<TabId>("all");
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortAscending, setSortAscending] = useState(true);

  const [items, setItems] = useState<SearchResultItem[]>([]);
  const [indexed, setIndexed] = useState(0);
  const [totalFound, setTotalFound] = useState(0);
  const [tookMs, setTookMs] = useState(0);
  const [buildStatus, setBuildStatus] = useState("Ready");
  const [watchStatus, setWatchStatus] = useState("Watcher stopped");
  const [isWatchRunning, setIsWatchRunning] = useState(false);
  const [isWatchPending, setIsWatchPending] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [isIndexLoading, setIsIndexLoading] = useState(true);
  const [isBuilding, setIsBuilding] = useState(false);
  const [isPickingPath, setIsPickingPath] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [openWithVisible, setOpenWithVisible] = useState(false);
  const [selectedItemPaths, setSelectedItemPaths] = useState<string[]>([]);
  const [selectionAnchorPath, setSelectionAnchorPath] = useState<string | null>(null);
  const [previewZoom, setPreviewZoom] = useState<PreviewZoomState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [columnWidths, setColumnWidths] = useState<Record<ColumnKey, number>>(
    () => loadStoredColumnWidths() ?? DEFAULT_COLUMN_WIDTHS
  );
  const [activeResizer, setActiveResizer] = useState<string | null>(null);
  const tableShellRef = useRef<HTMLElement | null>(null);
  const pathPickerRef = useRef<HTMLDivElement | null>(null);
  const pathInputRef = useRef<HTMLInputElement | null>(null);
  const rowRefs = useRef(new Map<string, HTMLElement>());
  const openWithCloseTimerRef = useRef<number | null>(null);
  const previewZoomCleanupRef = useRef<number | null>(null);
  const previewZoomRef = useRef<PreviewZoomState | null>(null);
  const previewSourcePathRef = useRef<string | null>(null);
  const previewActiveSessionRef = useRef<number | null>(null);
  const columnWidthsRef = useRef(columnWidths);
  const resizeStateRef = useRef<{
    left: ColumnKey;
    right: ColumnKey;
    startX: number;
    leftStart: number;
    rightStart: number;
  } | null>(null);

  const gridTemplateColumns = `${columnWidths.name}px ${columnWidths.path}px ${columnWidths.type}px ${columnWidths.size}px ${columnWidths.modified}px`;
  const resolvedTheme = resolveTheme(themeMode, systemDark);
  const t = I18N[language];
  const normalizedPathPrefix = pathPrefix.trim().toLowerCase();
  const visiblePathSuggestions = [...pathSuggestions]
    .filter((path) => {
      if (!normalizedPathPrefix) {
        return true;
      }
      return path.toLowerCase().includes(normalizedPathPrefix);
    })
    .sort((left, right) => {
      const leftLower = left.toLowerCase();
      const rightLower = right.toLowerCase();
      const leftStarts = normalizedPathPrefix.length > 0 && leftLower.startsWith(normalizedPathPrefix) ? 0 : 1;
      const rightStarts = normalizedPathPrefix.length > 0 && rightLower.startsWith(normalizedPathPrefix) ? 0 : 1;
      if (leftStarts !== rightStarts) {
        return leftStarts - rightStarts;
      }
      if (left.length !== right.length) {
        return left.length - right.length;
      }
      return left.localeCompare(right);
    })
    .slice(0, 8);
  const isPathDropdownVisible = !isIndexLoading && isPathDropdownOpen && visiblePathSuggestions.length > 0;
  const selectedItemPathSet = useMemo(() => new Set(selectedItemPaths), [selectedItemPaths]);
  const selectedItemsInOrder = useMemo(
    () => items.filter((item) => selectedItemPathSet.has(item.path)),
    [items, selectedItemPathSet]
  );
  const selectedPathsInOrder = useMemo(
    () => selectedItemsInOrder.map((item) => item.path),
    [selectedItemsInOrder]
  );
  const hasMultiSelection = selectedPathsInOrder.length > 1;

  const tabLabel = (tab: TabId): string => t[`tab_${tab}` as const];
  const formatIndexedItems = (count: number): string => fmt(t.indexedItems, { count: count.toLocaleString() });
  const formatShownItems = (count: number): string => fmt(t.shownItems, { count: count.toLocaleString() });
  const clearOpenWithCloseTimer = () => {
    if (openWithCloseTimerRef.current !== null) {
      window.clearTimeout(openWithCloseTimerRef.current);
      openWithCloseTimerRef.current = null;
    }
  };
  const openOpenWithMenu = () => {
    clearOpenWithCloseTimer();
    setOpenWithVisible(true);
  };
  const scheduleCloseOpenWithMenu = () => {
    clearOpenWithCloseTimer();
    openWithCloseTimerRef.current = window.setTimeout(() => {
      setOpenWithVisible(false);
      openWithCloseTimerRef.current = null;
    }, 220);
  };
  const closeContextMenu = () => {
    clearOpenWithCloseTimer();
    setContextMenu(null);
    setOpenWithVisible(false);
  };

  const closePathDropdown = () => {
    setIsPathDropdownOpen(false);
    setActivePathSuggestion(-1);
  };

  const applyPathSuggestion = (path: string) => {
    setPathPrefix(path);
    closePathDropdown();
    pathInputRef.current?.focus();
  };

  const handlePathInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      closePathDropdown();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (!isPathDropdownOpen) {
        setIsPathDropdownOpen(true);
      }
      if (visiblePathSuggestions.length === 0) {
        return;
      }
      setActivePathSuggestion((prev) => {
        if (prev < 0 || prev >= visiblePathSuggestions.length - 1) {
          return 0;
        }
        return prev + 1;
      });
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (!isPathDropdownOpen) {
        setIsPathDropdownOpen(true);
      }
      if (visiblePathSuggestions.length === 0) {
        return;
      }
      setActivePathSuggestion((prev) => {
        if (prev <= 0) {
          return visiblePathSuggestions.length - 1;
        }
        return prev - 1;
      });
      return;
    }

    if (event.key === "Enter" && isPathDropdownVisible && activePathSuggestion >= 0) {
      event.preventDefault();
      applyPathSuggestion(visiblePathSuggestions[activePathSuggestion]);
    }
  };

  useEffect(() => {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === "system" || stored === "light" || stored === "dark") {
      setThemeMode(stored);
    }
  }, []);

  useEffect(() => {
    columnWidthsRef.current = columnWidths;
  }, [columnWidths]);

  useEffect(() => {
    previewZoomRef.current = previewZoom;
  }, [previewZoom]);

  useEffect(() => {
    return () => {
      clearOpenWithCloseTimer();
      if (previewZoomCleanupRef.current !== null) {
        window.clearTimeout(previewZoomCleanupRef.current);
        previewZoomCleanupRef.current = null;
      }
      previewActiveSessionRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (activePathSuggestion < visiblePathSuggestions.length) {
      return;
    }
    setActivePathSuggestion(-1);
  }, [activePathSuggestion, visiblePathSuggestions.length]);

  useEffect(() => {
    if (!isPathDropdownOpen) {
      return;
    }

    const closeWhenClickOutside = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }
      if (pathPickerRef.current?.contains(target)) {
        return;
      }
      closePathDropdown();
    };

    window.addEventListener("mousedown", closeWhenClickOutside);
    return () => {
      window.removeEventListener("mousedown", closeWhenClickOutside);
    };
  }, [isPathDropdownOpen]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeContextMenu();
      }
    };
    const close = () => closeContextMenu();

    window.addEventListener("keydown", closeOnEscape);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("keydown", closeOnEscape);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [contextMenu]);

  useEffect(() => {
    const visible = new Set(items.map((item) => item.path));
    setSelectedItemPaths((prev) => {
      const next = prev.filter((path) => visible.has(path));
      return next.length === prev.length ? prev : next;
    });
    setSelectionAnchorPath((prev) => (prev && visible.has(prev) ? prev : null));
  }, [items]);

  useEffect(() => {
    const stored = localStorage.getItem(LANGUAGE_STORAGE_KEY);
    if (stored === "zh" || stored === "en") {
      setLanguage(stored);
    }
  }, []);

  useEffect(() => {
    localStorage.setItem(THEME_STORAGE_KEY, themeMode);
  }, [themeMode]);

  useEffect(() => {
    localStorage.setItem(REGEX_ENABLED_STORAGE_KEY, regexEnabled ? "1" : "0");
  }, [regexEnabled]);

  useEffect(() => {
    localStorage.setItem(CASE_SENSITIVE_STORAGE_KEY, caseSensitive ? "1" : "0");
  }, [caseSensitive]);

  useEffect(() => {
    localStorage.setItem(LANGUAGE_STORAGE_KEY, language);
  }, [language]);

  useEffect(() => {
    void invoke("set_menu_language", { language });
  }, [language]);

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", resolvedTheme);
  }, [resolvedTheme]);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => {
      setSystemDark(event.matches);
    };
    setSystemDark(media.matches);

    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", onChange);
      return () => media.removeEventListener("change", onChange);
    }

    const legacyListener = (event: MediaQueryListEvent) => onChange(event);
    media.addListener(legacyListener);
    return () => media.removeListener(legacyListener);
  }, []);

  useEffect(() => {
    if (isIndexLoading) {
      closePathDropdown();
    }
  }, [isIndexLoading]);

  useEffect(() => {
    let unlistenMenu: (() => void) | undefined;
    void listen(EVENT_OPEN_SETTINGS, () => {
      setActiveView("settings");
    })
      .then((dispose) => {
        unlistenMenu = dispose;
      })
      .catch(() => {
        // Keep UI usable even if menu event binding fails.
      });

    return () => {
      if (unlistenMenu) {
        unlistenMenu();
      }
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const init = async () => {
      if (mounted) {
        setIsIndexLoading(true);
      }
      try {
        const initial = await invoke<InitResponse>("initialize");
        if (!mounted) {
          return;
        }
        setIndexed(initial.indexed);
        if (!initial.hasIndex) {
          setBuildStatus("Index missing. Click Build to create it.");
        }
      } catch (err) {
        if (!mounted) {
          return;
        }
        setError(String(err));
      } finally {
        if (mounted) {
          setIsIndexLoading(false);
        }
      }

      try {
        const watch = await invoke<WatchResponse>("start_watch_auto");
        if (mounted) {
          setIsWatchRunning(watch.running);
          setWatchStatus(watch.message);
        }
      } catch (err) {
        if (mounted) {
          setError(String(err));
        }
        try {
          const watch = await invoke<WatchResponse>("watch_status");
          if (mounted) {
            setIsWatchRunning(watch.running);
            setWatchStatus(watch.message);
          }
        } catch {
          // Keep UI usable even if watcher state fetch fails.
        }
      }
    };

    void init();

    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    let mounted = true;
    const loadPathSuggestions = async () => {
      try {
        const suggestions = await invoke<string[]>("list_path_suggestions");
        if (!mounted) {
          return;
        }
        setPathSuggestions(suggestions);
      } catch {
        // Keep path filter usable even if suggestion load fails.
      }
    };
    void loadPathSuggestions();
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    const fitColumnsToContainer = () => {
      const shell = tableShellRef.current;
      if (!shell) {
        return;
      }
      const available = Math.max(0, shell.clientWidth - 32);
      setColumnWidths((prev) => {
        const total = prev.name + prev.path + prev.type + prev.size + prev.modified;
        if (total <= available) {
          return prev;
        }

        const next = { ...prev };
        let overflow = total - available;
        const order: ColumnKey[] = ["path", "modified", "size", "name", "type"];

        for (const key of order) {
          if (overflow <= 0) {
            break;
          }
          const minWidth = MIN_COLUMN_WIDTHS[key];
          const current = next[key];
          const reducible = Math.max(0, current - minWidth);
          if (reducible <= 0) {
            continue;
          }
          const cut = Math.min(reducible, overflow);
          next[key] = Math.round(current - cut);
          overflow -= cut;
        }

        if (
          next.name === prev.name &&
          next.path === prev.path &&
          next.type === prev.type &&
          next.size === prev.size &&
          next.modified === prev.modified
        ) {
          return prev;
        }
        return next;
      });
    };

    fitColumnsToContainer();
    window.addEventListener("resize", fitColumnsToContainer);
    return () => {
      window.removeEventListener("resize", fitColumnsToContainer);
    };
  }, []);

  useEffect(() => {
    const onMouseMove = (event: MouseEvent) => {
      const state = resizeStateRef.current;
      if (!state) {
        return;
      }
      const delta = event.clientX - state.startX;
      const total = state.leftStart + state.rightStart;
      const minLeft = MIN_COLUMN_WIDTHS[state.left];
      const minRight = MIN_COLUMN_WIDTHS[state.right];

      let nextLeft = state.leftStart + delta;
      if (nextLeft < minLeft) {
        nextLeft = minLeft;
      }
      if (nextLeft > total - minRight) {
        nextLeft = total - minRight;
      }
      const nextRight = total - nextLeft;

      setColumnWidths((prev) => ({
        ...prev,
        [state.left]: Math.round(nextLeft),
        [state.right]: Math.round(nextRight)
      }));
    };

    const onMouseUp = () => {
      if (resizeStateRef.current && typeof window !== "undefined") {
        try {
          window.localStorage.setItem(COLUMN_WIDTHS_STORAGE_KEY, JSON.stringify(columnWidthsRef.current));
        } catch {
          // Ignore write failures (e.g. private mode/quota), keep UI functional.
        }
      }
      resizeStateRef.current = null;
      setActiveResizer(null);
      document.body.style.userSelect = "";
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, []);

  const startResize =
    (left: ColumnKey, right: ColumnKey, marker: string) => (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault();
      resizeStateRef.current = {
        left,
        right,
        startX: event.clientX,
        leftStart: columnWidths[left],
        rightStart: columnWidths[right]
      };
      setActiveResizer(marker);
      document.body.style.userSelect = "none";
    };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<BuildEvent>("index://build-status", (event) => {
      const payload = event.payload;
      if (payload.phase === "started") {
        setIsBuilding(true);
        setBuildStatus("Index building...");
      } else {
        setIsBuilding(false);
        if (typeof payload.indexed === "number") {
          setIndexed(payload.indexed);
        }
        const took = typeof payload.tookMs === "number" ? ` in ${payload.tookMs} ms` : "";
        setBuildStatus(`Index build finished${took}`);
      }
    })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch(() => {
        setError("Unable to listen to build events.");
      });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    const persist = () => {
      void invoke("persist_watch_cursor");
    };
    window.addEventListener("beforeunload", persist);
    return () => {
      window.removeEventListener("beforeunload", persist);
    };
  }, []);

  useEffect(() => {
    if (isIndexLoading) {
      setIsSearching(false);
      return;
    }

    let cancelled = false;

    const runSearch = async () => {
      const needle = query.trim();
      if (needle.length === 0) {
        setItems([]);
        setTotalFound(0);
        setTookMs(0);
        return;
      }

      setIsSearching(true);
      setError(null);
      try {
        const response = await invoke<SearchResponse>(
          "search",
          buildSearchRequest(needle, activeTab, pathPrefix, caseSensitive, regexEnabled)
        );
        if (cancelled) {
          return;
        }
        const sorted = sortItems(filterByTab(response.items, activeTab), sortKey, sortAscending);
        setItems(sorted);
        setTotalFound(sorted.length);
        setTookMs(response.tookMs);
      } catch (err) {
        if (!cancelled) {
          setError(String(err));
          setItems([]);
          setTotalFound(0);
        }
      } finally {
        if (!cancelled) {
          setIsSearching(false);
        }
      }
    };

    const timer = window.setTimeout(() => {
      void runSearch();
    }, 180);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query, pathPrefix, activeTab, regexEnabled, caseSensitive, sortKey, sortAscending, isIndexLoading]);

  const runBuild = async (rebuild: boolean) => {
    if (isBuilding) {
      return;
    }
    setError(null);
    setIsBuilding(true);
    setBuildStatus(rebuild ? "Rebuilding index..." : "Building index...");
    try {
      const result = await invoke<BuildResponse>("build_index", {
        path: pathPrefix.trim() || null,
        rebuild,
        includeDirs: true
      });
      setIndexed(result.indexed);
      setBuildStatus(`Indexed ${result.indexed} paths in ${result.tookMs} ms`);
    } catch (err) {
      setError(String(err));
      setBuildStatus("Build failed");
    } finally {
      setIsBuilding(false);
    }
  };

  const toggleWatch = async () => {
    if (isWatchPending) {
      return;
    }
    setError(null);
    setIsWatchPending(true);
    try {
      const command = isWatchRunning ? "stop_watch" : "start_watch_auto";
      const status = await invoke<WatchResponse>(command);
      setIsWatchRunning(status.running);
      setWatchStatus(status.message);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsWatchPending(false);
    }
  };

  const pickPath = async () => {
    if (isPickingPath) {
      return;
    }
    setError(null);
    setIsPickingPath(true);
    try {
      const selected = await invoke<string | null>("pick_path_in_finder");
      if (selected && selected.trim().length > 0) {
        setPathPrefix(selected);
        closePathDropdown();
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsPickingPath(false);
    }
  };

  const openResult = async (path: string) => {
    try {
      await invoke("open_search_result", { path });
    } catch (err) {
      setError(String(err));
    }
  };

  const previewResults = async (paths: string[]) => {
    if (paths.length === 0) {
      return;
    }
    try {
      await invoke("preview_search_result", { paths });
    } catch (err) {
      setPreviewZoom(null);
      previewActiveSessionRef.current = null;
      setError(String(err));
    }
  };

  const revealInFinder = async (path: string) => {
    try {
      await invoke("reveal_in_finder", { path });
    } catch (err) {
      setError(String(err));
    }
  };

  const openInQSpace = async (path: string) => {
    try {
      await invoke("open_in_qspace", { path });
    } catch (err) {
      setError(String(err));
    }
  };

  const openInTerminal = async (path: string) => {
    try {
      await invoke("open_in_terminal", { path });
    } catch (err) {
      setError(String(err));
    }
  };

  const openInWezTerm = async (path: string) => {
    try {
      await invoke("open_in_wezterm", { path });
    } catch (err) {
      setError(String(err));
    }
  };

  const copyText = async (text: string) => {
    try {
      await invoke("copy_to_clipboard", { text });
    } catch (err) {
      setError(String(err));
    }
  };

  const copyAllSelectedNames = async () => {
    await copyText(selectedItemsInOrder.map((item) => item.name).join("\n"));
  };

  const copyAllSelectedPaths = async () => {
    await copyText(selectedItemsInOrder.map((item) => item.path).join("\n"));
  };

  const moveToTrash = async (path: string) => {
    try {
      await invoke("move_to_trash", { path });
    } catch (err) {
      setError(String(err));
    }
  };

  const runContextAction = async (action: () => Promise<void>) => {
    try {
      await action();
    } finally {
      closeContextMenu();
    }
  };

  const toggleHeaderSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortAscending((prev) => !prev);
      return;
    }
    setSortKey(key);
    setSortAscending(true);
  };

  const getRowRect = (path: string): DOMRect | null => {
    const row = rowRefs.current.get(path);
    if (!row) {
      return null;
    }
    const rect = row.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return null;
    }
    return rect;
  };

  const getPreviewTargetRect = (): { left: number; top: number; width: number; height: number } => {
    const width = Math.max(620, Math.min(window.innerWidth * 0.74, 1040));
    const height = Math.max(420, Math.min(window.innerHeight * 0.76, 740));
    return {
      left: Math.round((window.innerWidth - width) / 2),
      top: Math.round((window.innerHeight - height) / 2),
      width: Math.round(width),
      height: Math.round(height)
    };
  };

  const animatePreviewOpen = (path: string) => {
    const source = getRowRect(path);
    if (!source) {
      return;
    }
    const target = getPreviewTargetRect();
    previewSourcePathRef.current = path;
    const id = Date.now();
    setPreviewZoom({
      id,
      left: source.left,
      top: source.top,
      width: source.width,
      height: source.height,
      opacity: 0.92,
      transitionMs: 0
    });
    window.requestAnimationFrame(() => {
      setPreviewZoom((prev) => {
        if (!prev || prev.id !== id) {
          return prev;
        }
        return {
          ...prev,
          left: target.left,
          top: target.top,
          width: target.width,
          height: target.height,
          opacity: 0.28,
          transitionMs: PREVIEW_OPEN_MS
        };
      });
    });
  };

  const animatePreviewClose = () => {
    const current = previewZoomRef.current;
    if (!current) {
      return;
    }
    const sourcePath = previewSourcePathRef.current;
    const destinationRect = sourcePath ? getRowRect(sourcePath) : null;
    const fallback = {
      left: current.left + (current.width - 340) / 2,
      top: current.top + (current.height - 46) / 2,
      width: 340,
      height: 46
    };
    const target = destinationRect
      ? {
          left: destinationRect.left,
          top: destinationRect.top,
          width: destinationRect.width,
          height: destinationRect.height
        }
      : fallback;

    const id = Date.now();
    setPreviewZoom({
      id,
      left: current.left,
      top: current.top,
      width: current.width,
      height: current.height,
      opacity: current.opacity,
      transitionMs: 0
    });
    window.requestAnimationFrame(() => {
      setPreviewZoom((prev) => {
        if (!prev || prev.id !== id) {
          return prev;
        }
        return {
          ...prev,
          left: target.left,
          top: target.top,
          width: target.width,
          height: target.height,
          opacity: 0,
          transitionMs: PREVIEW_CLOSE_MS
        };
      });
    });
    if (previewZoomCleanupRef.current !== null) {
      window.clearTimeout(previewZoomCleanupRef.current);
    }
    previewZoomCleanupRef.current = window.setTimeout(() => {
      setPreviewZoom((prev) => (prev && prev.id === id ? null : prev));
      previewZoomCleanupRef.current = null;
    }, PREVIEW_CLOSE_MS + 30);
  };

  const handleRowClick = (event: React.MouseEvent<HTMLElement>, item: SearchResultItem, index: number) => {
    blurActiveEditable();
    const path = item.path;
    const isMetaMulti = event.metaKey;

    if (event.shiftKey) {
      const anchorPath = selectionAnchorPath ?? selectedPathsInOrder[0] ?? path;
      const anchorIndex = items.findIndex((entry) => entry.path === anchorPath);
      if (anchorIndex < 0) {
        setSelectedItemPaths([path]);
        setSelectionAnchorPath(path);
        return;
      }

      const rangeStart = Math.min(anchorIndex, index);
      const rangeEnd = Math.max(anchorIndex, index);
      const rangePaths = items.slice(rangeStart, rangeEnd + 1).map((entry) => entry.path);

      if (isMetaMulti) {
        const merged = new Set(selectedPathsInOrder);
        for (const p of rangePaths) {
          merged.add(p);
        }
        setSelectedItemPaths(Array.from(merged));
      } else {
        setSelectedItemPaths(rangePaths);
      }
      setSelectionAnchorPath(path);
      return;
    }

    if (isMetaMulti) {
      if (selectedItemPathSet.has(path)) {
        const next = selectedItemPaths.filter((p) => p !== path);
        setSelectedItemPaths(next);
        setSelectionAnchorPath(next.length > 0 ? next[next.length - 1] : null);
      } else {
        setSelectedItemPaths([...selectedItemPaths, path]);
        setSelectionAnchorPath(path);
      }
      return;
    }

    setSelectedItemPaths([path]);
    setSelectionAnchorPath(path);
  };

  const moveSelectionByArrow = (delta: number) => {
    if (items.length === 0) {
      return;
    }

    const anchorPath = selectionAnchorPath ?? selectedPathsInOrder[0] ?? null;
    const anchorIndex = anchorPath ? items.findIndex((entry) => entry.path === anchorPath) : -1;
    const startIndex = anchorIndex >= 0 ? anchorIndex : delta > 0 ? -1 : items.length;
    const nextIndex = Math.max(0, Math.min(items.length - 1, startIndex + delta));
    const nextPath = items[nextIndex].path;

    setSelectedItemPaths([nextPath]);
    setSelectionAnchorPath(nextPath);

    window.requestAnimationFrame(() => {
      rowRefs.current.get(nextPath)?.scrollIntoView({ block: "nearest" });
    });
  };

  const openResultContextMenu = (event: React.MouseEvent<HTMLElement>, item: SearchResultItem) => {
    event.preventDefault();
    blurActiveEditable();
    const menuWidth = 230;
    const keepsMultiSelection = selectedItemPathSet.has(item.path);
    const menuIsMulti = keepsMultiSelection && hasMultiSelection;
    const menuHeight = menuIsMulti ? 392 : 332;
    const x = Math.max(8, Math.min(event.clientX, window.innerWidth - menuWidth - 8));
    const y = Math.max(8, Math.min(event.clientY, window.innerHeight - menuHeight - 8));
    if (!selectedItemPathSet.has(item.path)) {
      setSelectedItemPaths([item.path]);
      setSelectionAnchorPath(item.path);
    }
    setContextMenu({
      x,
      y,
      item,
      multiSelection: menuIsMulti
    });
    setOpenWithVisible(false);
  };

  useEffect(() => {
    let unlistenPreview: (() => void) | undefined;
    void listen<PreviewStatusEvent>("preview://status", (event) => {
      const payload = event.payload;
      if (payload.phase === "opened") {
        previewActiveSessionRef.current = payload.sessionId;
        return;
      }
      if (previewActiveSessionRef.current !== payload.sessionId) {
        return;
      }
      previewActiveSessionRef.current = null;
      animatePreviewClose();
    })
      .then((dispose) => {
        unlistenPreview = dispose;
      })
      .catch(() => {
        // Keep app usable even if preview status event binding fails.
      });

    return () => {
      if (unlistenPreview) {
        unlistenPreview();
      }
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.key === "ArrowDown" || event.key === "ArrowUp") && activeView === "search" && !contextMenu) {
        if (!isEditableTarget(event.target)) {
          event.preventDefault();
          moveSelectionByArrow(event.key === "ArrowDown" ? 1 : -1);
        }
        return;
      }

      if (event.key !== " " && event.code !== "Space") {
        return;
      }
      if (event.repeat || activeView !== "search" || selectedPathsInOrder.length === 0 || contextMenu) {
        return;
      }
      if (isEditableTarget(event.target)) {
        return;
      }
      event.preventDefault();
      const leadPath = selectedPathsInOrder[0];
      blurActiveEditable();
      previewActiveSessionRef.current = null;
      animatePreviewOpen(leadPath);
      void previewResults(selectedPathsInOrder);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [activeView, contextMenu, selectedPathsInOrder]);

  const settingsThemeOptions: Array<{ mode: ThemeMode; title: string; description: string }> = [
    { mode: "system", title: t.themeSystemTitle, description: t.themeSystemDesc },
    { mode: "light", title: t.themeLightTitle, description: t.themeLightDesc },
    { mode: "dark", title: t.themeDarkTitle, description: t.themeDarkDesc }
  ];

  const settingsLanguageOptions: Array<{ code: Language; title: string; description: string }> = [
    { code: "zh", title: t.languageZhTitle, description: t.languageZhDesc },
    { code: "en", title: t.languageEnTitle, description: t.languageEnDesc }
  ];

  const previewZoomStyle = previewZoom
    ? {
        left: `${previewZoom.left}px`,
        top: `${previewZoom.top}px`,
        width: `${previewZoom.width}px`,
        height: `${previewZoom.height}px`,
        opacity: previewZoom.opacity,
        transition:
          previewZoom.transitionMs > 0
            ? `left ${previewZoom.transitionMs}ms cubic-bezier(0.2, 0.85, 0.16, 1), top ${previewZoom.transitionMs}ms cubic-bezier(0.2, 0.85, 0.16, 1), width ${previewZoom.transitionMs}ms cubic-bezier(0.2, 0.85, 0.16, 1), height ${previewZoom.transitionMs}ms cubic-bezier(0.2, 0.85, 0.16, 1), opacity ${previewZoom.transitionMs}ms ease-out`
            : "none"
      }
    : undefined;

  return (
    <div className="app-background">
      <main className={activeView === "search" ? "app-shell" : "app-shell app-shell-settings"}>
        {activeView === "search" ? (
          <>
            <section className="search-panel">
              <div className="search-input-wrap">
                <input
                  className="search-input"
                  placeholder={t.searchPlaceholder}
                  value={query}
                  disabled={isIndexLoading}
                  autoComplete="off"
                  autoCorrect="off"
                  autoCapitalize="off"
                  spellCheck={false}
                  onChange={(event) => setQuery(event.target.value)}
                />
                <span className="search-shortcut">{t.searchTag}</span>
              </div>

              <div className="filter-row">
                <div className="path-picker" ref={pathPickerRef}>
                  <div className="path-input-wrap">
                    <input
                      ref={pathInputRef}
                      className="path-input"
                      placeholder={t.pathPlaceholder}
                      value={pathPrefix}
                      disabled={isIndexLoading}
                      autoComplete="off"
                      autoCorrect="off"
                      autoCapitalize="off"
                      spellCheck={false}
                      onFocus={() => {
                        if (!isIndexLoading) {
                          setIsPathDropdownOpen(true);
                        }
                      }}
                      onBlur={(event) => {
                        const next = event.relatedTarget;
                        if (next instanceof Node && pathPickerRef.current?.contains(next)) {
                          return;
                        }
                        closePathDropdown();
                      }}
                      onKeyDown={handlePathInputKeyDown}
                      onChange={(event) => {
                        setPathPrefix(event.target.value);
                        setIsPathDropdownOpen(true);
                        setActivePathSuggestion(-1);
                      }}
                    />
                    {isPathDropdownVisible && (
                      <div className="path-suggest-panel">
                        {visiblePathSuggestions.map((path, index) => (
                          <button
                            key={path}
                            type="button"
                            className={index === activePathSuggestion ? "path-suggest-item active" : "path-suggest-item"}
                            onMouseEnter={() => setActivePathSuggestion(index)}
                            onMouseDown={(event) => {
                              event.preventDefault();
                              applyPathSuggestion(path);
                            }}
                          >
                            <span className="path-suggest-icon">»</span>
                            <span className="path-suggest-text">{path}</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                  <button
                    className="action-btn path-pick-btn"
                    onClick={() => void pickPath()}
                    disabled={isPickingPath || isIndexLoading}
                  >
                    {t.choosePath}
                  </button>
                </div>

                <div className="mode-controls">
                  <button
                    className={regexEnabled ? "case-btn active" : "case-btn"}
                    onClick={() => setRegexEnabled((prev) => !prev)}
                    title={t.regexEnabled}
                  >
                    {t.regexEnabled}
                  </button>
                  <button
                    className={caseSensitive ? "case-btn active" : "case-btn"}
                    onClick={() => setCaseSensitive((prev) => !prev)}
                    title={t.caseSensitive}
                  >
                    Aa
                  </button>
                </div>

                <div className="actions">
                  <button className="action-btn" onClick={() => void runBuild(false)} disabled={isBuilding}>
                    {t.build}
                  </button>
                  <button className="action-btn" onClick={() => void runBuild(true)} disabled={isBuilding}>
                    {t.rebuild}
                  </button>
                  <button
                    className={isWatchRunning ? "action-btn watch-stop" : "action-btn primary"}
                    onClick={() => void toggleWatch()}
                    disabled={isWatchPending}
                  >
                    <span className={isWatchRunning ? "watch-dot stop" : "watch-dot start"} />
                    {isWatchPending ? (isWatchRunning ? t.stopping : t.starting) : isWatchRunning ? t.stopWatch : t.startWatch}
                  </button>
                </div>
              </div>
            </section>

            <section className="tabs-row">
              <div className="tabs">
                {TAB_IDS.map((tab) => (
                  <button
                    key={tab}
                    className={tab === activeTab ? "tab-btn active" : "tab-btn"}
                    onClick={() => setActiveTab(tab)}
                  >
                    {tabLabel(tab)}
                  </button>
                ))}
              </div>
            </section>

            <section className="table-shell" ref={tableShellRef}>
              <div className="table-header" style={{ gridTemplateColumns }}>
                <span className="header-cell">
                  <button type="button" className="header-sort-btn" onClick={() => toggleHeaderSort("name")}>
                    <span className="header-sort-label">{t.header_name}</span>
                    {sortKey === "name" && <span className="header-sort-indicator">{sortAscending ? "▲" : "▼"}</span>}
                  </button>
                  <span
                    className={activeResizer === "name-path" ? "column-resizer active" : "column-resizer"}
                    onMouseDown={startResize("name", "path", "name-path")}
                  />
                </span>
                <span className="header-cell">
                  <button type="button" className="header-sort-btn" onClick={() => toggleHeaderSort("path")}>
                    <span className="header-sort-label">{t.header_path}</span>
                    {sortKey === "path" && <span className="header-sort-indicator">{sortAscending ? "▲" : "▼"}</span>}
                  </button>
                  <span
                    className={activeResizer === "path-type" ? "column-resizer active" : "column-resizer"}
                    onMouseDown={startResize("path", "type", "path-type")}
                  />
                </span>
                <span className="header-cell">
                  <button type="button" className="header-sort-btn" onClick={() => toggleHeaderSort("type")}>
                    <span className="header-sort-label">{t.header_type}</span>
                    {sortKey === "type" && <span className="header-sort-indicator">{sortAscending ? "▲" : "▼"}</span>}
                  </button>
                  <span
                    className={activeResizer === "type-size" ? "column-resizer active" : "column-resizer"}
                    onMouseDown={startResize("type", "size", "type-size")}
                  />
                </span>
                <span className="header-cell">
                  <button type="button" className="header-sort-btn" onClick={() => toggleHeaderSort("size")}>
                    <span className="header-sort-label">{t.header_size}</span>
                    {sortKey === "size" && <span className="header-sort-indicator">{sortAscending ? "▲" : "▼"}</span>}
                  </button>
                  <span
                    className={activeResizer === "size-modified" ? "column-resizer active" : "column-resizer"}
                    onMouseDown={startResize("size", "modified", "size-modified")}
                  />
                </span>
                <span className="header-cell">
                  <button type="button" className="header-sort-btn" onClick={() => toggleHeaderSort("modified")}>
                    <span className="header-sort-label">{t.header_modified}</span>
                    {sortKey === "modified" && <span className="header-sort-indicator">{sortAscending ? "▲" : "▼"}</span>}
                  </button>
                </span>
              </div>

              <div className="table-body">
                {items.map((item, index) => {
                  const token = iconToken(item);
                  return (
                    <article
                      key={`${item.path}-${index}`}
                      ref={(element) => {
                        if (element) {
                          rowRefs.current.set(item.path, element);
                        } else {
                          rowRefs.current.delete(item.path);
                        }
                      }}
                      className={selectedItemPathSet.has(item.path) ? "row selected" : "row"}
                      style={{ gridTemplateColumns }}
                      onMouseDown={(event) => {
                        if (event.button === 0) {
                          blurActiveEditable();
                        }
                      }}
                      onClick={(event) => handleRowClick(event, item, index)}
                      onDoubleClick={() => void openResult(item.path)}
                      onContextMenu={(event) => openResultContextMenu(event, item)}
                    >
                      <div className="cell name-cell">
                        <span className={`file-icon ${token}`}>{iconGlyph(token)}</span>
                        <span
                          className="name-text"
                          onMouseEnter={(event) => setCellPreviewTooltip(event, item.name)}
                          onMouseLeave={(event) => event.currentTarget.removeAttribute("title")}
                        >
                          {item.name}
                        </span>
                      </div>
                      <div
                        className="cell path-cell"
                        onMouseEnter={(event) => setCellPreviewTooltip(event, item.parent)}
                        onMouseLeave={(event) => event.currentTarget.removeAttribute("title")}
                      >
                        {item.parent}
                      </div>
                      <div className="cell type-cell">{typeLabel(item, language)}</div>
                      <div className="cell">{formatBytes(item.sizeBytes)}</div>
                      <div className="cell">{formatDate(item.modifiedUnixMs)}</div>
                    </article>
                  );
                })}

                {items.length === 0 && (
                  <div className="empty-state">
                    {query.trim().length === 0
                      ? t.emptyTypeHint
                      : t.emptyNoMatch}
                  </div>
                )}
              </div>
            </section>

            <footer className="status-bar">
              <div className="status-left">
                <span>{formatIndexedItems(indexed)}</span>
                <span>{buildStatus}</span>
                <span>{watchStatus}</span>
              </div>
              <div className="status-right">
                <span>{formatShownItems(totalFound)}</span>
                <span>{isSearching ? t.searching : `${tookMs} ms`}</span>
              </div>
            </footer>
          </>
        ) : (
          <section className="settings-page">
            <header className="settings-header">
              <div>
                <h2>{t.settingsTitle}</h2>
                <p>{t.settingsDesc}</p>
              </div>
              <button className="action-btn primary" onClick={() => setActiveView("search")}>
                {t.backToSearch}
              </button>
            </header>

            <article className="settings-card">
              <h3>{t.themeModeTitle}</h3>
              <div className="theme-select-list">
                {settingsThemeOptions.map((option) => (
                  <button
                    key={option.mode}
                    className={themeMode === option.mode ? "theme-option active" : "theme-option"}
                    onClick={() => setThemeMode(option.mode)}
                  >
                    <div className="theme-option-title">{option.title}</div>
                    <div className="theme-option-desc">{option.description}</div>
                  </button>
                ))}
              </div>
              <div className="theme-preview-line">
                {t.themeCurrent}: {resolvedTheme === "dark" ? t.themeDarkTitle : t.themeLightTitle}
              </div>
            </article>

            <article className="settings-card">
              <h3>{t.languageTitle}</h3>
              <div className="theme-select-list">
                {settingsLanguageOptions.map((option) => (
                  <button
                    key={option.code}
                    className={language === option.code ? "theme-option active" : "theme-option"}
                    onClick={() => setLanguage(option.code)}
                  >
                    <div className="theme-option-title">{option.title}</div>
                    <div className="theme-option-desc">{option.description}</div>
                  </button>
                ))}
              </div>
            </article>
          </section>
        )}

        {error && <aside className="error-box">{error}</aside>}
      </main>
      {previewZoom && <div className="preview-zoom" style={previewZoomStyle} />}
      {contextMenu && (
        <div
          className="context-menu-layer"
          onMouseDown={closeContextMenu}
          onContextMenu={(event) => event.preventDefault()}
        >
          <div
            className="context-menu"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            onMouseDown={(event) => event.stopPropagation()}
            onContextMenu={(event) => event.preventDefault()}
          >
            <button
              className="context-menu-btn"
              onClick={() => void runContextAction(() => openResult(contextMenu.item.path))}
            >
              {t.menuOpen}
            </button>

            <div
              className="context-submenu-wrap"
              onMouseEnter={openOpenWithMenu}
              onMouseLeave={scheduleCloseOpenWithMenu}
            >
              <button className="context-menu-btn">
                <span>{t.menuOpenWith}</span>
                <span className="context-menu-arrow">›</span>
              </button>
              {openWithVisible && (
                <div
                  className="context-submenu"
                  onMouseEnter={openOpenWithMenu}
                  onMouseLeave={scheduleCloseOpenWithMenu}
                >
                  <button
                    className="context-menu-btn"
                    onClick={() => void runContextAction(() => revealInFinder(contextMenu.item.path))}
                  >
                    {t.menuFinder}
                  </button>
                  <button
                    className="context-menu-btn"
                    onClick={() => void runContextAction(() => openInQSpace(contextMenu.item.path))}
                  >
                    {t.menuQSpace}
                  </button>
                  <button
                    className="context-menu-btn"
                    onClick={() => void runContextAction(() => openInTerminal(contextMenu.item.path))}
                  >
                    {t.menuTerminal}
                  </button>
                  <button
                    className="context-menu-btn"
                    onClick={() => void runContextAction(() => openInWezTerm(contextMenu.item.path))}
                  >
                    {t.menuWezTerm}
                  </button>
                </div>
              )}
            </div>

            <div className="context-menu-sep" />

            <button
              className="context-menu-btn"
              onClick={() => void runContextAction(() => copyText(contextMenu.item.name))}
            >
              {t.menuCopyName}
            </button>
            <button
              className="context-menu-btn"
              onClick={() => void runContextAction(() => copyText(contextMenu.item.path))}
            >
              {t.menuCopyPath}
            </button>
            {contextMenu.multiSelection && (
              <>
                <button
                  className="context-menu-btn"
                  onClick={() => void runContextAction(copyAllSelectedNames)}
                >
                  {t.menuCopyAllNames}
                </button>
                <button
                  className="context-menu-btn"
                  onClick={() => void runContextAction(copyAllSelectedPaths)}
                >
                  {t.menuCopyAllPaths}
                </button>
              </>
            )}
            <button
              className="context-menu-btn danger"
              onClick={() => void runContextAction(() => moveToTrash(contextMenu.item.path))}
            >
              {t.menuTrash}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
