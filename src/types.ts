export type TabId = "all" | "files" | "folders" | "documents" | "images" | "media" | "code" | "archives";
export type SortKey = "name" | "path" | "type" | "size" | "modified";
export type ColumnKey = "name" | "path" | "type" | "size" | "modified";
export type ViewMode = "search" | "pinned" | "settings";
export type VolumeEventType = { type: "mountDetected"; path: string; name: string }
  | { type: "indexComplete"; path: string; fileCount: number; totalIndexed: number }
  | { type: "volumeRemoved"; path: string; name: string; totalIndexed: number };
export type Language = "zh" | "en";
export type ExcludeRuleType = "exact" | "pattern";

export interface SearchResultItem {
  name: string;
  path: string;
  parent: string;
  isDir: boolean;
  isFile: boolean;
  sizeBytes?: number;
  modifiedUnixMs?: number;
}

export interface SearchResponse {
  items: SearchResultItem[];
  total: number;
  tookMs: number;
}

export interface InitResponse {
  indexed: number;
  hasIndex: boolean;
  lastEventId?: number;
}

export interface BuildResponse {
  indexed: number;
  tookMs: number;
}

export interface BuildEvent {
  phase: "started" | "finished";
  indexed?: number;
  tookMs?: number;
}

export interface WatchResponse {
  running: boolean;
  mode: string;
  code: string;
  message: string;
  lastEventId?: number;
}

export interface LaunchSettingsResponse {
  launchAtLogin: boolean;
  silentStart: boolean;
  showDockIcon: boolean;
}

export interface AutoVacuumSettingsResponse {
  autoVacuumOnRebuild: boolean;
}

export interface ExcludeDirSettingsResponse {
  exactDirs: string[];
  patternDirs: string[];
}

export interface ExcludeFileSettingsResponse {
  excludeDotFiles: boolean;
  filePatterns: string[];
}

export interface WatchRootsSettingsResponse {
  roots: string[];
}

export interface FileManagerSettingsResponse {
  defaultFolderAction: string;
  defaultTerminalAction: string;
  customFolderApp: string;
  customTerminalApp: string;
}

export interface ContextMenuState {
  x: number;
  y: number;
  item: SearchResultItem;
  multiSelection: boolean;
}
