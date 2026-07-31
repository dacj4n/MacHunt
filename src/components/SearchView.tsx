import { useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CustomSelect } from "./CustomSelect";
import type { SearchResultItem, TabId, SortKey, ColumnKey } from "../types";
import {
  TAB_IDS, TAB_ICONS, COLUMN_KEYS,
  iconToken, iconGlyph, typeLabel, formatBytes, formatDate,
  setCellPreviewTooltip, blurActiveEditable, appForExt, extensionOf,
  fmt,
} from "../utils";

export interface SearchViewProps {
  t: Record<string, string>;
  // search state
  query: string;
  setQuery: (v: string) => void;
  pathPrefix: string;
  setPathPrefix: (v: string) => void;
  isIndexLoading: boolean;
  activeTab: TabId;
  setActiveTab: (v: TabId) => void;
  regexEnabled: boolean;
  setRegexEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  fuzzyEnabled: boolean;
  setFuzzyEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  caseSensitive: boolean;
  setCaseSensitive: React.Dispatch<React.SetStateAction<boolean>>;
  // path picker
  pathPickerRef: React.RefObject<HTMLDivElement>;
  pathInputRef: React.RefObject<HTMLInputElement>;
  pathSuggestions: string[];
  isPathDropdownOpen: boolean;
  setIsPathDropdownOpen: (v: boolean) => void;
  activePathSuggestion: number;
  setActivePathSuggestion: (v: number) => void;
  isPickingPath: boolean;
  pickPath: () => Promise<void>;
  handlePathInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
  isPathDropdownVisible: boolean;
  visiblePathSuggestions: string[];
  closePathDropdown: () => void;
  applyPathSuggestion: (path: string) => void;
  // time filter
  timeFilter: string;
  setTimeFilter: (v: string) => void;
  showTimePopover: boolean;
  setShowTimePopover: (v: boolean) => void;
  showSizePopover: boolean;
  setShowSizePopover: (v: boolean) => void;
  timePopoverRef: React.RefObject<HTMLDivElement>;
  sizePopoverRef: React.RefObject<HTMLDivElement>;
  calMonth: number;
  setCalMonth: React.Dispatch<React.SetStateAction<number>>;
  calYear: number;
  setCalYear: React.Dispatch<React.SetStateAction<number>>;
  calSelecting: "from" | "to";
  setCalSelecting: (v: "from" | "to") => void;
  calFrom: string;
  setCalFrom: (v: string) => void;
  calTo: string;
  setCalTo: (v: string) => void;
  customTimeFromRef: React.MutableRefObject<number>;
  customTimeToRef: React.MutableRefObject<number>;
  customTimeLabel: string | null;
  setFilterVersion: React.Dispatch<React.SetStateAction<number>>;
  // size filter
  sizeFilter: string;
  setSizeFilter: (v: string) => void;
  customSizeMin: string;
  setCustomSizeMinTemp: (v: string) => void;
  customSizeMax: string;
  setCustomSizeMaxTemp: (v: string) => void;
  customSizeMinUnit: string;
  setCustomSizeMinUnit: (v: string) => void;
  customSizeMaxUnit: string;
  setCustomSizeMaxUnit: (v: string) => void;
  customSizeMinRef: React.MutableRefObject<number>;
  customSizeMaxRef: React.MutableRefObject<number>;
  customSizeLabel: string | null;
  // app filter
  appFilter: string;
  setAppFilter: (v: string) => void;
  appFilterOptions: string[];
  // results
  filteredItems: SearchResultItem[];
  items: SearchResultItem[];
  itemsRef: React.MutableRefObject<SearchResultItem[]>;
  selectedItemPathSet: Set<string>;
  selectedItemPaths: string[];
  setSelectedItemPaths: (paths: string[]) => void;
  selectionAnchorPath: string | null;
  setSelectionAnchorPath: (path: string | null) => void;
  isSearching: boolean;
  tookMs: number;
  indexed: number;
  buildStatus: string;
  // column sizing
  gridTemplateColumns: string;
  sortKey: SortKey;
  sortAscending: boolean;
  toggleHeaderSort: (key: SortKey) => void;
  columnWidths: Record<ColumnKey, number>;
  activeResizer: string | null;
  startResize: (left: ColumnKey, right: ColumnKey, marker: string) => (event: React.MouseEvent<HTMLSpanElement>) => void;
  tableShellRef: React.RefObject<HTMLDivElement>;
  tableBodyRef: React.RefObject<HTMLDivElement>;
  // virtual scroll
  scrollTop: number;
  setScrollTop: (v: number) => void;
  visibleStart: number;
  visibleEnd: number;
  visibleItems: SearchResultItem[];
  topSpacerHeight: number;
  bottomSpacerHeight: number;
  handleScrollbarScroll: (e: React.UIEvent<HTMLElement>) => void;
  // actions
  openResult: (path: string) => Promise<void>;
  handleRowClick: (event: React.MouseEvent<HTMLElement>, item: SearchResultItem, index: number) => void;
  openResultContextMenu: (event: React.MouseEvent<HTMLElement>, item: SearchResultItem) => void;
  isPinned: (path: string) => boolean;
  togglePin: (item: SearchResultItem) => void;
  searchInputRef: React.RefObject<HTMLInputElement>;
  rowRefs: React.MutableRefObject<Map<string, HTMLElement>>;
  // select/deselect
  onClickOutside: (e: React.MouseEvent<HTMLElement>) => void;
}

