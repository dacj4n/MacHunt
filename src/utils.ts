import type { ColumnKey, Language, SearchResultItem, SortKey, TabId } from "./types";

// ── Constants ──
export const DEFAULT_WINDOW_TOGGLE_SHORTCUT = "CmdOrCtrl+Shift+KeyD";
export const DEFAULT_COLUMN_WIDTHS: Record<ColumnKey, number> = {
  name: 500,
  path: 420,
  type: 140,
  size: 160,
  modified: 300
};

export const MIN_COLUMN_WIDTHS: Record<ColumnKey, number> = {
  name: 160,
  path: 120,
  type: 90,
  size: 72,
  modified: 120
};
export const COLUMN_KEYS: ColumnKey[] = ["name", "path", "type", "size", "modified"];

export const LANGUAGE_STORAGE_KEY = "machunt.language";
export const COLUMN_WIDTHS_STORAGE_KEY = "machunt.table.column.widths";
export const LEGACY_SEARCH_MODE_STORAGE_KEY = "machunt.search.mode";
export const REGEX_ENABLED_STORAGE_KEY = "machunt.search.regex_enabled";
export const CASE_SENSITIVE_STORAGE_KEY = "machunt.search.case_sensitive";
export const FUZZY_ENABLED_STORAGE_KEY = "machunt.search.fuzzy_enabled";
export const PINNED_STORAGE_KEY = "machunt.pinned.items";
export const EVENT_OPEN_SETTINGS = "app://open-settings";
export const EVENT_FOCUS_SEARCH = "app://focus-search";
export const EVENT_OPEN_PINNED = "app://open-pinned";
export const TAB_IDS: TabId[] = ["all", "files", "folders", "documents", "images", "media", "code", "archives"];

