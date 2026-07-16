import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

import { I18N } from "./i18n";
import type {
  TabId, SortKey, ColumnKey, ThemeMode, ViewMode, VolumeEventType,
  Language, ExcludeRuleType, SearchResultItem, ContextMenuState,
  SearchResponse, InitResponse, BuildResponse, BuildEvent, WatchResponse,
  LaunchSettingsResponse, AutoVacuumSettingsResponse, ExcludeDirSettingsResponse,
  WatchRootsSettingsResponse, FileManagerSettingsResponse,
} from "./types";
import {
  DEFAULT_WINDOW_TOGGLE_SHORTCUT, DEFAULT_COLUMN_WIDTHS, COLUMN_KEYS,
  EVENT_OPEN_SETTINGS, EVENT_FOCUS_SEARCH,
  loadStoredTheme, detectDefaultLanguage, systemPrefersDark, resolveTheme,
  loadStoredRegexEnabled, loadStoredCaseSensitive, loadStoredFuzzyEnabled,
  loadPinnedItems, savePinnedItems, loadStoredColumnWidths,
  buildSearchRequest, displayShortcut, shortcutFromKeyboardEvent,
  fmt, isEditableTarget, blurActiveEditable, extensionOf, appForExt,
  COLUMN_WIDTHS_STORAGE_KEY, LANGUAGE_STORAGE_KEY, REGEX_ENABLED_STORAGE_KEY,
  CASE_SENSITIVE_STORAGE_KEY, FUZZY_ENABLED_STORAGE_KEY, THEME_STORAGE_KEY,
  PINNED_STORAGE_KEY,
  iconToken, iconGlyph, typeLabel, formatBytes, formatDate,
} from "./utils";
import { SearchView } from "./components/SearchView";
import { SettingsView } from "./components/SettingsView";
import { ContextMenu } from "./components/ContextMenu";

const ROW_HEIGHT = 46;
const VISIBLE_BUFFER = 10;