export function SearchView(props: SearchViewProps) {
  const {
    t, query, setQuery, pathPrefix, setPathPrefix, isIndexLoading,
    activeTab, setActiveTab, regexEnabled, setRegexEnabled, fuzzyEnabled, setFuzzyEnabled,
    caseSensitive, setCaseSensitive,
    pathPickerRef, pathInputRef, pathSuggestions, isPathDropdownOpen, setIsPathDropdownOpen,
    activePathSuggestion, setActivePathSuggestion, isPickingPath, pickPath,
    handlePathInputKeyDown, isPathDropdownVisible, visiblePathSuggestions,
    closePathDropdown, applyPathSuggestion,
    timeFilter, setTimeFilter, showTimePopover, setShowTimePopover,
    showSizePopover, setShowSizePopover, timePopoverRef, sizePopoverRef,
    calMonth, setCalMonth, calYear, setCalYear, calSelecting, setCalSelecting,
    calFrom, setCalFrom, calTo, setCalTo,
    customTimeFromRef, customTimeToRef, customTimeLabel, setFilterVersion,
    sizeFilter, setSizeFilter, customSizeMin, setCustomSizeMinTemp,
    customSizeMax, setCustomSizeMaxTemp, customSizeMinUnit, setCustomSizeMinUnit,
    customSizeMaxUnit, setCustomSizeMaxUnit, customSizeMinRef, customSizeMaxRef, customSizeLabel,
    appFilter, setAppFilter, appFilterOptions,
    filteredItems, gridTemplateColumns, sortKey, sortAscending, toggleHeaderSort,
    columnWidths, activeResizer, startResize,
    tableShellRef, tableBodyRef, scrollTop, setScrollTop,
    visibleStart, visibleEnd, visibleItems, topSpacerHeight, bottomSpacerHeight,
    handleScrollbarScroll,
    openResult, handleRowClick, openResultContextMenu,
    isPinned, togglePin, searchInputRef, rowRefs,
    selectedItemPathSet, onClickOutside,
    isSearching, tookMs, indexed, buildStatus,
  } = props;

  const formatIndexedItems = (count: number): string => fmt(t.indexedItems, { count: count.toLocaleString() });
  const formatShownItems = (count: number): string => fmt(t.shownItems, { count: count.toLocaleString() });
  const tabLabel = (tab: TabId): string => t[`tab_${tab}` as const];

  return (
    <div className="search-view" onClick={onClickOutside}>
      <div className="search-main">
        <div className="search-hero">
          <div className="search-input-container">
            <span className="search-icon-left">⌕</span>
            <input
              ref={searchInputRef}
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
            <span className="search-kbd">⌘F</span>
          </div>
        </div>

        <div className="filter-chips">
          {TAB_IDS.map((tab) => (
            <button
              key={tab}
              className={tab === activeTab ? "chip-btn active" : "chip-btn"}
              onClick={() => setActiveTab(tab)}
            >
              <span className="chip-icon">{TAB_ICONS[tab]}</span>
              {tabLabel(tab)}
            </button>
          ))}
        </div>

        <div className="filter-toolbar">
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
                onClick={() => {
                  if (!isIndexLoading && pathPrefix.trim().length > 0) {
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
              {pathPrefix.trim().length > 0 && (
                <button
                  type="button"
                  className="path-clear-btn"
                  aria-label="Clear path filter"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    setPathPrefix("");
                    pathInputRef.current?.focus();
                  }}
                >&#10005;</button>
              )}
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
                      <span className="path-suggest-icon">›</span>
                      <span className="path-suggest-text">{path}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
            <button
              className="act-btn"
              onClick={() => void pickPath()}
              disabled={isPickingPath || isIndexLoading}
            >
              {t.choosePath}
            </button>
          </div>

          <button
            className={regexEnabled ? "toggle-btn active" : "toggle-btn"}
            onClick={() => {
              if (fuzzyEnabled) setFuzzyEnabled(false);
              setRegexEnabled((prev) => !prev);
            }}
            title={t.regexEnabled}
          >
            {t.regexEnabled}
          </button>
          <button
            className={fuzzyEnabled ? "toggle-btn active" : "toggle-btn"}
            onClick={() => {
              if (regexEnabled) setRegexEnabled(false);
              setFuzzyEnabled((prev) => !prev);
            }}
            title={t.fuzzyEnabled}
          >
            {t.fuzzyEnabled}
          </button>
          <button
            className={caseSensitive ? "toggle-btn active" : "toggle-btn"}
            onClick={() => setCaseSensitive((prev) => !prev)}
            title={t.caseSensitive}
          >
            Aa
          </button>

          <span className="toolbar-sep" />

          {/* Time filter */}
          <div style={{ position: "relative", flex: "1 1 0", minWidth: 0 }}>
            <CustomSelect
              value={timeFilter}
              title={customTimeLabel ?? undefined}
              options={[
                { value: "all", label: t.timeAll },
                { value: "today", label: t.timeToday },
                { value: "week", label: t.timeWeek },
                { value: "month", label: t.timeMonth },
                { value: "year", label: t.timeYear },
                { value: "custom", label: customTimeLabel ?? t.timeCustom },
              ]}
              onChange={(v) => {
                if (v === "custom") {
                  if (customTimeFromRef.current) {
                    setCalFrom(new Date(customTimeFromRef.current).toISOString().slice(0, 10));
                  } else { setCalFrom(""); }
                  if (customTimeToRef.current) {
                    setCalTo(new Date(customTimeToRef.current - 86399999).toISOString().slice(0, 10));
                  } else { setCalTo(""); }
                  setCalSelecting("from");
                  setShowTimePopover(true); setShowSizePopover(false);
                } else {
                  setShowTimePopover(false);
                  customTimeFromRef.current = 0; customTimeToRef.current = 0;
                  setTimeFilter(v);
                }
              }}
            />
            {showTimePopover && (
              <div className="filter-popover" ref={timePopoverRef} onClick={(e) => e.stopPropagation()}>
                <div className="cal-header">
                  <button className="cal-nav" onClick={() => { if (calMonth === 0) { setCalMonth(11); setCalYear(y => y - 1); } else setCalMonth(m => m - 1); }}>‹</button>
                  <span className="cal-title">{fmt(t.calYearMonth, { year: calYear, month: calMonth + 1 })}</span>
                  <button className="cal-nav" onClick={() => { if (calMonth === 11) { setCalMonth(0); setCalYear(y => y + 1); } else setCalMonth(m => m + 1); }}>›</button>
                </div>
                <div className="cal-weekdays">
                  {t.calWeekdays.split(",").map((d: string) => <span key={d} className="cal-wd">{d}</span>)}
                </div>
                <div className="cal-grid">
                  {(() => {
                    const first = new Date(calYear, calMonth, 1).getDay();
                    const days = new Date(calYear, calMonth + 1, 0).getDate();
                    const cells: Array<number | null> = [];
                    for (let i = 0; i < first; i++) cells.push(null);
                    for (let d = 1; d <= days; d++) cells.push(d);
                    const fromTs = calFrom ? new Date(calFrom).getTime() : 0;
                    const toTs = calTo ? new Date(calTo).getTime() + 86399999 : 0;
                    return cells.map((d, i) => {
                      if (d === null) return <span key={`e${i}`} className="cal-day empty" />;
                      const ds = `${calYear}-${String(calMonth+1).padStart(2,"0")}-${String(d).padStart(2,"0")}`;
                      const ts = new Date(ds).getTime();
                      const inRange = fromTs && toTs && ts >= fromTs && ts <= toTs;
                      const isFrom = calFrom === ds;
                      const isTo = calTo === ds;
                      let cls = "cal-day";
                      if (isFrom || isTo) cls += " cal-range-edge";
                      else if (inRange) cls += " cal-in-range";
                      return (
                        <span key={ds} className={cls} onClick={() => {
                          if (calSelecting === "from") {
                            setCalFrom(ds); setCalTo(""); setCalSelecting("to");
                          } else {
                            if (ds < calFrom) { setCalTo(calFrom); setCalFrom(ds); }
                            else setCalTo(ds);
                            setCalSelecting("from");
                          }
                        }}>{d}</span>
                      );
                    });
                  })()}
                </div>
                <div className="filter-popover-row" style={{ marginTop: 8 }}>
                  <label style={{ fontSize: "0.8rem" }}>{t.calFrom}</label>
                  <input className="filter-input" readOnly value={calFrom} style={{ fontSize: "0.85rem" }} />
                  <label style={{ fontSize: "0.8rem", width: 24 }}>{t.calTo}</label>
                  <input className="filter-input" readOnly value={calTo} style={{ fontSize: "0.85rem" }} />
                </div>
                <div className="filter-popover-actions">
                  <button className="act-btn cancel" onClick={() => {
                    setCalFrom(""); setCalTo(""); setCalSelecting("from");
                  }}>{t.calClear}</button>
                  <button className="act-btn" onClick={() => {
                    customTimeFromRef.current = calFrom ? new Date(calFrom).getTime() : 0;
                    customTimeToRef.current = calTo ? new Date(calTo).getTime() + 86399999 : 0;
                    setTimeFilter("custom");
                    setShowTimePopover(false);
                    setFilterVersion(v => v + 1);
                  }}>{t.shortcutApply}</button>
                </div>
              </div>
            )}
          </div>

          {/* Size filter */}
          <div style={{ position: "relative", flex: "1 1 0", minWidth: 0 }}>
            <CustomSelect
              value={sizeFilter}
              options={[
                { value: "all", label: t.sizeAll },
                { value: "kb", label: t.sizeKB },
                { value: "mb", label: t.sizeMB },
                { value: "gb", label: t.sizeGB },
                { value: "custom", label: customSizeLabel ?? t.sizeCustom },
              ]}
              onChange={(v) => {
                if (v === "custom") {
                  setShowSizePopover(true); setShowTimePopover(false);
                } else {
                  setShowSizePopover(false);
                  customSizeMinRef.current = 0; customSizeMaxRef.current = 0;
                  setSizeFilter(v);
                }
              }}
            />
            {showSizePopover && (
              <div className="filter-popover" ref={sizePopoverRef} onClick={(e) => e.stopPropagation()}>
                <div className="filter-popover-row">
                  <label>{t.sizeMin}</label>
                  <input className="filter-input" type="text" inputMode="decimal" placeholder="0"
                    value={customSizeMin} onChange={(e) => setCustomSizeMinTemp(e.target.value)} />
                  <CustomSelect
                    value={customSizeMinUnit}
                    options={[
                      { value: "", label: t.sizeUnit },
                      { value: "KB", label: "KB" },
                      { value: "MB", label: "MB" },
                      { value: "GB", label: "GB" },
                    ]}
                    onChange={(v) => setCustomSizeMinUnit(v)}
                  />
                </div>
                <div className="filter-popover-row">
                  <label>{t.sizeMax}</label>
                  <input className="filter-input" type="text" inputMode="decimal" placeholder="100"
                    value={customSizeMax} onChange={(e) => setCustomSizeMaxTemp(e.target.value)} />
                  <CustomSelect
                    value={customSizeMaxUnit}
                    options={[
                      { value: "", label: t.sizeUnit },
                      { value: "KB", label: "KB" },
                      { value: "MB", label: "MB" },
                      { value: "GB", label: "GB" },
                    ]}
                    onChange={(v) => setCustomSizeMaxUnit(v)}
                  />
                </div>
                <div className="filter-popover-actions">
                  <button className="act-btn" onClick={() => {
                    const parseVal = (v: string, unit: string) => {
                      const n = parseFloat(v) || 0;
                      if (unit === "GB") return n * 1073741824;
                      if (unit === "MB") return n * 1048576;
                      if (unit === "KB") return n * 1024;
                      return n;
                    };
                    customSizeMinRef.current = parseVal(customSizeMin, customSizeMinUnit);
                    customSizeMaxRef.current = parseVal(customSizeMax, customSizeMaxUnit);
                    setSizeFilter("custom");
                    setShowSizePopover(false);
                    setFilterVersion(v => v + 1);
                  }}>{t.shortcutApply}</button>
                </div>
              </div>
            )}
          </div>

          {/* App filter */}
          <div style={{ flex: "1 1 0", minWidth: 0 }}>
            <CustomSelect
              value={appFilter}
              options={[
                { value: "", label: t.appFilterAll },
                ...appFilterOptions.map((app) => ({ value: app, label: app })),
              ]}
              onChange={(v) => setAppFilter(v)}
            />
          </div>
        </div>

        <div className="results-area" ref={tableShellRef}>
          {filteredItems.length === 0 ? (
            <div className="empty-state">
              {query.trim().length === 0
                ? t.emptyTypeHint
                : t.emptyNoMatch}
            </div>
          ) : (
            <>
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

              <div className="table-body custom-scrollbar" ref={tableBodyRef}
                onScroll={(e) => { setScrollTop((e.target as HTMLDivElement).scrollTop); handleScrollbarScroll(e); }}>
                <div style={{ height: topSpacerHeight }} />
                {visibleItems.map((item, vi) => {
                  const index = visibleStart + vi;
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
                      className={selectedItemPathSet.has(item.path) ? "result-row selected" : "result-row"}
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
                      <div className="cell type-cell">{typeLabel(item, t.typeFolder, t.typeFile)}</div>
                      <div className="cell size-cell">{formatBytes(item.sizeBytes)}</div>
                      <div className="cell date-cell">{formatDate(item.modifiedUnixMs)}</div>
                      <button
                        className="term-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          void invoke("open_in_default_terminal", { path: item.path });
                        }}
                        title={t.openInDefaultTerminal}
                      >
                        &gt;_
                      </button>
                      <button
                        className={`pin-btn ${isPinned(item.path) ? "pinned" : ""}`}
                        onClick={(e) => {
                          e.stopPropagation();
                          togglePin(item);
                        }}
                        title={isPinned(item.path) ? t.menuUnpin : t.menuPin}
                      >
                        {isPinned(item.path) ? "★" : "☆"}
                      </button>
                    </article>
                  );
                })}
                <div style={{ height: bottomSpacerHeight }} />
              </div>
            </>
          )}
        </div>
      </div>

      <footer className="status-bar">
        <div className="status-left">
          {buildStatus ? (
            <span className="status-highlight">{buildStatus}</span>
          ) : (
            <span className="status-highlight">{formatIndexedItems(indexed)}</span>
          )}
        </div>
        <div className="status-right">
          <span className="status-highlight">{formatShownItems(filteredItems.length)}</span>
          <span>{isSearching ? t.searching : `${tookMs} ms`}</span>
        </div>
      </footer>
    </div>
  );
}