export const TAB_EXTENSIONS: Record<TabId, string[] | null> = {
  all: null,
  files: null,
  folders: null,
  documents: ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md"],
  images: ["png", "jpg", "jpeg", "gif", "webp", "svg", "heic", "bmp", "tif", "tiff", "eps", "raw", "cr2", "nef", "arw", "avif", "hdr", "exr", "ico", "icns"],
  media: ["mp3", "m4a", "wav", "flac", "aac", "ogg", "mp4", "mov", "avi", "mkv", "webm", "wmv", "mts", "aiff"],
  code: ["rs", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml", "py", "go", "java", "c", "cpp", "h", "hpp", "html", "css", "scss", "less", "vue", "rb", "php", "sql", "sh", "bash", "zsh", "swift", "m", "mm", "xml"],
  archives: ["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "tgz", "dmg", "iso"],
};

export const TAB_ICONS: Record<TabId, string> = {
  all: "\u229E", files: "\u25A3", folders: "\u25A4", documents: "\u2261", images: "\u25C9", media: "\u266A", code: "\u2329\u232A", archives: "\u25A0"
};

// ── Extension → Application name mapping ──
const EXT_APP_MAP: Record<string, string> = {
  // ═══ Adobe Creative Cloud ═══
  ai: "Adobe Illustrator", ait: "Adobe Illustrator", eps: "Adobe Illustrator",
  psd: "Adobe Photoshop", psb: "Adobe Photoshop", psp: "Adobe Photoshop",
  aco: "Adobe Photoshop", abr: "Adobe Photoshop", pat: "Adobe Photoshop",
  csh: "Adobe Photoshop", grd: "Adobe Photoshop", ase: "Adobe Photoshop",
  indd: "Adobe InDesign", indt: "Adobe InDesign", idml: "Adobe InDesign", indb: "Adobe InDesign",
  aep: "Adobe After Effects", aet: "Adobe After Effects", aepx: "Adobe After Effects",
  prproj: "Adobe Premiere Pro", prel: "Adobe Premiere Pro",
  lrcat: "Adobe Lightroom", xmp: "Adobe Lightroom", lrtemplate: "Adobe Lightroom",
  dng: "Adobe Lightroom",
  fla: "Adobe Animate", xfl: "Adobe Animate",
  swf: "Adobe Flash",
  xd: "Adobe XD",
  pdf: "Adobe Acrobat",
  // ═══ Apple 专业应用 ═══
  fcpxml: "Final Cut Pro", fcpx: "Final Cut Pro",
  aupreset: "Logic Pro",
  band: "GarageBand",
  // ═══ Sketch / Figma ═══
  sketch: "Sketch",
  fig: "Figma", jam: "Figma",
  // ═══ Affinity Suite ═══
  afdesign: "Affinity Designer", afphoto: "Affinity Photo", afpub: "Affinity Publisher",
  // ═══ Corel ═══
  cdr: "CorelDRAW", cdt: "CorelDRAW", cmx: "CorelDRAW",
  // ═══ 像素画 ═══
  pxm: "Pixelmator Pro",
  aseprite: "Aseprite",
  // ═══ DaVinci Resolve ═══
  drp: "DaVinci Resolve",
  // ═══ Capture One ═══
  cos: "Capture One", eip: "Capture One",
  // ═══ 3D / CAD ═══
  blend: "Blender", blend1: "Blender",
  c4d: "Cinema 4D",
  skp: "SketchUp",
  max: "3ds Max", "3ds": "3ds Max",
  ma: "Maya", mb: "Maya",
  fbx: "预览", obj: "预览", stl: "预览", glb: "预览", gltf: "预览",
  usdz: "预览", usd: "预览", usda: "预览", usdc: "预览",
  // ═══ 字体 ═══
  ttf: "字体册", otf: "字体册", woff: "字体册", woff2: "字体册",
  // ═══ 通用图像 ═══
  jpg: "预览", jpeg: "预览", jpe: "预览",
  png: "预览", gif: "预览", webp: "预览",
  bmp: "预览", heic: "预览", heif: "预览",
  tif: "预览", tiff: "预览",
  ico: "预览", icns: "预览",
  svg: "预览", svgz: "预览",
  raw: "预览", cr2: "预览", cr3: "预览", crw: "预览",
  nef: "预览", nrw: "预览",
  arw: "预览", srf: "预览", sr2: "预览",
  orf: "预览", rw2: "预览",
  pef: "预览", raf: "预览",
  dcr: "预览", kdc: "预览", mrw: "预览",
  "3fr": "预览", fff: "预览",
  avif: "预览", hdr: "预览", exr: "预览",
  // ═══ 音视频 ═══
  mp4: "QuickTime Player", m4v: "QuickTime Player", mov: "QuickTime Player",
  avi: "QuickTime Player", mkv: "QuickTime Player", webm: "QuickTime Player",
  wmv: "QuickTime Player", flv: "QuickTime Player",
  mts: "QuickTime Player", m2ts: "QuickTime Player",
  mp3: "音乐", m4a: "音乐", m4r: "音乐",
  wav: "音乐", aiff: "音乐", aif: "音乐",
  flac: "音乐", aac: "音乐", ogg: "音乐",
  wma: "音乐", caf: "音乐",
  // ═══ 办公文档 ═══
  doc: "Microsoft Word", docx: "Microsoft Word", dot: "Microsoft Word", dotx: "Microsoft Word",
  xls: "Microsoft Excel", xlsx: "Microsoft Excel", xlt: "Microsoft Excel", xltx: "Microsoft Excel",
  csv: "Microsoft Excel",
  ppt: "Microsoft PowerPoint", pptx: "Microsoft PowerPoint", pot: "Microsoft PowerPoint", potx: "Microsoft PowerPoint",
  pages: "Pages", numbers: "Numbers", key: "Keynote", kth: "Keynote",
  txt: "文本编辑", md: "文本编辑", rtf: "文本编辑", rtfd: "文本编辑",
  // ═══ 代码 / Web ═══
  html: "Safari 浏览器", htm: "Safari 浏览器", css: "Safari 浏览器", xml: "Safari 浏览器",
  swift: "Xcode", c: "Xcode", cpp: "Xcode", h: "Xcode", hpp: "Xcode", m: "Xcode", mm: "Xcode",
  playground: "Xcode", xcodeproj: "Xcode", xcworkspace: "Xcode",
  storyboard: "Xcode", xib: "Xcode", plist: "Xcode",
  rs: "VS Code", ts: "VS Code", tsx: "VS Code", js: "VS Code", jsx: "VS Code",
  json: "VS Code", toml: "VS Code", yaml: "VS Code", yml: "VS Code",
  py: "VS Code", go: "VS Code", java: "VS Code", rb: "VS Code", php: "VS Code", sql: "VS Code",
  scss: "VS Code", less: "VS Code", sass: "VS Code", vue: "VS Code", svelte: "VS Code",
  sh: "终端", bash: "终端", zsh: "终端", fish: "终端", command: "终端",
  // ═══ 压缩 / 磁盘 ═══
  zip: "归档实用工具", rar: "归档实用工具", "7z": "归档实用工具",
  tar: "归档实用工具", gz: "归档实用工具", bz2: "归档实用工具", xz: "归档实用工具",
  tgz: "归档实用工具", tbz2: "归档实用工具",
  dmg: "磁盘工具", iso: "磁盘工具",
  sparseimage: "磁盘工具", sparsebundle: "磁盘工具",
};

// ── Utility Functions ──

export function extensionOf(name: string): string {
  const idx = name.lastIndexOf(".");
  if (idx < 0 || idx === name.length - 1) {
    return "";
  }
  return name.slice(idx + 1).toLowerCase();
}

export function classifyTab(item: SearchResultItem): TabId {
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

export function filterByTab(items: SearchResultItem[], tab: TabId): SearchResultItem[] {
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

export function sortItems(items: SearchResultItem[], key: SortKey, ascending: boolean): SearchResultItem[] {
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

export function formatBytes(bytes?: number): string {
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

export function formatDate(ms?: number): string {
  if (!ms) {
    return "--";
  }
  return new Date(ms).toLocaleString();
}

export function parseSize(input: string): number {
  if (!input.trim()) return 0;
  const s = input.trim().toLowerCase();
  const num = parseFloat(s);
  if (isNaN(num)) return 0;
  if (s.endsWith("gb") || s.endsWith("g")) return num * 1073741824;
  if (s.endsWith("mb") || s.endsWith("m")) return num * 1048576;
  if (s.endsWith("kb") || s.endsWith("k")) return num * 1024;
  return num; // raw bytes
}

export function appForExt(ext: string): string {
  if (!ext) return "Other";
  return EXT_APP_MAP[ext.toLowerCase()] || "Other";
}

export function iconToken(item: SearchResultItem): string {
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

export function typeLabel(item: SearchResultItem, tFolder: string, tFile: string): string {
  if (item.isDir) {
    return tFolder;
  }
  const ext = extensionOf(item.name);
  if (ext.length > 0) {
    return ext.toUpperCase();
  }
  return tFile;
}

export function typeSortKey(item: SearchResultItem): string {
  if (item.isDir) {
    return "0-folder";
  }

  const ext = extensionOf(item.name);
  if (ext.length > 0) {
    return `1-${ext.toLowerCase()}`;
  }
  return "1-file";
}

export function iconGlyph(token: string): string {
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

export function setCellPreviewTooltip(
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

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable;
}

export function blurActiveEditable(): void {
  if (typeof document === "undefined") {
    return;
  }
  const active = document.activeElement;
  if (active instanceof HTMLElement && isEditableTarget(active)) {
    active.blur();
  }
}

export function buildSearchRequest(
  query: string,
  tab: TabId,
  pathPrefix: string,
  caseSensitive: boolean,
  regexEnabled: boolean,
  fuzzyEnabled: boolean,
  sortKey: SortKey,
  sortAscending: boolean,
  limit: number
) {
  const includeFiles = tab !== "folders";
  const includeDirs = tab === "all" || tab === "folders";
  const extensions = TAB_EXTENSIONS[tab];
  const mode = fuzzyEnabled ? "Fuzzy" : regexEnabled ? "Pattern" : "Substring";
  // Pass null as limit so the engine returns all matching results without truncation.
  // Frontend applies size/time/app filters on the full dataset, then
  // virtual-scroll renders only visible rows (controlled by maxResults display setting).
  return {
    request: {
      query,
      mode,
      regexEnabled,
      caseSensitive,
      pathPrefix: pathPrefix.trim() || null,
      includeFiles,
      includeDirs,
      limit: null,
      extensions,
      sortKey,
      sortAscending,
    }
  };
}

export function detectDefaultLanguage(): Language {
  if (typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")) {
    return "zh";
  }
  return "en";
}

export function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") {
    return true;
  }
  return /mac/i.test(navigator.platform);
}

export function displayShortcut(shortcut: string): string {
  const tokens = shortcut
    .split("+")
    .map((token) => token.trim())
    .filter((token) => token.length > 0);
  if (tokens.length === 0) {
    return "";
  }

  const isMac = isMacPlatform();
  return tokens
    .map((token) => {
      const upper = token.toUpperCase();
      if (upper === "CMDORCTRL" || upper === "COMMANDORCONTROL" || upper === "COMMANDORCTRL" || upper === "CMDORCONTROL") {
        return isMac ? "Cmd" : "Ctrl";
      }
      if (upper === "CMD" || upper === "COMMAND" || upper === "SUPER") {
        return "Cmd";
      }
      if (upper === "CTRL" || upper === "CONTROL") {
        return "Ctrl";
      }
      if (upper === "ALT" || upper === "OPTION") {
        return isMac ? "Option" : "Alt";
      }
      if (upper === "SHIFT") {
        return "Shift";
      }
      if (upper.startsWith("KEY") && upper.length === 4) {
        return upper.slice(3);
      }
      if (upper.startsWith("DIGIT") && upper.length === 6) {
        return upper.slice(5);
      }
      if (upper === "SPACE") {
        return "Space";
      }
      return token;
    })
    .join("+");
}

export function shortcutFromKeyboardEvent(event: React.KeyboardEvent<HTMLInputElement>): string | null {
  const modifierCodes = new Set([
    "MetaLeft",
    "MetaRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "ShiftLeft",
    "ShiftRight"
  ]);

  const modifiers: string[] = [];
  if (event.metaKey || event.ctrlKey) {
    modifiers.push("CmdOrCtrl");
  }
  if (event.altKey) {
    modifiers.push("Alt");
  }
  if (event.shiftKey) {
    modifiers.push("Shift");
  }
  if (modifiers.length === 0 || modifierCodes.has(event.code)) {
    return null;
  }

  let keyToken = event.code;
  if (keyToken === "NumpadDecimal") {
    keyToken = "Period";
  }
  if (keyToken === "NumpadAdd") {
    keyToken = "Equal";
  }
  return `${modifiers.join("+")}+${keyToken}`;
}

export function fmt(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(vars[key] ?? ""));
}

export function normalizeStoredColumnWidth(value: unknown, key: ColumnKey): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }
  const min = MIN_COLUMN_WIDTHS[key];
  const max = 2200;
  return Math.round(Math.max(min, Math.min(max, value)));
}

export function loadStoredColumnWidths(): Record<ColumnKey, number> | null {
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

export function loadStoredRegexEnabled(): boolean | null {
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

export function loadStoredCaseSensitive(): boolean | null {
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

export function loadStoredFuzzyEnabled(): boolean | null {
  if (typeof window === "undefined") {
    return null;
  }
  const raw = window.localStorage.getItem(FUZZY_ENABLED_STORAGE_KEY);
  if (raw === "1") return true;
  if (raw === "0") return false;
  return null;
}

export function loadPinnedItems(): SearchResultItem[] {
  if (typeof window === "undefined") {
    return [];
  }
  try {
    const raw = window.localStorage.getItem(PINNED_STORAGE_KEY);
    if (!raw) return [];
    return JSON.parse(raw) as SearchResultItem[];
  } catch {
    return [];
  }
}

export function savePinnedItems(items: SearchResultItem[]) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(PINNED_STORAGE_KEY, JSON.stringify(items));
}