function App() {
  // ── core state ──
  const [activeView, setActiveView] = useState<ViewMode>("search");
  const [themeMode, setThemeMode] = useState<ThemeMode>(loadStoredTheme);
  const [systemDark, setSystemDark] = useState(systemPrefersDark());
  const [language, setLanguage] = useState<Language>(detectDefaultLanguage());
  const resolvedTheme = resolveTheme(themeMode, systemDark);
  const t = I18N[language];

  // ── shortcut state ──
  const [windowToggleShortcut, setWindowToggleShortcut] = useState(DEFAULT_WINDOW_TOGGLE_SHORTCUT);
  const [shortcutDraft, setShortcutDraft] = useState(DEFAULT_WINDOW_TOGGLE_SHORTCUT);
  const [shortcutStatus, setShortcutStatus] = useState("");
  const [isShortcutSaving, setIsShortcutSaving] = useState(false);

  // ── launch settings ──
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [silentStart, setSilentStart] = useState(false);
  const [showDockIcon, setShowDockIcon] = useState(false);
  const [isLaunchSettingsSaving, setIsLaunchSettingsSaving] = useState(false);
  const [launchSettingsStatus, setLaunchSettingsStatus] = useState("");

  // ── auto vacuum ──
  const [autoVacuumOnRebuild, setAutoVacuumOnRebuild] = useState(true);
  const [isAutoVacuumSettingsSaving, setIsAutoVacuumSettingsSaving] = useState(false);
  const [autoVacuumSettingsStatus, setAutoVacuumSettingsStatus] = useState("");

  // ── update check ──
  const [autoCheckUpdate, setAutoCheckUpdate] = useState(true);
  const [isAutoCheckSaving, setIsAutoCheckSaving] = useState(false);
  const [autoCheckStatus, setAutoCheckStatus] = useState("");
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<{ hasUpdate: boolean; latestVersion: string } | null>(null);

  // ── max results ──
  const [maxResults, setMaxResults] = useState(500);
  const [isMaxResultsSaving, setIsMaxResultsSaving] = useState(false);
  const [maxResultsStatus, setMaxResultsStatus] = useState("");

  // ── exclude dirs ──
  const [excludeRuleType, setExcludeRuleType] = useState<ExcludeRuleType>("exact");
  const [excludeRuleDraft, setExcludeRuleDraft] = useState("");
  const [excludeExactDirs, setExcludeExactDirs] = useState<string[]>([]);
  const [excludePatternDirs, setExcludePatternDirs] = useState<string[]>([]);
  const [excludeDirStatus, setExcludeDirStatus] = useState("");
  const [isExcludeDirSaving, setIsExcludeDirSaving] = useState(false);

  // ── watch roots ──
  const [watchRootDraft, setWatchRootDraft] = useState("");
  const [watchRoots, setWatchRoots] = useState<string[]>([]);
  const [watchRootStatus, setWatchRootStatus] = useState("");
  const [isWatchRootSaving, setIsWatchRootSaving] = useState(false);

  // ── search state ──
  const [query, setQuery] = useState("");
  const [pathPrefix, setPathPrefix] = useState("");
  const [pathSuggestions, setPathSuggestions] = useState<string[]>([]);
  const [isPathDropdownOpen, setIsPathDropdownOpen] = useState(false);
  const [activePathSuggestion, setActivePathSuggestion] = useState(-1);
  const [regexEnabled, setRegexEnabled] = useState(() => loadStoredRegexEnabled() ?? false);
  const [caseSensitive, setCaseSensitive] = useState(() => loadStoredCaseSensitive() ?? false);
  const [fuzzyEnabled, setFuzzyEnabled] = useState(() => loadStoredFuzzyEnabled() ?? false);
  const [activeTab, setActiveTab] = useState<TabId>("all");
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortAscending, setSortAscending] = useState(true);

  // ── filter state ──
  const [timeFilter, setTimeFilter] = useState("all");
  const [sizeFilter, setSizeFilter] = useState("all");
  const [customSizeMin, setCustomSizeMinTemp] = useState("");
  const [customSizeMax, setCustomSizeMaxTemp] = useState("");
  const [customSizeUnit, setCustomSizeUnit] = useState("MB");
  const [appFilter, setAppFilter] = useState("");
  const [showTimePopover, setShowTimePopover] = useState(false);
  const [showSizePopover, setShowSizePopover] = useState(false);
  const timePopoverRef = useRef<HTMLDivElement | null>(null);
  const sizePopoverRef = useRef<HTMLDivElement | null>(null);
  const customSizeMinRef = useRef(0);
  const customSizeMaxRef = useRef(0);
  const customTimeFromRef = useRef(0);
  const customTimeToRef = useRef(0);
  const [filterVersion, setFilterVersion] = useState(0);

  // ── calendar state ──
  const [calMonth, setCalMonth] = useState(() => new Date().getMonth());
  const [calYear, setCalYear] = useState(() => new Date().getFullYear());
  const [calSelecting, setCalSelecting] = useState<"from" | "to">("from");
  const [calFrom, setCalFrom] = useState("");
  const [calTo, setCalTo] = useState("");

  // ── results state ──
  const [items, setItems] = useState<SearchResultItem[]>([]);
  const itemsRef = useRef(items);
  itemsRef.current = items;
  const [pinnedItems, setPinnedItems] = useState<SearchResultItem[]>(loadPinnedItems);
  const [indexed, setIndexed] = useState(0);
  const [appVersion, setAppVersion] = useState("");
  const [totalFound, setTotalFound] = useState(0);
  const [tookMs, setTookMs] = useState(0);
  const [buildStatus, setBuildStatus] = useState("");
  const [isWatchRunning, setIsWatchRunning] = useState(false);
  const [isWatchPending, setIsWatchPending] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [isIndexLoading, setIsIndexLoading] = useState(true);
  const [isBuilding, setIsBuilding] = useState(false);
  const [isPickingPath, setIsPickingPath] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ── file manager ──
  const [defaultFolderAction, setDefaultFolderAction] = useState("Finder");
  const [defaultTerminalAction, setDefaultTerminalAction] = useState("Terminal");
  const [customFolderApp, setCustomFolderApp] = useState("");
  const [customTerminalApp, setCustomTerminalApp] = useState("");

  // ── context menu ──
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [openWithVisible, setOpenWithVisible] = useState(false);
  const openWithCloseTimerRef = useRef<number | null>(null);
  const isPreviewingRef = useRef(false);

  // ── selection ──
  const [selectedItemPaths, setSelectedItemPaths] = useState<string[]>([]);
  const [selectionAnchorPath, setSelectionAnchorPath] = useState<string | null>(null);

  // ── column resize ──
  const [scrollTop, setScrollTop] = useState(0);
  const [columnWidths, setColumnWidths] = useState<Record<ColumnKey, number>>(
    () => loadStoredColumnWidths() ?? DEFAULT_COLUMN_WIDTHS
  );
  const [activeResizer, setActiveResizer] = useState<string | null>(null);
  const columnWidthsRef = useRef(columnWidths);
  const resizeStateRef = useRef<{
    left: ColumnKey;
    right: ColumnKey;
    startX: number;
    startWidths: Record<ColumnKey, number>;
  } | null>(null);

  // ── refs ──
  const tableShellRef = useRef<HTMLDivElement | null>(null);
  const tableBodyRef = useRef<HTMLDivElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const pathPickerRef = useRef<HTMLDivElement | null>(null);
  const pathInputRef = useRef<HTMLInputElement | null>(null);
  const rowRefs = useRef(new Map<string, HTMLElement>());

  // ── derived ──
  const gridTemplateColumns = `${columnWidths.name}px ${columnWidths.path}px ${columnWidths.type}px ${columnWidths.size}px ${columnWidths.modified}px`;
  const normalizedPathPrefix = pathPrefix.trim().toLowerCase();
  const visiblePathSuggestions = [...pathSuggestions]
    .filter((path) => {
      if (!normalizedPathPrefix) return true;
      if (path.startsWith("/Volumes/")) return true;
      if (path.startsWith("/Users/") && path.split("/").length <= 3) return true;
      return path.toLowerCase().includes(normalizedPathPrefix);
    })
    .sort((left, right) => {
      const leftLower = left.toLowerCase();
      const rightLower = right.toLowerCase();
      const leftChild = normalizedPathPrefix.length > 0 && leftLower.startsWith(normalizedPathPrefix) && leftLower.length > normalizedPathPrefix.length;
      const rightChild = normalizedPathPrefix.length > 0 && rightLower.startsWith(normalizedPathPrefix) && rightLower.length > normalizedPathPrefix.length;
      if (leftChild !== rightChild) return leftChild ? -1 : 1;
      const leftRoot = left.startsWith("/Volumes/") || (left.startsWith("/Users/") && left.split("/").length <= 3);
      const rightRoot = right.startsWith("/Volumes/") || (right.startsWith("/Users/") && right.split("/").length <= 3);
      if (leftRoot !== rightRoot) return leftRoot ? -1 : 1;
      if (left.length !== right.length) return left.length - right.length;
      return left.localeCompare(right);
    })
    .slice(0, 8);
  const isPathDropdownVisible = !isIndexLoading && isPathDropdownOpen && visiblePathSuggestions.length > 0;
  const selectedItemPathSet = useMemo(() => new Set(selectedItemPaths), [selectedItemPaths]);
  const selectedItemsInOrder = useMemo(
    () => {
      const source = activeView === "pinned" ? pinnedItems : items;
      return source.filter((item) => selectedItemPathSet.has(item.path));
    },
    [items, pinnedItems, selectedItemPathSet, activeView]
  );
  const selectedPathsInOrder = useMemo(
    () => selectedItemsInOrder.map((item) => item.path),
    [selectedItemsInOrder]
  );
  const hasMultiSelection = selectedPathsInOrder.length > 1;

  // ── filtered items ──
  const filteredItems = useMemo(() => {
    let list = items;
    if (timeFilter !== "all") {
      const now = Date.now();
      const cutoff = timeFilter === "custom"
        ? 0
        : now - ({
            today: 86400000,
            week: 604800000,
            month: 2592000000,
            year: 31536000000,
          }[timeFilter] ?? 0);
      list = list.filter((item) => {
        const m = item.modifiedUnixMs;
        if (m == null) return false;
        if (timeFilter === "custom") {
          const from = customTimeFromRef.current;
          const to = customTimeToRef.current;
          return (from === 0 || m >= from) && (to === 0 || m <= to);
        }
        return m >= cutoff;
      });
    }
    if (sizeFilter !== "all") {
      list = list.filter((item) => {
        const b = item.sizeBytes;
        if (b == null) return false;
        switch (sizeFilter) {
          case "kb": return b < 1048576;
          case "mb": return b >= 1048576 && b < 104857600;
          case "gb": return b >= 104857600;
          case "custom": {
            const min = customSizeMinRef.current;
            const max = customSizeMaxRef.current;
            return (min <= 0 || b >= min) && (max <= 0 || b <= max);
          }
          default: return true;
        }
      });
    }
    if (appFilter !== "") {
      list = list.filter((item) => {
        if (item.isDir) return false;
        return appForExt(extensionOf(item.name)) === appFilter;
      });
    }
    return list;
  }, [items, timeFilter, sizeFilter, appFilter, filterVersion]);

  const visibleStart = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - VISIBLE_BUFFER);
  const visibleEnd = Math.min(filteredItems.length, visibleStart + Math.ceil(window.innerHeight / ROW_HEIGHT) + VISIBLE_BUFFER * 2);
  const visibleItems = filteredItems.slice(visibleStart, visibleEnd);
  const topSpacerHeight = visibleStart * ROW_HEIGHT;
  const bottomSpacerHeight = (filteredItems.length - visibleEnd) * ROW_HEIGHT;

  const sortedPinnedItems = useMemo(() => {
    const sorted = [...pinnedItems];
    const dir = sortAscending ? 1 : -1;
    sorted.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case "name": cmp = a.name.localeCompare(b.name); break;
        case "path": cmp = (a.parent || "").localeCompare(b.parent || ""); if (cmp === 0) cmp = a.name.localeCompare(b.name); break;
        case "type": {
          const extA = a.name.includes(".") ? a.name.slice(a.name.lastIndexOf(".") + 1).toLowerCase() : "";
          const extB = b.name.includes(".") ? b.name.slice(b.name.lastIndexOf(".") + 1).toLowerCase() : "";
          cmp = extA.localeCompare(extB);
          if (cmp === 0) cmp = a.name.localeCompare(b.name);
          break;
        }
        case "size": cmp = (a.sizeBytes ?? 0) - (b.sizeBytes ?? 0); break;
        case "modified": cmp = (a.modifiedUnixMs ?? 0) - (b.modifiedUnixMs ?? 0); break;
      }
      return cmp * dir;
    });
    return sorted;
  }, [pinnedItems, sortKey, sortAscending]);

  const customTimeLabel = useMemo(() => {
    if (timeFilter !== "custom") return null;
    const from = customTimeFromRef.current;
    const to = customTimeToRef.current;
    if (!from && !to) return null;
    const fmtDate = (ts: number) => {
      const d = new Date(ts);
      return `${d.getFullYear()}/${d.getMonth()+1}/${d.getDate()}`;
    };
    let label = "";
    if (from) label += fmtDate(from);
    label += " ~ ";
    if (to) label += fmtDate(to - 86399999);
    return label;
  }, [timeFilter, showTimePopover, filterVersion]);

  const customSizeLabel = useMemo(() => {
    if (sizeFilter !== "custom" || showSizePopover) return null;
    const min = customSizeMinRef.current;
    const max = customSizeMaxRef.current;
    if (!min && !max) return null;
    const fmtNum = (v: number) => Number.isInteger(v) ? v.toString() : v.toFixed(1);
    const fmtBytes = (b: number) => {
      if (b >= 1073741824) return `${fmtNum(b/1073741824)} GB`;
      if (b >= 1048576) return `${fmtNum(b/1048576)} MB`;
      if (b >= 1024) return `${fmtNum(b/1024)} KB`;
      return `${b} B`;
    };
    let label = "";
    if (min) label += fmtBytes(min);
    label += " ~ ";
    if (max) label += fmtBytes(max);
    return label;
  }, [sizeFilter, showSizePopover, filterVersion]);

  // ── app filter options ──
  const appFilterOptions = useMemo(() => {
    const apps = new Set<string>();
    for (const item of items) {
      if (!item.isDir) apps.add(appForExt(extensionOf(item.name)));
    }
    return Array.from(apps).sort();
  }, [items]);

  // ── context menu helpers ──
  const clearOpenWithCloseTimer = () => {
    if (openWithCloseTimerRef.current !== null) {
      window.clearTimeout(openWithCloseTimerRef.current);
      openWithCloseTimerRef.current = null;
    }
  };
  const openOpenWithMenu = () => { clearOpenWithCloseTimer(); setOpenWithVisible(true); };
  const scheduleCloseOpenWithMenu = () => {
    clearOpenWithCloseTimer();
    openWithCloseTimerRef.current = window.setTimeout(() => { setOpenWithVisible(false); openWithCloseTimerRef.current = null; }, 220);
  };
  const closeContextMenu = () => { clearOpenWithCloseTimer(); setContextMenu(null); setOpenWithVisible(false); };

  const closePathDropdown = () => { setIsPathDropdownOpen(false); setActivePathSuggestion(-1); };
  const applyPathSuggestion = (path: string) => { setPathPrefix(path); closePathDropdown(); pathInputRef.current?.focus(); };

  // ── isPinned / togglePin ──
  const isPinned = useCallback((path: string) => pinnedItems.some((item) => item.path === path), [pinnedItems]);
  const togglePin = useCallback((item: SearchResultItem) => {
    setPinnedItems((prev) => {
      const already = prev.some((p) => p.path === item.path);
      const next = already ? prev.filter((p) => p.path !== item.path) : [...prev, item];
      savePinnedItems(next);
      return next;
    });
  }, []);

  // ── scrollbar ──
  const scrollTimers = useRef(new Map<HTMLElement, ReturnType<typeof setTimeout>>());
  const handleScrollbarScroll = (e: React.UIEvent<HTMLElement>) => {
    const el = e.currentTarget;
    el.classList.add("scrolling");
    const prev = scrollTimers.current.get(el);
    if (prev) clearTimeout(prev);
    scrollTimers.current.set(el, setTimeout(() => { el.classList.remove("scrolling"); scrollTimers.current.delete(el); }, 600));
  };

  // ── actions ──
  const openResult = async (path: string) => { try { await invoke("open_search_result", { path }); } catch (err) { setError(String(err)); } };
  const revealInFinder = async (path: string) => { try { await invoke("reveal_in_finder", { path }); } catch (err) { setError(String(err)); } };
  const openInQSpace = async (path: string) => { try { await invoke("open_in_qspace", { path }); } catch (err) { setError(String(err)); } };
  const openInTerminal = async (path: string) => { try { await invoke("open_in_terminal", { path }); } catch (err) { setError(String(err)); } };
  const openInWezTerm = async (path: string) => { try { await invoke("open_in_wezterm", { path }); } catch (err) { setError(String(err)); } };
  const moveToTrash = async (path: string) => { try { await invoke("move_to_trash", { path }); } catch (err) { setError(String(err)); } };

  const copyViaExecCommand = (text: string): boolean => {
    const textarea = document.createElement("textarea");
    textarea.value = text; textarea.setAttribute("readonly", ""); textarea.style.position = "fixed"; textarea.style.top = "0"; textarea.style.left = "-9999px"; textarea.style.opacity = "0";
    document.body.appendChild(textarea); textarea.focus(); textarea.select();
    const ok = document.execCommand("copy"); document.body.removeChild(textarea); return ok;
  };
  const copyText = async (text: string) => {
    const errors: string[] = [];
    try { if (navigator.clipboard?.writeText) { await navigator.clipboard.writeText(text); return; } } catch (err) { errors.push(`navigator.clipboard: ${String(err)}`); }
    try { if (copyViaExecCommand(text)) return; errors.push("document.execCommand(copy) returned false"); } catch (err) { errors.push(`document.execCommand(copy): ${String(err)}`); }
    try { await invoke("copy_to_clipboard", { text }); return; } catch (err) { errors.push(`tauri invoke copy_to_clipboard: ${String(err)}`); }
    setError(`Copy failed. ${errors.join("; ")}`);
  };
  const copySearchResults = async (paths: string[]) => {
    const selected = paths.filter((p) => p.trim().length > 0);
    if (selected.length === 0) return;
    try { await invoke("copy_search_results", { paths: selected }); } catch (err) { setError(String(err)); }
  };
  const previewResults = async (paths: string[]) => {
    if (paths.length === 0) return;
    try {
      let iconX = 0, iconY = 0, iconW = 28, iconH = 28;
      const firstPath = paths[0];
      const rowEl = rowRefs.current.get(firstPath);
      if (rowEl) {
        const iconEl = rowEl.querySelector(".file-icon") as HTMLElement | null;
        if (iconEl) { const rect = iconEl.getBoundingClientRect(); iconX = Math.round(rect.left); iconY = Math.round(rect.top); iconW = Math.round(rect.width); iconH = Math.round(rect.height); }
      }
      isPreviewingRef.current = true;
      await invoke("preview_search_result", { paths, iconX, iconY, iconW, iconH });
      setTimeout(() => { isPreviewingRef.current = false; }, 300);
    } catch (err) { isPreviewingRef.current = false; setError(String(err)); }
  };

  // ── row click / selection ──
  const handleRowClick = (event: React.MouseEvent<HTMLElement>, item: SearchResultItem, index: number) => {
    blurActiveEditable();
    const path = item.path;
    const isMetaMulti = event.metaKey;
    if (event.shiftKey) {
      const anchorPath = selectionAnchorPath ?? selectedPathsInOrder[0] ?? path;
      const anchorIndex = items.findIndex((entry) => entry.path === anchorPath);
      if (anchorIndex < 0) { setSelectedItemPaths([path]); setSelectionAnchorPath(path); return; }
      const rangeStart = Math.min(anchorIndex, index);
      const rangeEnd = Math.max(anchorIndex, index);
      const rangePaths = items.slice(rangeStart, rangeEnd + 1).map((entry) => entry.path);
      if (isMetaMulti) { const merged = new Set(selectedPathsInOrder); for (const p of rangePaths) merged.add(p); setSelectedItemPaths(Array.from(merged)); }
      else setSelectedItemPaths(rangePaths);
      setSelectionAnchorPath(path); return;
    }
    if (isMetaMulti) {
      if (selectedItemPathSet.has(path)) { const next = selectedItemPaths.filter((p) => p !== path); setSelectedItemPaths(next); setSelectionAnchorPath(next.length > 0 ? next[next.length - 1] : null); }
      else { setSelectedItemPaths([...selectedItemPaths, path]); setSelectionAnchorPath(path); }
      return;
    }
    setSelectedItemPaths([path]); setSelectionAnchorPath(path);
  };
  const moveSelectionByArrow = (delta: number) => {
    if (items.length === 0) return;
    const anchorPath = selectionAnchorPath ?? selectedPathsInOrder[0] ?? null;
    const anchorIndex = anchorPath ? items.findIndex((entry) => entry.path === anchorPath) : -1;
    const startIndex = anchorIndex >= 0 ? anchorIndex : delta > 0 ? -1 : items.length;
    const nextIndex = Math.max(0, Math.min(items.length - 1, startIndex + delta));
    const nextPath = items[nextIndex].path;
    setSelectedItemPaths([nextPath]); setSelectionAnchorPath(nextPath);
    window.requestAnimationFrame(() => { rowRefs.current.get(nextPath)?.scrollIntoView({ block: "nearest" }); });
  };
  const openResultContextMenu = (event: React.MouseEvent<HTMLElement>, item: SearchResultItem) => {
    event.preventDefault(); blurActiveEditable();
    const menuWidth = 230;
    const keepsMultiSelection = selectedItemPathSet.has(item.path);
    const menuIsMulti = keepsMultiSelection && hasMultiSelection;
    const menuHeight = menuIsMulti ? 432 : 372;
    const x = Math.max(8, Math.min(event.clientX, window.innerWidth - menuWidth - 8));
    const y = Math.max(8, Math.min(event.clientY, window.innerHeight - menuHeight - 8));
    if (!selectedItemPathSet.has(item.path)) { setSelectedItemPaths([item.path]); setSelectionAnchorPath(item.path); }
    setContextMenu({ x, y, item, multiSelection: menuIsMulti }); setOpenWithVisible(false);
  };

  // ── sort ──
  const toggleHeaderSort = (key: SortKey) => {
    if (sortKey === key) { setSortAscending((prev) => !prev); return; }
    setSortKey(key); setSortAscending(true);
  };

  // ── column resize ──
  const startResize = (left: ColumnKey, right: ColumnKey, marker: string) => (event: React.MouseEvent<HTMLSpanElement>) => {
    if (event.button !== 0) return;
    event.preventDefault(); event.stopPropagation();
    resizeStateRef.current = { left, right, startX: event.clientX, startWidths: { ...columnWidthsRef.current } };
    setActiveResizer(marker); document.body.style.userSelect = "none"; document.body.style.cursor = "col-resize";
  };
  const fitColumnsToContainer = useCallback(() => {
    const body = tableBodyRef.current; const shell = tableShellRef.current;
    const hostWidth = body?.clientWidth ?? shell?.clientWidth ?? 0;
    if (hostWidth <= 0) return;
    const available = Math.max(0, hostWidth - 32);
    setColumnWidths((prev) => {
      const total = prev.name + prev.path + prev.type + prev.size + prev.modified;
      if (total === available) return prev;
      if (total < available) {
        const deficit = available - total; const next = { ...prev };
        const weightTotal = COLUMN_KEYS.reduce((sum, key) => sum + DEFAULT_COLUMN_WIDTHS[key], 0);
        let distributed = 0;
        for (const key of COLUMN_KEYS) { const add = Math.floor((deficit * DEFAULT_COLUMN_WIDTHS[key]) / weightTotal); next[key] = Math.round(next[key] + add); distributed += add; }
        let remaining = deficit - distributed; const growOrder: ColumnKey[] = ["path", "name", "modified", "type", "size"]; let cursor = 0;
        while (remaining > 0) { const key = growOrder[cursor % growOrder.length]; next[key] = Math.round(next[key] + 1); remaining -= 1; cursor += 1; }
        if (next.name === prev.name && next.path === prev.path && next.type === prev.type && next.size === prev.size && next.modified === prev.modified) return prev;
        return next;
      }
      const next = { ...prev }; let overflow = total - available; const order: ColumnKey[] = ["path", "modified", "size", "name", "type"];
      for (const key of order) {
        if (overflow <= 0) break;
        const current = next[key]; const reducible = Math.max(0, current - DEFAULT_COLUMN_WIDTHS[key] + (DEFAULT_COLUMN_WIDTHS[key] - 1));
        // Use min width directly
        const minWidth = 72; // fallback
        const realMin: Record<string, number> = { name: 160, path: 120, type: 90, size: 72, modified: 120 };
        const actualMin = realMin[key] ?? minWidth;
        const cuttable = Math.max(0, current - actualMin);
        if (cuttable <= 0) continue;
        const cut = Math.min(cuttable, overflow); next[key] = Math.round(current - cut); overflow -= cut;
      }
      if (next.name === prev.name && next.path === prev.path && next.type === prev.type && next.size === prev.size && next.modified === prev.modified) return prev;
      return next;
    });
  }, []);

  // ── settings apply ──
  const applyWindowToggleShortcut = async (shortcut: string) => {
    const normalized = shortcut.trim();
    if (!normalized) { setShortcutStatus(t.shortcutNeedModifier); return; }
    setError(null); setShortcutStatus(""); setIsShortcutSaving(true);
    try {
      const saved = await invoke<string>("set_window_toggle_shortcut", { shortcut: normalized });
      const next = saved.trim().length > 0 ? saved : normalized;
      setWindowToggleShortcut(next); setShortcutDraft(next); setShortcutStatus(t.shortcutSaved);
    } catch (err) { const message = String(err); setError(message); setShortcutStatus(message); }
    finally { setIsShortcutSaving(false); }
  };
  const resetWindowToggleShortcut = async () => { setShortcutDraft(DEFAULT_WINDOW_TOGGLE_SHORTCUT); await applyWindowToggleShortcut(DEFAULT_WINDOW_TOGGLE_SHORTCUT); };

  const applyLaunchSettings = async (nextLaunchAtLogin: boolean, nextSilentStart: boolean, nextShowDockIcon: boolean) => {
    if (isLaunchSettingsSaving) return;
    setIsLaunchSettingsSaving(true); setLaunchSettingsStatus(t.startupSaving); setError(null);
    try {
      const saved = await invoke<LaunchSettingsResponse>("set_launch_settings", { launchAtLogin: nextLaunchAtLogin, silentStart: nextSilentStart, showDockIcon: nextShowDockIcon });
      setLaunchAtLogin(saved.launchAtLogin); setSilentStart(saved.silentStart); setShowDockIcon(saved.showDockIcon); setLaunchSettingsStatus(t.startupSaved);
    } catch (err) { setLaunchSettingsStatus(t.startupSaveFailed); setError(String(err)); }
    finally { setIsLaunchSettingsSaving(false); }
  };
  const applyAutoVacuumSettings = async (next: boolean) => {
    if (isAutoVacuumSettingsSaving) return;
    setIsAutoVacuumSettingsSaving(true); setAutoVacuumSettingsStatus(t.autoVacuumSaving); setError(null);
    try {
      const saved = await invoke<AutoVacuumSettingsResponse>("set_auto_vacuum_settings", { autoVacuumOnRebuild: next });
      setAutoVacuumOnRebuild(saved.autoVacuumOnRebuild); setAutoVacuumSettingsStatus(t.autoVacuumSaved);
    } catch (err) { setAutoVacuumSettingsStatus(t.autoVacuumSaveFailed); setError(String(err)); }
    finally { setIsAutoVacuumSettingsSaving(false); }
  };
  const applyAutoCheckUpdate = async (next: boolean) => {
    if (isAutoCheckSaving) return;
    setIsAutoCheckSaving(true); setAutoCheckStatus(""); setError(null);
    try {
      const saved = await invoke<{ autoCheckUpdate: boolean }>("set_auto_check_update", { autoCheckUpdate: next });
      setAutoCheckUpdate(saved.autoCheckUpdate); setAutoCheckStatus(t.updateSaved);
    } catch (err) { setAutoCheckStatus(t.updateFailed); setError(String(err)); }
    finally { setIsAutoCheckSaving(false); }
  };
  const applyMaxResults = async (next: number) => {
    if (isMaxResultsSaving) return;
    const clamped = Math.max(50, Math.min(10000, Math.round(next)));
    setMaxResults(clamped); setIsMaxResultsSaving(true); setMaxResultsStatus(t.maxResultsSaving); setError(null);
    try {
      const saved = await invoke<{ maxResults: number }>("set_max_results", { maxResults: clamped });
      setMaxResults(saved.maxResults); setMaxResultsStatus(t.maxResultsSaved);
    } catch (err) { setMaxResultsStatus(t.maxResultsSaveFailed); setError(String(err)); }
    finally { setIsMaxResultsSaving(false); }
  };
  const checkForUpdatesManually = async () => {
    if (isCheckingUpdate) return;
    setIsCheckingUpdate(true); setUpdateInfo(null);
    try { const result = await invoke<{ hasUpdate: boolean; latestVersion: string }>("check_for_update"); setUpdateInfo(result); }
    catch { setUpdateInfo({ hasUpdate: false, latestVersion: "" }); }
    finally { setIsCheckingUpdate(false); }
  };
  const applyExcludeDirSettings = async (nextExact: string[], nextPattern: string[]) => {
    if (isExcludeDirSaving) return;
    setIsExcludeDirSaving(true); setExcludeDirStatus(""); setError(null);
    try {
      const saved = await invoke<ExcludeDirSettingsResponse>("set_exclude_dir_settings", { exactDirs: nextExact, patternDirs: nextPattern });
      setExcludeExactDirs(saved.exactDirs); setExcludePatternDirs(saved.patternDirs); setExcludeDirStatus(t.excludeSaved);
    } catch (err) { setExcludeDirStatus(t.excludeSaveFailed); setError(String(err)); }
    finally { setIsExcludeDirSaving(false); }
  };
  const addExcludeRule = async () => {
    const rule = excludeRuleDraft.trim(); if (!rule) return;
    if (excludeRuleType === "exact") await applyExcludeDirSettings([...excludeExactDirs, rule], excludePatternDirs);
    else await applyExcludeDirSettings(excludeExactDirs, [...excludePatternDirs, rule]);
    setExcludeRuleDraft("");
  };
  const removeExcludeRule = async (type: ExcludeRuleType, rule: string) => {
    if (type === "exact") await applyExcludeDirSettings(excludeExactDirs.filter((r) => r !== rule), excludePatternDirs);
    else await applyExcludeDirSettings(excludeExactDirs, excludePatternDirs.filter((r) => r !== rule));
  };
  const applyWatchRoots = async (nextRoots: string[]) => {
    if (isWatchRootSaving) return;
    setIsWatchRootSaving(true); setWatchRootStatus(""); setError(null);
    try {
      const saved = await invoke<WatchRootsSettingsResponse>("set_watch_roots_settings", { roots: nextRoots });
      setWatchRoots(saved.roots); setWatchRootStatus(t.watchRootsSaved);
    } catch (err) { setWatchRootStatus(t.watchRootsSaveFailed); setError(String(err)); }
    finally { setIsWatchRootSaving(false); }
  };
  const addWatchRoot = async () => { const root = watchRootDraft.trim(); if (!root) return; await applyWatchRoots([...watchRoots, root]); setWatchRootDraft(""); };
  const removeWatchRoot = async (root: string) => { await applyWatchRoots(watchRoots.filter((r) => r !== root)); };
  const pickWatchRoot = async () => {
    if (isPickingPath || isWatchRootSaving) return;
    setError(null); setIsPickingPath(true);
    try { const selected = await invoke<string | null>("pick_path_in_finder"); if (selected && selected.trim().length > 0) setWatchRootDraft(selected.trim()); }
    catch (err) { setError(String(err)); }
    finally { setIsPickingPath(false); }
  };
  const pickExcludeRulePath = async () => {
    if (isPickingPath || excludeRuleType !== "exact") return;
    setError(null); setIsPickingPath(true);
    try { const selected = await invoke<string | null>("pick_path_in_finder"); if (selected && selected.trim().length > 0) setExcludeRuleDraft(selected.trim()); }
    catch (err) { setError(String(err)); }
    finally { setIsPickingPath(false); }
  };
  const pickPath = async () => {
    if (isPickingPath) return;
    setError(null); setIsPickingPath(true);
    try { const selected = await invoke<string | null>("pick_path_in_finder"); if (selected && selected.trim().length > 0) { setPathPrefix(selected); closePathDropdown(); } }
    catch (err) { setError(String(err)); }
    finally { setIsPickingPath(false); }
  };
  const pickApp = async (): Promise<string | null> => {
    if (isPickingPath) return null;
    setError(null); setIsPickingPath(true);
    try { return await invoke<string | null>("pick_app"); }
    catch (err) { setError(String(err)); return null; }
    finally { setIsPickingPath(false); }
  };
  const applyFileManagerSettings = async (fa: string, ta: string, cfa: string, cta: string) => {
    try {
      const saved = await invoke<FileManagerSettingsResponse>("set_file_manager_settings", { defaultFolderAction: fa, defaultTerminalAction: ta, customFolderApp: cfa, customTerminalApp: cta });
      setDefaultFolderAction(saved.defaultFolderAction); setDefaultTerminalAction(saved.defaultTerminalAction); setCustomFolderApp(saved.customFolderApp); setCustomTerminalApp(saved.customTerminalApp);
    } catch (err) { setError(String(err)); }
  };

  const runBuild = async (rebuild: boolean) => {
    if (isBuilding) return;
    setError(null); setIsBuilding(true);
    setBuildStatus(rebuild ? t.buildStatusRebuilding : t.buildStatusBuilding);
    try {
      const result = await invoke<BuildResponse>("build_index", { path: pathPrefix.trim() || null, rebuild, includeDirs: true });
      setIndexed(result.indexed); setBuildStatus("");
    } catch (err) { setError(String(err)); setBuildStatus(t.buildStatusFailed); }
    finally { setIsBuilding(false); }
  };
  const toggleWatch = async () => {
    if (isWatchPending) return;
    setError(null); setIsWatchPending(true);
    try {
      const command = isWatchRunning ? "stop_watch" : "start_watch_auto";
      const status = await invoke<WatchResponse>(command);
      setIsWatchRunning(status.running);
    } catch (err) { setError(String(err)); }
    finally { setIsWatchPending(false); }
  };

  const handlePathInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") { closePathDropdown(); return; }
    if (event.key === "ArrowDown") {
      event.preventDefault(); if (!isPathDropdownOpen) setIsPathDropdownOpen(true);
      if (visiblePathSuggestions.length === 0) return;
      setActivePathSuggestion((prev) => prev < 0 || prev >= visiblePathSuggestions.length - 1 ? 0 : prev + 1); return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault(); if (!isPathDropdownOpen) setIsPathDropdownOpen(true);
      if (visiblePathSuggestions.length === 0) return;
      setActivePathSuggestion((prev) => prev <= 0 ? visiblePathSuggestions.length - 1 : prev - 1); return;
    }
    if (event.key === "Enter" && isPathDropdownVisible && activePathSuggestion >= 0) {
      event.preventDefault(); applyPathSuggestion(visiblePathSuggestions[activePathSuggestion]);
    }
  };

  // ── effects ──
  useEffect(() => { columnWidthsRef.current = columnWidths; }, [columnWidths]);
  useEffect(() => { return () => { clearOpenWithCloseTimer(); }; }, []);
  useEffect(() => { if (activePathSuggestion < visiblePathSuggestions.length) return; setActivePathSuggestion(-1); }, [activePathSuggestion, visiblePathSuggestions.length]);

  useEffect(() => {
    if (!isPathDropdownOpen) return;
    const closeWhenClickOutside = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (pathPickerRef.current?.contains(target)) return;
      closePathDropdown();
    };
    window.addEventListener("mousedown", closeWhenClickOutside);
    return () => window.removeEventListener("mousedown", closeWhenClickOutside);
  }, [isPathDropdownOpen]);

  useEffect(() => {
    if (!contextMenu) return;
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === "Escape") closeContextMenu(); };
    const close = () => closeContextMenu();
    window.addEventListener("keydown", closeOnEscape); window.addEventListener("resize", close); window.addEventListener("scroll", close, true);
    return () => { window.removeEventListener("keydown", closeOnEscape); window.removeEventListener("resize", close); window.removeEventListener("scroll", close, true); };
  }, [contextMenu]);

  useEffect(() => {
    const visible = new Set(items.map((item) => item.path));
    setSelectedItemPaths((prev) => { const next = prev.filter((p) => visible.has(p)); return next.length === prev.length ? prev : next; });
    setSelectionAnchorPath((prev) => (prev && visible.has(prev) ? prev : null));
  }, [items]);

  useEffect(() => { const stored = localStorage.getItem(LANGUAGE_STORAGE_KEY); if (stored === "zh" || stored === "en") setLanguage(stored); }, []);
  useEffect(() => { localStorage.setItem(THEME_STORAGE_KEY, themeMode); }, [themeMode]);
  useEffect(() => { localStorage.setItem(REGEX_ENABLED_STORAGE_KEY, regexEnabled ? "1" : "0"); }, [regexEnabled]);
  useEffect(() => { localStorage.setItem(CASE_SENSITIVE_STORAGE_KEY, caseSensitive ? "1" : "0"); }, [caseSensitive]);
  useEffect(() => { localStorage.setItem(FUZZY_ENABLED_STORAGE_KEY, fuzzyEnabled ? "1" : "0"); }, [fuzzyEnabled]);
  useEffect(() => { localStorage.setItem(LANGUAGE_STORAGE_KEY, language); }, [language]);
  useEffect(() => { void invoke("set_menu_language", { language }); }, [language]);
  useEffect(() => { document.documentElement.setAttribute("data-theme", resolvedTheme); }, [resolvedTheme]);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    setSystemDark(media.matches);
    if (typeof media.addEventListener === "function") { media.addEventListener("change", onChange); return () => media.removeEventListener("change", onChange); }
    const legacyListener = (event: MediaQueryListEvent) => onChange(event);
    media.addListener(legacyListener); return () => media.removeListener(legacyListener);
  }, []);

  useEffect(() => { if (isIndexLoading) closePathDropdown(); }, [isIndexLoading]);

  // Listen for menu events
  useEffect(() => {
    let unlistenMenu: (() => void) | undefined;
    void listen(EVENT_OPEN_SETTINGS, () => setActiveView("settings")).then((d) => { unlistenMenu = d; }).catch(() => {});
    return () => { if (unlistenMenu) unlistenMenu(); };
  }, []);
  useEffect(() => {
    let unlistenFocus: (() => void) | undefined;
    void listen(EVENT_FOCUS_SEARCH, () => { setActiveView("search"); window.requestAnimationFrame(() => { searchInputRef.current?.focus(); searchInputRef.current?.select(); }); }).then((d) => { unlistenFocus = d; }).catch(() => {});
    return () => { if (unlistenFocus) unlistenFocus(); };
  }, []);

  // Load settings
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<string>("get_window_toggle_shortcut"); if (!m) return; const n = s.trim().length > 0 ? s : DEFAULT_WINDOW_TOGGLE_SHORTCUT; setWindowToggleShortcut(n); setShortcutDraft(n); } catch { if (m) { setWindowToggleShortcut(DEFAULT_WINDOW_TOGGLE_SHORTCUT); setShortcutDraft(DEFAULT_WINDOW_TOGGLE_SHORTCUT); } } };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<WatchRootsSettingsResponse>("get_watch_roots_settings"); if (m) setWatchRoots(s.roots); } catch (e) { if (m) setError(String(e)); } };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<AutoVacuumSettingsResponse>("get_auto_vacuum_settings"); if (m) setAutoVacuumOnRebuild(s.autoVacuumOnRebuild); } catch (e) { if (m) setError(String(e)); } };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<{ autoCheckUpdate: boolean }>("get_auto_check_update"); if (m) setAutoCheckUpdate(s.autoCheckUpdate); } catch {} };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<{ maxResults: number }>("get_max_results"); if (m) setMaxResults(s.maxResults); } catch {} };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    if (!autoCheckUpdate) return; let m = true;
    const check = async () => { try { const r = await invoke<{ hasUpdate: boolean; latestVersion: string }>("check_for_update"); if (m && r.hasUpdate) setUpdateInfo(r); } catch {} };
    const tmr = window.setTimeout(() => { void check(); }, 3000);
    return () => { m = false; window.clearTimeout(tmr); };
  }, [autoCheckUpdate]);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<LaunchSettingsResponse>("get_launch_settings"); if (m) { setLaunchAtLogin(s.launchAtLogin); setSilentStart(s.silentStart); setShowDockIcon(s.showDockIcon); } } catch (e) { if (m) setError(String(e)); } };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<ExcludeDirSettingsResponse>("get_exclude_dir_settings"); if (m) { setExcludeExactDirs(s.exactDirs); setExcludePatternDirs(s.patternDirs); } } catch (e) { if (m) setError(String(e)); } };
    void load(); return () => { m = false; };
  }, []);
  useEffect(() => {
    let m = true; const load = async () => { try { const s = await invoke<FileManagerSettingsResponse>("get_file_manager_settings"); if (m) { setDefaultFolderAction(s.defaultFolderAction); setDefaultTerminalAction(s.defaultTerminalAction); setCustomFolderApp(s.customFolderApp); setCustomTerminalApp(s.customTerminalApp); } } catch (e) { console.error("Failed to load file manager settings", e); } };
    void load(); return () => { m = false; };
  }, []);

  // Initialize + start watch
  useEffect(() => {
    let m = true;
    const init = async () => {
      if (m) setIsIndexLoading(true);
      try { const initial = await invoke<InitResponse>("initialize"); if (m) { setIndexed(initial.indexed); try { const v = await invoke<string>("get_version"); if (m) setAppVersion(v); } catch {} } }
      catch (e) { if (m) setError(String(e)); }
      finally { if (m) setIsIndexLoading(false); }
      try {
        const watch = await invoke<WatchResponse>("start_watch_auto");
        if (m) { setIsWatchRunning(watch.running); if (watch.code === "bootstrap") { setIsBuilding(true); setBuildStatus(t.buildStatusBuilding); } }
      } catch (e) {
        if (m) setError(String(e));
        try { const watch = await invoke<WatchResponse>("watch_status"); if (m) setIsWatchRunning(watch.running); } catch {}
      }
    };
    void init(); return () => { m = false; };
  }, []);

  // Path suggestions
  useEffect(() => {
    if (!isPathDropdownOpen) return; let m = true;
    const load = async () => { try { const s = await invoke<string[]>("list_path_suggestions"); if (m) setPathSuggestions(s); } catch {} };
    void load(); return () => { m = false; };
  }, [isPathDropdownOpen]);

  // Column resize + fit
  useEffect(() => {
    fitColumnsToContainer(); window.addEventListener("resize", fitColumnsToContainer);
    return () => window.removeEventListener("resize", fitColumnsToContainer);
  }, [fitColumnsToContainer]);

  useEffect(() => {
    const onMouseMove = (event: MouseEvent) => {
      const state = resizeStateRef.current; if (!state) return;
      const delta = event.clientX - state.startX; const start = state.startWidths;
      const leftKey = state.left; const leftIndex = COLUMN_KEYS.indexOf(leftKey);
      const rightIndex = COLUMN_KEYS.indexOf(state.right);
      if (leftIndex < 0 || rightIndex < 0 || rightIndex !== leftIndex + 1) return;
      const next: Record<ColumnKey, number> = { ...start };
      if (delta >= 0) {
        let remaining = delta;
        for (let i = rightIndex; i < COLUMN_KEYS.length && remaining > 0; i += 1) {
          const key = COLUMN_KEYS[i];
          const min = { name: 160, path: 120, type: 90, size: 72, modified: 120 }[key] ?? 72;
          const cuttable = Math.max(0, next[key] - min); if (cuttable <= 0) continue;
          const cut = Math.min(cuttable, remaining); next[key] = Math.round(next[key] - cut); remaining -= cut;
        }
        next[leftKey] = Math.round(start[leftKey] + (delta - remaining));
      } else {
        let remaining = -delta;
        for (let i = leftIndex; i >= 0 && remaining > 0; i -= 1) {
          const key = COLUMN_KEYS[i];
          const min = { name: 160, path: 120, type: 90, size: 72, modified: 120 }[key] ?? 72;
          const cuttable = Math.max(0, next[key] - min); if (cuttable <= 0) continue;
          const cut = Math.min(cuttable, remaining); next[key] = Math.round(next[key] - cut); remaining -= cut;
        }
        next[COLUMN_KEYS[rightIndex]] = Math.round(start[COLUMN_KEYS[rightIndex]] + (-delta - remaining));
      }
      setColumnWidths(next);
    };
    const finishResize = () => {
      if (resizeStateRef.current && typeof window !== "undefined") {
        try { window.localStorage.setItem(COLUMN_WIDTHS_STORAGE_KEY, JSON.stringify(columnWidthsRef.current)); } catch {}
      }
      resizeStateRef.current = null; setActiveResizer(null); document.body.style.userSelect = ""; document.body.style.cursor = "";
    };
    window.addEventListener("mousemove", onMouseMove); window.addEventListener("mouseup", finishResize);
    window.addEventListener("blur", finishResize); window.addEventListener("mouseleave", finishResize);
    return () => { window.removeEventListener("mousemove", onMouseMove); window.removeEventListener("mouseup", finishResize); window.removeEventListener("blur", finishResize); window.removeEventListener("mouseleave", finishResize); };
  }, []);

  // Build status listener
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<BuildEvent>("index://build-status", (event) => {
      const p = event.payload;
      if (p.phase === "started") setIsBuilding(true);
      else { setIsBuilding(false); if (typeof p.indexed === "number") setIndexed(p.indexed); setBuildStatus(""); }
    }).then((d) => { unlisten = d; }).catch(() => { setError("Unable to listen to build events."); });
    return () => { if (unlisten) unlisten(); };
  }, []);

  // Volume events
  const volumeMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const isZh = language === "zh";
    void listen<VolumeEventType>("volume://event", (event) => {
      const e = event.payload;
      if (volumeMsgTimer.current) { clearTimeout(volumeMsgTimer.current); }
      const volNameFromPath = (path: string) => { const parts = path.split("/"); return parts[parts.length - 1] || path; };
      if (e.type === "mountDetected") { const name = volNameFromPath(e.path); setBuildStatus(isZh ? `检测到新卷 ${name}，正在索引...` : `New volume ${name} detected, indexing...`); }
      else if (e.type === "indexComplete") { const name = volNameFromPath(e.path); setBuildStatus(isZh ? `${name} 索引完成，${e.fileCount} 个文件` : `${name} indexed, ${e.fileCount} files`); volumeMsgTimer.current = setTimeout(() => setBuildStatus(""), 5000); }
      else if (e.type === "volumeRemoved") { const name = volNameFromPath(e.path); setBuildStatus(isZh ? `${name} 已断开，索引已清理` : `${name} disconnected, index removed`); volumeMsgTimer.current = setTimeout(() => setBuildStatus(""), 5000); }
    }).then((d) => { unlisten = d; }).catch(() => {});
    return () => { if (unlisten) unlisten(); if (volumeMsgTimer.current) clearTimeout(volumeMsgTimer.current); };
  }, []);

  // Persist watch cursor on unload
  useEffect(() => {
    const persist = () => { void invoke("persist_watch_cursor"); };
    window.addEventListener("beforeunload", persist);
    return () => window.removeEventListener("beforeunload", persist);
  }, []);

  // Search
  useEffect(() => {
    if (isIndexLoading) { setIsSearching(false); return; }
    let cancelled = false;
    const runSearch = async () => {
      const needle = query.trim();
      if (needle.length === 0) { setItems([]); setTotalFound(0); setTookMs(0); setScrollTop(0); return; }
      setIsSearching(true); setError(null);
      try {
        const response = await invoke<SearchResponse>("search", buildSearchRequest(needle, activeTab, pathPrefix, caseSensitive, regexEnabled, fuzzyEnabled, sortKey, sortAscending, maxResults));
        if (cancelled) return;
        setItems(response.items); setTotalFound(response.total); setTookMs(response.tookMs); setScrollTop(0);
        if (tableBodyRef.current) tableBodyRef.current.scrollTop = 0;
      } catch (err) { if (!cancelled) { setError(String(err)); setItems([]); setTotalFound(0); setScrollTop(0); } }
      finally { if (!cancelled) setIsSearching(false); }
    };
    const timer = window.setTimeout(() => { void runSearch(); }, 180);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [query, pathPrefix, activeTab, regexEnabled, caseSensitive, fuzzyEnabled, sortKey, sortAscending, isIndexLoading, maxResults]);

  // Reset app filter when no longer valid
  useEffect(() => { if (appFilter && !appFilterOptions.includes(appFilter)) setAppFilter(""); }, [appFilterOptions, appFilter]);

  // Popovers click outside
  useEffect(() => {
    if (!showTimePopover && !showSizePopover) return;
    const onMouseDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (showTimePopover && timePopoverRef.current && !timePopoverRef.current.contains(target)) setShowTimePopover(false);
      if (showSizePopover && sizePopoverRef.current && !sizePopoverRef.current.contains(target)) setShowSizePopover(false);
    };
    document.addEventListener("mousedown", onMouseDown);
    return () => document.removeEventListener("mousedown", onMouseDown);
  }, [showTimePopover, showSizePopover]);

  // Keyboard navigation
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.key === "ArrowDown" || event.key === "ArrowUp") && activeView === "search" && !contextMenu) {
        if (!isEditableTarget(event.target)) { event.preventDefault(); moveSelectionByArrow(event.key === "ArrowDown" ? 1 : -1); }
        return;
      }
      const openSelectedResult = () => {
        const source = activeView === "pinned" ? pinnedItems : itemsRef.current;
        const selected = source.find((i) => selectedItemPathSet.has(i.path));
        if (selected) { event.preventDefault(); void openResult(selected.path); }
      };
      if (event.key === "Enter" && (activeView === "search" || activeView === "pinned") && !contextMenu && !isEditableTarget(event.target)) { openSelectedResult(); return; }
      if ((event.metaKey || event.ctrlKey) && !event.altKey) {
        const key = event.key.toLowerCase();
        if (key === "o") { openSelectedResult(); return; }
        if (key === "1") { event.preventDefault(); setActiveView("search"); return; }
        if (key === "2") { event.preventDefault(); setActiveView("pinned"); return; }
        if (key === "3") { event.preventDefault(); setActiveView("settings"); return; }
        if (key === "f") { event.preventDefault(); setActiveView("search"); searchInputRef.current?.focus(); searchInputRef.current?.select(); return; }
      }
      if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "a") {
        if ((activeView !== "search" && activeView !== "pinned") || contextMenu) { if (activeView === "settings" || !isEditableTarget(event.target)) event.preventDefault(); return; }
        if (isEditableTarget(event.target)) return;
        event.preventDefault();
        const source = activeView === "pinned" ? pinnedItems : itemsRef.current;
        setSelectedItemPaths(source.map((item) => item.path)); return;
      }
      if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "c") {
        if ((activeView !== "search" && activeView !== "pinned") || contextMenu || selectedPathsInOrder.length === 0) return;
        if (isEditableTarget(event.target)) return;
        event.preventDefault(); blurActiveEditable(); void copySearchResults(selectedPathsInOrder); return;
      }
      if (event.key !== " " && event.code !== "Space") return;
      if (event.repeat || (activeView !== "search" && activeView !== "pinned") || selectedPathsInOrder.length === 0 || contextMenu) return;
      if (isEditableTarget(event.target)) return;
      event.preventDefault(); blurActiveEditable(); void previewResults(selectedPathsInOrder);
    };
    window.addEventListener("keydown", onKeyDown);
    const onBlur = () => { if (isPreviewingRef.current) return; if (selectedPathsInOrder.length > 0) { setSelectedItemPaths([]); setSelectionAnchorPath(null); } };
    window.addEventListener("blur", onBlur);
    return () => { window.removeEventListener("keydown", onKeyDown); window.removeEventListener("blur", onBlur); };
  }, [activeView, contextMenu, selectedPathsInOrder]);

  // ── render ──
  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="header-left">
          <div className="logo-wrap">
            <span className="logo-icon">⚡</span>
            <span className="logo-text">MacHunt</span>
          </div>
          <nav className="header-nav">
            <button className={activeView === "search" ? "nav-btn active" : "nav-btn"} onClick={() => setActiveView("search")}>
              <span className="nav-btn-icon">⌂</span>{t.searchTag}
            </button>
            <button className={activeView === "pinned" ? "nav-btn active" : "nav-btn"} onClick={() => setActiveView("pinned")}>
              <span className="nav-btn-icon">★</span>{t.pinnedTag}
            </button>
            <button className={activeView === "settings" ? "nav-btn active" : "nav-btn"} onClick={() => setActiveView("settings")}>
              <span className="nav-btn-icon">⚙</span>{t.settingsTitle}
            </button>
          </nav>
        </div>
      </header>

      {activeView === "search" ? (
        <SearchView
          t={t}
          query={query} setQuery={setQuery}
          pathPrefix={pathPrefix} setPathPrefix={setPathPrefix}
          isIndexLoading={isIndexLoading}
          activeTab={activeTab} setActiveTab={setActiveTab}
          regexEnabled={regexEnabled} setRegexEnabled={setRegexEnabled}
          fuzzyEnabled={fuzzyEnabled} setFuzzyEnabled={setFuzzyEnabled}
          caseSensitive={caseSensitive} setCaseSensitive={setCaseSensitive}
          pathPickerRef={pathPickerRef} pathInputRef={pathInputRef}
          pathSuggestions={pathSuggestions}
          isPathDropdownOpen={isPathDropdownOpen} setIsPathDropdownOpen={setIsPathDropdownOpen}
          activePathSuggestion={activePathSuggestion} setActivePathSuggestion={setActivePathSuggestion}
          isPickingPath={isPickingPath} pickPath={pickPath}
          handlePathInputKeyDown={handlePathInputKeyDown}
          isPathDropdownVisible={isPathDropdownVisible}
          visiblePathSuggestions={visiblePathSuggestions}
          closePathDropdown={closePathDropdown}
          applyPathSuggestion={applyPathSuggestion}
          timeFilter={timeFilter} setTimeFilter={setTimeFilter}
          showTimePopover={showTimePopover} setShowTimePopover={setShowTimePopover}
          showSizePopover={showSizePopover} setShowSizePopover={setShowSizePopover}
          timePopoverRef={timePopoverRef} sizePopoverRef={sizePopoverRef}
          calMonth={calMonth} setCalMonth={setCalMonth}
          calYear={calYear} setCalYear={setCalYear}
          calSelecting={calSelecting} setCalSelecting={setCalSelecting}
          calFrom={calFrom} setCalFrom={setCalFrom}
          calTo={calTo} setCalTo={setCalTo}
          customTimeFromRef={customTimeFromRef} customTimeToRef={customTimeToRef}
          customTimeLabel={customTimeLabel}
          setFilterVersion={setFilterVersion}
          sizeFilter={sizeFilter} setSizeFilter={setSizeFilter}
          customSizeMin={customSizeMin} setCustomSizeMinTemp={setCustomSizeMinTemp}
          customSizeMax={customSizeMax} setCustomSizeMaxTemp={setCustomSizeMaxTemp}
          customSizeUnit={customSizeUnit} setCustomSizeUnit={setCustomSizeUnit}
          customSizeMinRef={customSizeMinRef} customSizeMaxRef={customSizeMaxRef}
          customSizeLabel={customSizeLabel}
          appFilter={appFilter} setAppFilter={setAppFilter}
          appFilterOptions={appFilterOptions}
          filteredItems={filteredItems} items={items} itemsRef={itemsRef}
          selectedItemPathSet={selectedItemPathSet}
          selectedItemPaths={selectedItemPaths} setSelectedItemPaths={setSelectedItemPaths}
          selectionAnchorPath={selectionAnchorPath} setSelectionAnchorPath={setSelectionAnchorPath}
          isSearching={isSearching} tookMs={tookMs} indexed={indexed} buildStatus={buildStatus}
          gridTemplateColumns={gridTemplateColumns}
          sortKey={sortKey} sortAscending={sortAscending} toggleHeaderSort={toggleHeaderSort}
          columnWidths={columnWidths} activeResizer={activeResizer} startResize={startResize}
          tableShellRef={tableShellRef} tableBodyRef={tableBodyRef}
          scrollTop={scrollTop} setScrollTop={setScrollTop}
          visibleStart={visibleStart} visibleEnd={visibleEnd}
          visibleItems={visibleItems}
          topSpacerHeight={topSpacerHeight} bottomSpacerHeight={bottomSpacerHeight}
          handleScrollbarScroll={handleScrollbarScroll}
          openResult={openResult} handleRowClick={handleRowClick}
          openResultContextMenu={openResultContextMenu}
          isPinned={isPinned} togglePin={togglePin}
          searchInputRef={searchInputRef} rowRefs={rowRefs}
          onClickOutside={(e) => {
            if (!(e.target as HTMLElement).closest(".result-row") && !(e.target as HTMLElement).closest(".context-menu-layer")) {
              setSelectedItemPaths([]); setSelectionAnchorPath(null);
            }
          }}
        />
      ) : activeView === "pinned" ? (
        <div className="pinned-view"
          onClick={(e) => {
            if (!(e.target as HTMLElement).closest(".result-row") && !(e.target as HTMLElement).closest(".context-menu-layer")) {
              setSelectedItemPaths([]); setSelectionAnchorPath(null);
            }
          }}>
          <div className="pinned-content">
          <header className="settings-header">
            <div><h2>{t.pinnedTag}</h2></div>
          </header>
          {pinnedItems.length === 0 ? (
            <div className="empty-state">{t.pinnedEmpty}</div>
          ) : (
            <>
              <div className="table-header" style={{ gridTemplateColumns }}>
                {(["name", "path", "type", "size", "modified"] as const).map((key, i, arr) => (
                  <span className="header-cell" key={key}>
                    <button type="button" className="header-sort-btn" onClick={() => toggleHeaderSort(key)}>
                      <span className="header-sort-label">{t[`header_${key}` as const]}</span>
                      {sortKey === key && <span className="header-sort-indicator">{sortAscending ? "▲" : "▼"}</span>}
                    </button>
                    {i < arr.length - 1 && (
                      <span
                        className={activeResizer === `${key}-${arr[i+1]}` ? "column-resizer active" : "column-resizer"}
                        onMouseDown={startResize(key, arr[i+1], `${key}-${arr[i+1]}`)}
                      />
                    )}
                  </span>
                ))}
              </div>
              <div className="table-body custom-scrollbar" onScroll={handleScrollbarScroll}>
                {sortedPinnedItems.map((item, index) => {
                  const token = iconToken(item);
                  return (
                    <article
                      key={`${item.path}-${index}`}
                      ref={(el) => { if (el) rowRefs.current.set(item.path, el); else rowRefs.current.delete(item.path); }}
                      className={selectedItemPathSet.has(item.path) ? "result-row selected" : "result-row"}
                      style={{ gridTemplateColumns }}
                      onClick={(event) => handleRowClick(event, item, index)}
                      onDoubleClick={() => void openResult(item.path)}
                      onContextMenu={(event) => openResultContextMenu(event, item)}
                    >
                      <div className="cell name-cell"><span className={`file-icon ${token}`}>{iconGlyph(token)}</span><span className="name-text">{item.name}</span></div>
                      <div className="cell path-cell">{item.parent}</div>
                      <div className="cell type-cell">{typeLabel(item, t.typeFolder, t.typeFile)}</div>
                      <div className="cell size-cell">{formatBytes(item.sizeBytes)}</div>
                      <div className="cell date-cell">{formatDate(item.modifiedUnixMs)}</div>
                      <button className="term-btn" onClick={(e) => { e.stopPropagation(); void invoke("open_in_default_terminal", { path: item.path }); }} title={t.openInDefaultTerminal}>&gt;_</button>
                      <button className="pin-btn pinned" onClick={(e) => { e.stopPropagation(); togglePin(item); }} title={t.menuUnpin}>★</button>
                    </article>
                  );
                })}
              </div>
            </>
          )}
          </div>
          <footer className="status-bar">
            <div className="status-left"><span className="status-highlight">{fmt(t.pinnedCount, { count: pinnedItems.length })}</span></div>
            <div className="status-right" />
          </footer>
        </div>
      ) : (
        <SettingsView
          t={t}
          themeMode={themeMode} setThemeMode={setThemeMode}
          resolvedTheme={resolvedTheme}
          language={language} setLanguage={setLanguage}
          windowToggleShortcut={windowToggleShortcut}
          shortcutDraft={shortcutDraft} setShortcutDraft={setShortcutDraft}
          shortcutStatus={shortcutStatus} setShortcutStatus={setShortcutStatus}
          isShortcutSaving={isShortcutSaving}
          applyWindowToggleShortcut={applyWindowToggleShortcut}
          resetWindowToggleShortcut={resetWindowToggleShortcut}
          launchAtLogin={launchAtLogin} silentStart={silentStart} showDockIcon={showDockIcon}
          isLaunchSettingsSaving={isLaunchSettingsSaving} launchSettingsStatus={launchSettingsStatus}
          applyLaunchSettings={applyLaunchSettings}
          maxResults={maxResults} isMaxResultsSaving={isMaxResultsSaving} maxResultsStatus={maxResultsStatus}
          applyMaxResults={applyMaxResults}
          appVersion={appVersion}
          autoCheckUpdate={autoCheckUpdate} isAutoCheckSaving={isAutoCheckSaving} autoCheckStatus={autoCheckStatus}
          applyAutoCheckUpdate={applyAutoCheckUpdate}
          isCheckingUpdate={isCheckingUpdate} updateInfo={updateInfo}
          checkForUpdatesManually={checkForUpdatesManually}
          openUrl={openUrl}
          defaultFolderAction={defaultFolderAction} setDefaultFolderAction={setDefaultFolderAction}
          defaultTerminalAction={defaultTerminalAction} setDefaultTerminalAction={setDefaultTerminalAction}
          customFolderApp={customFolderApp} setCustomFolderApp={setCustomFolderApp}
          customTerminalApp={customTerminalApp} setCustomTerminalApp={setCustomTerminalApp}
          applyFileManagerSettings={applyFileManagerSettings}
          pickApp={pickApp}
          autoVacuumOnRebuild={autoVacuumOnRebuild}
          isAutoVacuumSettingsSaving={isAutoVacuumSettingsSaving} autoVacuumSettingsStatus={autoVacuumSettingsStatus}
          applyAutoVacuumSettings={applyAutoVacuumSettings}
          watchRootDraft={watchRootDraft} setWatchRootDraft={setWatchRootDraft}
          watchRoots={watchRoots} isWatchRootSaving={isWatchRootSaving} watchRootStatus={watchRootStatus}
          addWatchRoot={addWatchRoot} removeWatchRoot={removeWatchRoot} pickWatchRoot={pickWatchRoot}
          excludeRuleType={excludeRuleType} setExcludeRuleType={setExcludeRuleType}
          excludeRuleDraft={excludeRuleDraft} setExcludeRuleDraft={setExcludeRuleDraft}
          excludeExactDirs={excludeExactDirs} excludePatternDirs={excludePatternDirs}
          excludeDirStatus={excludeDirStatus} isExcludeDirSaving={isExcludeDirSaving}
          addExcludeRule={addExcludeRule} removeExcludeRule={removeExcludeRule}
          pickExcludeRulePath={pickExcludeRulePath}
          isPickingPath={isPickingPath}
          runBuild={runBuild} isBuilding={isBuilding}
          isWatchRunning={isWatchRunning} isWatchPending={isWatchPending} toggleWatch={toggleWatch}
          handleScrollbarScroll={handleScrollbarScroll}
        />
      )}

      {error && <aside className="error-banner">{error}</aside>}

      {contextMenu && (
        <ContextMenu
          contextMenu={contextMenu}
          selectedPathsInOrder={selectedPathsInOrder}
          items={items}
          isPinned={isPinned}
          openResult={openResult}
          revealInFinder={revealInFinder}
          openInQSpace={openInQSpace}
          openInTerminal={openInTerminal}
          openInWezTerm={openInWezTerm}
          copyText={copyText}
          copySearchResults={copySearchResults}
          moveToTrash={moveToTrash}
          togglePin={togglePin}
          t={t}
          closeContextMenu={closeContextMenu}
          openOpenWithMenu={openOpenWithMenu}
          scheduleCloseOpenWithMenu={scheduleCloseOpenWithMenu}
          openWithVisible={openWithVisible}
          setOpenWithVisible={setOpenWithVisible}
          setSelectedItemPaths={setSelectedItemPaths}
          setSelectionAnchorPath={setSelectionAnchorPath}
        />
      )}
    </div>
  );
}

export default App;
