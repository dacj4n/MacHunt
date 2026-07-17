import { CustomSelect } from "./CustomSelect";
import type { Language, ThemeMode, ExcludeRuleType, FileManagerSettingsResponse } from "../types";
import { displayShortcut, shortcutFromKeyboardEvent } from "../utils";

export interface SettingsViewProps {
  t: Record<string, string>;
  themeMode: ThemeMode;
  setThemeMode: (v: ThemeMode) => void;
  resolvedTheme: "light" | "dark";
  language: Language;
  setLanguage: (v: Language) => void;
  windowToggleShortcut: string;
  shortcutDraft: string;
  setShortcutDraft: (v: string) => void;
  shortcutStatus: string;
  setShortcutStatus: (v: string) => void;
  isShortcutSaving: boolean;
  applyWindowToggleShortcut: (s: string) => Promise<void>;
  resetWindowToggleShortcut: () => Promise<void>;
  launchAtLogin: boolean;
  silentStart: boolean;
  showDockIcon: boolean;
  isLaunchSettingsSaving: boolean;
  launchSettingsStatus: string;
  applyLaunchSettings: (launchAtLogin: boolean, silentStart: boolean, showDockIcon: boolean) => Promise<void>;
  maxResults: number;
  isMaxResultsSaving: boolean;
  maxResultsStatus: string;
  applyMaxResults: (n: number) => Promise<void>;
  appVersion: string;
  autoCheckUpdate: boolean;
  isAutoCheckSaving: boolean;
  autoCheckStatus: string;
  applyAutoCheckUpdate: (v: boolean) => Promise<void>;
  isCheckingUpdate: boolean;
  updateInfo: { hasUpdate: boolean; latestVersion: string } | null;
  checkForUpdatesManually: () => Promise<void>;
  openUrl: (url: string) => Promise<void>;
  defaultFolderAction: string;
  setDefaultFolderAction: (v: string) => void;
  defaultTerminalAction: string;
  setDefaultTerminalAction: (v: string) => void;
  customFolderApp: string;
  setCustomFolderApp: (v: string) => void;
  customTerminalApp: string;
  setCustomTerminalApp: (v: string) => void;
  applyFileManagerSettings: (folderAction: string, terminalAction: string, customFolderApp: string, customTerminalApp: string) => Promise<void>;
  pickApp: () => Promise<string | null>;
  autoVacuumOnRebuild: boolean;
  isAutoVacuumSettingsSaving: boolean;
  autoVacuumSettingsStatus: string;
  applyAutoVacuumSettings: (v: boolean) => Promise<void>;
  watchRootDraft: string;
  setWatchRootDraft: (v: string) => void;
  watchRoots: string[];
  isWatchRootSaving: boolean;
  watchRootStatus: string;
  addWatchRoot: () => Promise<void>;
  removeWatchRoot: (root: string) => Promise<void>;
  pickWatchRoot: () => Promise<void>;
  excludeRuleType: ExcludeRuleType;
  setExcludeRuleType: (v: ExcludeRuleType) => void;
  excludeRuleDraft: string;
  setExcludeRuleDraft: (v: string) => void;
  excludeExactDirs: string[];
  excludePatternDirs: string[];
  excludeDirStatus: string;
  isExcludeDirSaving: boolean;
  addExcludeRule: () => Promise<void>;
  removeExcludeRule: (type: ExcludeRuleType, rule: string) => Promise<void>;
  pickExcludeRulePath: () => Promise<void>;
  isPickingPath: boolean;
  excludeDotFiles: boolean;
  toggleExcludeDotFiles: (v: boolean) => Promise<void>;
  excludeFilePatterns: string[];
  excludeFilePatternDraft: string;
  setExcludeFilePatternDraft: (v: string) => void;
  excludeFileStatus: string;
  isExcludeFileSaving: boolean;
  addExcludeFilePattern: () => Promise<void>;
  removeExcludeFilePattern: (pattern: string) => Promise<void>;
  runBuild: (rebuild: boolean) => Promise<void>;
  isBuilding: boolean;
  isWatchRunning: boolean;
  isWatchPending: boolean;
  toggleWatch: () => Promise<void>;
  handleScrollbarScroll: (e: React.UIEvent<HTMLElement>) => void;
}

export function SettingsView(props: SettingsViewProps) {
  const {
    t, themeMode, setThemeMode, resolvedTheme,
    language, setLanguage,
    windowToggleShortcut, shortcutDraft, setShortcutDraft,
    shortcutStatus, setShortcutStatus, isShortcutSaving,
    applyWindowToggleShortcut, resetWindowToggleShortcut,
    launchAtLogin, silentStart, showDockIcon,
    isLaunchSettingsSaving, launchSettingsStatus,
    applyLaunchSettings,
    maxResults, isMaxResultsSaving, maxResultsStatus, applyMaxResults,
    appVersion,
    autoCheckUpdate, isAutoCheckSaving, autoCheckStatus, applyAutoCheckUpdate,
    isCheckingUpdate, updateInfo, checkForUpdatesManually, openUrl,
    defaultFolderAction, setDefaultFolderAction,
    defaultTerminalAction, setDefaultTerminalAction,
    customFolderApp, setCustomFolderApp,
    customTerminalApp, setCustomTerminalApp,
    applyFileManagerSettings, pickApp,
    autoVacuumOnRebuild, isAutoVacuumSettingsSaving, autoVacuumSettingsStatus,
    applyAutoVacuumSettings,
    watchRootDraft, setWatchRootDraft, watchRoots, isWatchRootSaving, watchRootStatus,
    addWatchRoot, removeWatchRoot, pickWatchRoot,
    excludeRuleType, setExcludeRuleType, excludeRuleDraft, setExcludeRuleDraft,
    excludeExactDirs, excludePatternDirs, excludeDirStatus, isExcludeDirSaving,
    addExcludeRule, removeExcludeRule, pickExcludeRulePath,
    isPickingPath,
    excludeDotFiles, toggleExcludeDotFiles,
    excludeFilePatterns, excludeFilePatternDraft, setExcludeFilePatternDraft,
    excludeFileStatus, isExcludeFileSaving,
    addExcludeFilePattern, removeExcludeFilePattern,
    runBuild, isBuilding, isWatchRunning, isWatchPending, toggleWatch, handleScrollbarScroll,
  } = props;

  const settingsThemeOptions: Array<{ mode: ThemeMode; title: string; description: string }> = [
    { mode: "system", title: t.themeSystemTitle, description: t.themeSystemDesc },
    { mode: "light", title: t.themeLightTitle, description: t.themeLightDesc },
    { mode: "dark", title: t.themeDarkTitle, description: t.themeDarkDesc }
  ];

  const settingsLanguageOptions: Array<{ code: Language; title: string; description: string }> = [
    { code: "zh", title: t.languageZhTitle, description: "界面使用中文。" },
    { code: "en", title: t.languageEnTitle, description: "Interface in English." }
  ];

  return (
    <div className="settings-view custom-scrollbar" onScroll={handleScrollbarScroll}>
      <header className="settings-header">
        <div>
          <h2>{t.settingsTitle}</h2>
          <p>{t.settingsDesc}</p>
        </div>
      </header>

      <div className="settings-grid">
        {/* Appearance Module */}
        <article className="set-card">
          <div className="set-card-header">
            <div className="set-card-icon">◉</div>
            <div>
              <div className="set-card-title">{t.themeModeTitle}</div>
              <div className="set-card-subtitle">{t.themeCurrent}: {resolvedTheme === "dark" ? t.themeDarkTitle : t.themeLightTitle}</div>
            </div>
          </div>
          <div className="theme-cards">
            {settingsThemeOptions.map((option) => (
              <div
                key={option.mode}
                className={themeMode === option.mode ? "tilt-card active" : "tilt-card"}
                role="button"
                tabIndex={0}
                onClick={() => setThemeMode(option.mode)}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setThemeMode(option.mode); } }}
                onMouseMove={(e) => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  const x = ((e.clientX - rect.left) / rect.width - 0.5) * 8;
                  const y = ((e.clientY - rect.top) / rect.height - 0.5) * -8;
                  e.currentTarget.style.setProperty("--tilt-x", `${x}deg`);
                  e.currentTarget.style.setProperty("--tilt-y", `${y}deg`);
                  e.currentTarget.style.setProperty("--glow-x", `${((e.clientX - rect.left) / rect.width) * 100}%`);
                  e.currentTarget.style.setProperty("--glow-y", `${((e.clientY - rect.top) / rect.height) * 100}%`);
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.setProperty("--tilt-x", "0deg");
                  e.currentTarget.style.setProperty("--tilt-y", "0deg");
                }}
              >
                <div className={`theme-preview theme-preview-${option.mode}`}>
                  <div className="tp-bar"/><div className="tp-bar"/><div className="tp-bar"/>
                </div>
                <div className="tilt-card-label"><span className="dot"/>{option.title}</div>
                <div className="tilt-card-desc">{option.description}</div>
              </div>
            ))}
          </div>

          <div className="rule-section">
            <div className="rule-section-title">{t.languageTitle}</div>
            <div className="lang-cards">
              {settingsLanguageOptions.map((option) => (
                <div
                  key={option.code}
                  className={language === option.code ? "tilt-card active" : "tilt-card"}
                  role="button"
                  tabIndex={0}
                  onClick={() => setLanguage(option.code)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setLanguage(option.code); } }}
                  onMouseMove={(e) => {
                    const rect = e.currentTarget.getBoundingClientRect();
                    const x = ((e.clientX - rect.left) / rect.width - 0.5) * 8;
                    const y = ((e.clientY - rect.top) / rect.height - 0.5) * -8;
                    e.currentTarget.style.setProperty("--tilt-x", `${x}deg`);
                    e.currentTarget.style.setProperty("--tilt-y", `${y}deg`);
                    e.currentTarget.style.setProperty("--glow-x", `${((e.clientX - rect.left) / rect.width) * 100}%`);
                    e.currentTarget.style.setProperty("--glow-y", `${((e.clientY - rect.top) / rect.height) * 100}%`);
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.setProperty("--tilt-x", "0deg");
                    e.currentTarget.style.setProperty("--tilt-y", "0deg");
                  }}
                >
                  <div className="tilt-card-label"><span className="dot"/>{option.title}</div>
                  <div className="tilt-card-desc">{option.description}</div>
                </div>
              ))}
            </div>
          </div>
        </article>

        {/* Preferences Module */}
        <article className="set-card">
          <div className="set-card-header">
            <div className="set-card-icon">⚙</div>
            <div>
              <div className="set-card-title">{t.shortcutTitle}</div>
              <div className="set-card-subtitle">{t.shortcutDesc}</div>
            </div>
          </div>
          <p className="set-card-hint">{t.shortcutInputHint}</p>
          <div className="form-row">
            <input
              className="form-input"
              value={displayShortcut(shortcutDraft)}
              placeholder={t.shortcutInputPlaceholder}
              readOnly
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              onFocus={(e) => e.target.select()}
              onKeyDown={(event) => {
                if (event.key === "Tab") {
                  return;
                }
                if (event.key === "Escape") {
                  event.currentTarget.blur();
                  return;
                }
                if (event.key === "Backspace" || event.key === "Delete") {
                  event.preventDefault();
                  setShortcutDraft("");
                  setShortcutStatus("");
                  return;
                }

                const next = shortcutFromKeyboardEvent(event);
                event.preventDefault();
                if (!next) {
                  setShortcutStatus(t.shortcutNeedModifier);
                  return;
                }

                setShortcutDraft(next);
                setShortcutStatus("");
                void applyWindowToggleShortcut(next);
              }}
            />
            <button
              className="act-btn"
              disabled={isShortcutSaving}
              onClick={() => void applyWindowToggleShortcut(shortcutDraft)}
            >
              {t.shortcutApply}
            </button>
            <button
              className="act-btn"
              disabled={isShortcutSaving}
              onClick={() => void resetWindowToggleShortcut()}
            >
              {t.shortcutReset}
            </button>
          </div>
          <div className="option-meta">
            {t.shortcutCurrent}: {displayShortcut(windowToggleShortcut)}
          </div>
          {shortcutStatus && <div className="status-msg">{shortcutStatus}</div>}

          <div className="rule-section">
            <div className="rule-section-title">{t.startupTitle}</div>
            <p className="set-card-desc">{t.startupDesc}</p>
            <div className="toggle-cards">
              <label className="toggle-card">
                <div className="toggle-card-copy">
                  <div className="toggle-card-title">{t.startupLaunchAtLogin}</div>
                  <div className="toggle-card-desc">{t.startupLaunchAtLoginDesc}</div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={launchAtLogin}
                  aria-label={t.startupLaunchAtLogin}
                  className={launchAtLogin ? "ns-switch on" : "ns-switch"}
                  disabled={isLaunchSettingsSaving}
                  onClick={() => void applyLaunchSettings(!launchAtLogin, silentStart, showDockIcon)}
                >
                  <span className="ns-switch-knob" />
                </button>
              </label>
              <label className="toggle-card">
                <div className="toggle-card-copy">
                  <div className="toggle-card-title">{t.startupSilentStart}</div>
                  <div className="toggle-card-desc">{t.startupSilentStartDesc}</div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={silentStart}
                  aria-label={t.startupSilentStart}
                  className={silentStart ? "ns-switch on" : "ns-switch"}
                  disabled={isLaunchSettingsSaving}
                  onClick={() => void applyLaunchSettings(launchAtLogin, !silentStart, showDockIcon)}
                >
                  <span className="ns-switch-knob" />
                </button>
              </label>
              <label className="toggle-card">
                <div className="toggle-card-copy">
                  <div className="toggle-card-title">{t.startupShowDockIcon}</div>
                  <div className="toggle-card-desc">{t.startupShowDockIconDesc}</div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={showDockIcon}
                  aria-label={t.startupShowDockIcon}
                  className={showDockIcon ? "ns-switch on" : "ns-switch"}
                  disabled={isLaunchSettingsSaving}
                  onClick={() => void applyLaunchSettings(launchAtLogin, silentStart, !showDockIcon)}
                >
                  <span className="ns-switch-knob" />
                </button>
              </label>
            </div>
            {launchSettingsStatus && <div className="status-msg">{launchSettingsStatus}</div>}
          </div>
        </article>

        {/* Max Results Module */}
        <article className="set-card">
          <div className="set-card-header">
            <div className="set-card-icon">☰</div>
            <div>
              <div className="set-card-title">{t.maxResultsTitle}</div>
            </div>
          </div>
          <div className="form-row">
            <CustomSelect
              triggerClassName="form-select"
              value={String(maxResults)}
              disabled={isMaxResultsSaving}
              options={[
                { value: "500", label: "500" },
                { value: "1000", label: "1000" },
                { value: "2000", label: "2000" },
                { value: "5000", label: "5000" },
                { value: "10000", label: "10000" },
              ]}
              onChange={(val) => {
                const n = parseInt(val, 10);
                if (!isNaN(n)) {
                  void applyMaxResults(n);
                }
              }}
            />
          </div>
          {maxResultsStatus && <div className="status-msg">{maxResultsStatus}</div>}
        </article>

        {/* About Module */}
        <article className="set-card">
          <div className="set-card-header">
            <div className="set-card-icon">ℹ</div>
            <div>
              <div className="set-card-title">{t.updateTitle}</div>
              <div className="set-card-subtitle">{appVersion ? `${t.versionLabel}: v${appVersion}` : t.updateDesc}</div>
            </div>
          </div>
          <label className="toggle-card">
            <div className="toggle-card-copy">
              <div className="toggle-card-title">{t.updateAutoCheck}</div>
              <div className="toggle-card-desc">{t.updateAutoCheckDesc}</div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={autoCheckUpdate}
              aria-label={t.updateAutoCheck}
              className={autoCheckUpdate ? "ns-switch on" : "ns-switch"}
              disabled={isAutoCheckSaving}
              onClick={() => void applyAutoCheckUpdate(!autoCheckUpdate)}
            >
              <span className="ns-switch-knob" />
            </button>
          </label>
          <div className="update-info">
            {isCheckingUpdate && <div className="update-status">{t.updateChecking}</div>}
            {!isCheckingUpdate && updateInfo && updateInfo.hasUpdate && (
              <div className="update-available">
                <span>{fmtStr(t.updateNewVersion, { version: updateInfo.latestVersion })}</span>
                <button className="act-btn primary"
                  onClick={() => openUrl("https://github.com/dacj4n/MacHunt/releases/latest")}
                >{t.updateDownload}</button>
              </div>
            )}
            {!isCheckingUpdate && updateInfo && !updateInfo.hasUpdate && updateInfo.latestVersion !== "" && (
              <div className="update-status">{t.updateNoUpdate}</div>
            )}
          </div>
          <div className="form-row">
            <button className="act-btn" disabled={isCheckingUpdate} onClick={() => void checkForUpdatesManually()}>
              {t.updateCheckNow}
            </button>
          </div>
          {autoCheckStatus && <div className="status-msg">{autoCheckStatus}</div>}
        </article>

        {/* File Manager & Terminal Module */}
        <article className="set-card">
          <div className="set-card-header">
            <div className="set-card-icon">⌘</div>
            <div>
              <div className="set-card-title">{t.fileManagerTitle}</div>
              <div className="set-card-subtitle">{t.fileManagerDesc}</div>
            </div>
          </div>

          <div className="rule-section">
            <div className="rule-section-title">{t.defaultFolderAction}</div>
            <div className="form-row">
              <CustomSelect
                triggerClassName="form-select"
                value={defaultFolderAction}
                options={[
                  { value: "Finder", label: t.folderActionFinder },
                  { value: "QSpace Pro", label: t.folderActionQSpace },
                  ...(defaultFolderAction && !["Finder", "QSpace Pro"].includes(defaultFolderAction)
                    ? [{
                        value: defaultFolderAction,
                        label: defaultFolderAction.includes("|") ? defaultFolderAction.split("|")[0] : defaultFolderAction,
                      }]
                    : []),
                  { value: "__custom__", label: t.folderActionCustom },
                ]}
                onChange={async (val) => {
                  if (val === "__custom__") {
                    const appStr = await pickApp();
                    if (appStr) {
                      setDefaultFolderAction(appStr);
                      setCustomFolderApp(appStr);
                      void applyFileManagerSettings(appStr, defaultTerminalAction, appStr, customTerminalApp);
                    }
                  } else {
                    setDefaultFolderAction(val);
                    setCustomFolderApp("");
                    void applyFileManagerSettings(val, defaultTerminalAction, "", customTerminalApp);
                  }
                }}
                style={{ flex: 1 }}
              />
            </div>
          </div>

          <div className="rule-section">
            <div className="rule-section-title">{t.defaultTerminalAction}</div>
            <div className="form-row">
              <CustomSelect
                triggerClassName="form-select"
                value={defaultTerminalAction}
                options={[
                  { value: "Terminal", label: t.terminalActionTerminal },
                  { value: "WezTerm", label: t.terminalActionWezTerm },
                  { value: "iTerm", label: "iTerm2" },
                  { value: "kitty", label: "Kitty" },
                  { value: "Warp", label: "Warp" },
                  ...(defaultTerminalAction && !["Terminal", "WezTerm", "iTerm", "kitty", "Warp"].includes(defaultTerminalAction)
                    ? [{
                        value: defaultTerminalAction,
                        label: defaultTerminalAction.includes("|") ? defaultTerminalAction.split("|")[0] : defaultTerminalAction,
                      }]
                    : []),
                  { value: "__custom__", label: t.terminalActionCustom },
                ]}
                onChange={async (val) => {
                  if (val === "__custom__") {
                    const appStr = await pickApp();
                    if (appStr) {
                      setDefaultTerminalAction(appStr);
                      setCustomTerminalApp(appStr);
                      void applyFileManagerSettings(defaultFolderAction, appStr, customFolderApp, appStr);
                    }
                  } else {
                    setDefaultTerminalAction(val);
                    setCustomTerminalApp("");
                    void applyFileManagerSettings(defaultFolderAction, val, customFolderApp, "");
                  }
                }}
                style={{ flex: 1 }}
              />
            </div>
          </div>
        </article>

        {/* Indexing Module */}
        <article className="set-card">
          <div className="set-card-header">
            <div className="set-card-icon">⊞</div>
            <div>
              <div className="set-card-title">{t.autoVacuumTitle}</div>
              <div className="set-card-subtitle">{t.autoVacuumDesc}</div>
            </div>
          </div>

          <div className="form-row" style={{ marginBottom: 12 }}>
            <button className="act-btn" onClick={() => void runBuild(false)} disabled={isBuilding}>
              {t.build}
            </button>
            <button className="act-btn" onClick={() => void runBuild(true)} disabled={isBuilding}>
              {t.rebuild}
            </button>
            <button className={isWatchRunning ? "act-btn danger" : "act-btn primary"} onClick={() => void toggleWatch()} disabled={isWatchPending}>
              <span className={isWatchRunning ? "watch-dot on" : "watch-dot off"} />
              {isWatchPending ? (isWatchRunning ? t.stopping : t.starting) : isWatchRunning ? t.stopWatch : t.startWatch}
            </button>
          </div>

          <label className="toggle-card">
            <div className="toggle-card-copy">
              <div className="toggle-card-title">{t.autoVacuumOn}</div>
              <div className="toggle-card-desc">{t.autoVacuumOnDesc}</div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={autoVacuumOnRebuild}
              aria-label={t.autoVacuumOn}
              className={autoVacuumOnRebuild ? "ns-switch on" : "ns-switch"}
              disabled={isAutoVacuumSettingsSaving}
              onClick={() => void applyAutoVacuumSettings(!autoVacuumOnRebuild)}
            >
              <span className="ns-switch-knob" />
            </button>
          </label>
          {autoVacuumSettingsStatus && <div className="status-msg">{autoVacuumSettingsStatus}</div>}

          <div className="rule-cols">
          <div className="rule-section">
            <div className="rule-section-title">{t.watchRootsTitle}</div>
            <p className="set-card-desc">{t.watchRootsDesc}</p>
            <div className="form-row">
              <input
                className="form-input"
                value={watchRootDraft}
                placeholder={t.watchRootsInputPlaceholder}
                disabled={isWatchRootSaving}
                autoComplete="off"
                autoCorrect="off"
                autoCapitalize="off"
                spellCheck={false}
                onChange={(event) => setWatchRootDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void addWatchRoot();
                  }
                }}
              />
              <button
                className="act-btn"
                disabled={isWatchRootSaving || isPickingPath}
                onClick={() => void pickWatchRoot()}
              >
                {t.choosePath}
              </button>
              <button
                className="act-btn"
                disabled={isWatchRootSaving || watchRootDraft.trim().length === 0}
                onClick={() => void addWatchRoot()}
              >
                {t.excludeAdd}
              </button>
            </div>
            {watchRoots.length === 0 ? (
              <div className="set-card-hint">{t.watchRootsEmptyHint}</div>
            ) : (
              <div className="rule-list">
                {watchRoots.map((root) => (
                  <div key={`watch-root-${root}`} className="rule-item">
                    <span className="rule-tag">root</span>
                    <span className="rule-value">{root}</span>
                    <button
                      className="act-btn"
                      disabled={isWatchRootSaving}
                      onClick={() => void removeWatchRoot(root)}
                    >
                      {t.removeRule}
                    </button>
                  </div>
                ))}
              </div>
            )}
            {watchRootStatus && <div className="status-msg">{watchRootStatus}</div>}
          </div>

          <div className="rule-section">
            <div className="rule-section-title">{t.excludeDirsTitle}</div>
            <p className="set-card-desc">{t.excludeDirsDesc}</p>
            <p className="set-card-hint">{t.excludeWildcardHint}</p>
            <div className="form-row">
              <CustomSelect
                triggerClassName="form-select"
                value={excludeRuleType}
                disabled={isExcludeDirSaving}
                options={[
                  { value: "exact", label: t.excludeRuleExact },
                  { value: "pattern", label: t.excludeRulePattern },
                ]}
                onChange={(val) => setExcludeRuleType(val as ExcludeRuleType)}
                style={{ width: "auto", flexShrink: 0 }}
              />
            </div>
            <div className="form-row">
              <input
                className="form-input"
                value={excludeRuleDraft}
                placeholder={
                  excludeRuleType === "exact"
                    ? t.excludeRuleInputPlaceholderExact
                    : t.excludeRuleInputPlaceholderPattern
                }
                disabled={isExcludeDirSaving}
                autoComplete="off"
                autoCorrect="off"
                autoCapitalize="off"
                spellCheck={false}
                onChange={(event) => setExcludeRuleDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void addExcludeRule();
                  }
                }}
              />
              {excludeRuleType === "exact" && (
                <button
                  className="act-btn"
                  disabled={isExcludeDirSaving || isPickingPath}
                  onClick={() => void pickExcludeRulePath()}
                >
                  {t.choosePath}
                </button>
              )}
              <button
                className="act-btn"
                disabled={isExcludeDirSaving || excludeRuleDraft.trim().length === 0}
                onClick={() => void addExcludeRule()}
              >
                {t.excludeAdd}
              </button>
            </div>

            <div className="rule-section">
              <div className="rule-section-title">{t.excludeExactListTitle}</div>
              {excludeExactDirs.length === 0 ? (
                <div className="set-card-hint">{t.excludeEmptyHint}</div>
              ) : (
                <div className="rule-list">
                  {excludeExactDirs.map((rule) => (
                    <div key={`exact-${rule}`} className="rule-item">
                      <span className="rule-tag">{t.excludeRuleExact}</span>
                      <span className="rule-value">{rule}</span>
                      <button
                        className="act-btn"
                        disabled={isExcludeDirSaving}
                        onClick={() => void removeExcludeRule("exact", rule)}
                      >
                        {t.removeRule}
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="rule-section">
              <div className="rule-section-title">{t.excludePatternListTitle}</div>
              {excludePatternDirs.length === 0 ? (
                <div className="set-card-hint">{t.excludeEmptyHint}</div>
              ) : (
                <div className="rule-list">
                  {excludePatternDirs.map((rule) => (
                    <div key={`pattern-${rule}`} className="rule-item">
                      <span className="rule-tag">{t.excludeRulePattern}</span>
                      <span className="rule-value">{rule}</span>
                      <button
                        className="act-btn"
                        disabled={isExcludeDirSaving}
                        onClick={() => void removeExcludeRule("pattern", rule)}
                      >
                        {t.removeRule}
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {excludeDirStatus && <div className="status-msg">{excludeDirStatus}</div>}
          </div>

          <div className="rule-section">
            <div className="rule-section-title">{t.excludeFilesTitle}</div>
            <p className="set-card-desc">{t.excludeFilesDesc}</p>

            <label className="toggle-card" style={{ marginBottom: 10 }}>
              <div className="toggle-card-copy">
                <div className="toggle-card-title">{t.excludeDotFilesLabel}</div>
                <div className="toggle-card-desc">{t.excludeDotFilesDesc}</div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={excludeDotFiles}
                className={excludeDotFiles ? "ns-switch on" : "ns-switch"}
                disabled={isExcludeFileSaving}
                onClick={() => void toggleExcludeDotFiles(!excludeDotFiles)}
              >
                <span className="ns-switch-knob" />
              </button>
            </label>

            <div className="rule-section-title" style={{ marginTop: 4 }}>{t.excludeFilePatternsTitle}</div>
            <p className="set-card-desc">{t.excludeFilePatternsDesc}</p>
            <div className="form-row">
              <input
                className="form-input"
                value={excludeFilePatternDraft}
                placeholder={t.excludeFilePatternInputPlaceholder}
                disabled={isExcludeFileSaving}
                autoComplete="off"
                autoCorrect="off"
                autoCapitalize="off"
                spellCheck={false}
                onChange={(event) => setExcludeFilePatternDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void addExcludeFilePattern();
                  }
                }}
              />
              <button
                className="act-btn"
                disabled={isExcludeFileSaving || excludeFilePatternDraft.trim().length === 0}
                onClick={() => void addExcludeFilePattern()}
              >
                {t.excludeAdd}
              </button>
            </div>

            {excludeFilePatterns.length > 0 && (
              <div className="rule-list" style={{ marginTop: 8 }}>
                {excludeFilePatterns.map((rule) => (
                  <div key={`file-${rule}`} className="rule-item">
                    <span className="rule-value">{rule}</span>
                    <button
                      className="act-btn"
                      disabled={isExcludeFileSaving}
                      onClick={() => void removeExcludeFilePattern(rule)}
                    >
                      {t.removeRule}
                    </button>
                  </div>
                ))}
              </div>
            )}

            {excludeFileStatus && <div className="status-msg">{excludeFileStatus}</div>}
          </div>
          </div>
        </article>

      </div>
    </div>
  );
}

function fmtStr(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(vars[key] ?? ""));
}
