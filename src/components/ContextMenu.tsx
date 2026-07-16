import type { ContextMenuState, SearchResultItem } from "../types";

export interface ContextMenuProps {
  contextMenu: ContextMenuState;
  selectedPathsInOrder: string[];
  items: SearchResultItem[];
  isPinned: (path: string) => boolean;
  openResult: (path: string) => Promise<void>;
  revealInFinder: (path: string) => Promise<void>;
  openInQSpace: (path: string) => Promise<void>;
  openInTerminal: (path: string) => Promise<void>;
  openInWezTerm: (path: string) => Promise<void>;
  copyText: (text: string) => Promise<void>;
  copySearchResults: (paths: string[]) => Promise<void>;
  moveToTrash: (path: string) => Promise<void>;
  togglePin: (item: SearchResultItem) => void;
  t: Record<string, string>;
  closeContextMenu: () => void;
  openOpenWithMenu: () => void;
  scheduleCloseOpenWithMenu: () => void;
  openWithVisible: boolean;
  setOpenWithVisible: (v: boolean) => void;
  setSelectedItemPaths: (paths: string[]) => void;
  setSelectionAnchorPath: (path: string | null) => void;
}

export function ContextMenu({
  contextMenu,
  selectedPathsInOrder,
  items,
  isPinned,
  openResult,
  revealInFinder,
  openInQSpace,
  openInTerminal,
  openInWezTerm,
  copyText,
  copySearchResults: copySR,
  moveToTrash,
  togglePin,
  t,
  closeContextMenu,
  openOpenWithMenu,
  scheduleCloseOpenWithMenu,
  openWithVisible,
  setSelectedItemPaths,
  setSelectionAnchorPath,
}: ContextMenuProps) {

  const runContextAction = async (action: () => Promise<void>) => {
    try {
      await action();
    } finally {
      closeContextMenu();
    }
  };

  const copyAllSelectedNames = async () => {
    const selected = items.filter((item) => selectedPathsInOrder.includes(item.path));
    await copyText(selected.map((item) => item.name).join("\n"));
  };

  const copyAllSelectedPaths = async () => {
    const selected = items.filter((item) => selectedPathsInOrder.includes(item.path));
    await copyText(selected.map((item) => item.path).join("\n"));
  };

  return (
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
          className="context-menu-item"
          onClick={() => void runContextAction(() => openResult(contextMenu.item.path))}
        >
          {t.menuOpen}
        </button>

        <div
          className="context-submenu-wrap"
          onMouseEnter={openOpenWithMenu}
          onMouseLeave={scheduleCloseOpenWithMenu}
        >
          <button className="context-menu-item">
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
                className="context-menu-item"
                onClick={() => void runContextAction(() => revealInFinder(contextMenu.item.path))}
              >
                {t.menuFinder}
              </button>
              <button
                className="context-menu-item"
                onClick={() => void runContextAction(() => openInQSpace(contextMenu.item.path))}
              >
                {t.menuQSpace}
              </button>
              <button
                className="context-menu-item"
                onClick={() => void runContextAction(() => openInTerminal(contextMenu.item.path))}
              >
                {t.menuTerminal}
              </button>
              <button
                className="context-menu-item"
                onClick={() => void runContextAction(() => openInWezTerm(contextMenu.item.path))}
              >
                {t.menuWezTerm}
              </button>
            </div>
          )}
        </div>

        <div className="context-menu-sep" />

        <button
          className="context-menu-item"
          onClick={() => void runContextAction(() => copyText(contextMenu.item.name))}
        >
          {t.menuCopyName}
        </button>
        <button
          className="context-menu-item"
          onClick={() => void runContextAction(() => copyText(contextMenu.item.path))}
        >
          {t.menuCopyPath}
        </button>
        <button
          className="context-menu-item"
          onClick={() =>
            void runContextAction(() =>
              copySR(contextMenu.multiSelection ? selectedPathsInOrder : [contextMenu.item.path])
            )
          }
        >
          {contextMenu.multiSelection ? t.menuCopyAllResults : t.menuCopyResult}
        </button>
        {contextMenu.multiSelection && (
          <>
            <button
              className="context-menu-item"
              onClick={() => void runContextAction(copyAllSelectedNames)}
            >
              {t.menuCopyAllNames}
            </button>
            <button
              className="context-menu-item"
              onClick={() => void runContextAction(copyAllSelectedPaths)}
            >
              {t.menuCopyAllPaths}
            </button>
          </>
        )}
        <div className="context-menu-sep" />
        <button
          className="context-menu-item"
          onClick={() => void runContextAction(async () => {
            const paths = contextMenu.multiSelection ? selectedPathsInOrder : [contextMenu.item.path];
            for (const p of paths) {
              const target = contextMenu.multiSelection
                ? items.find((it) => it.path === p)
                : contextMenu.item;
              if (target) togglePin(target);
            }
            setSelectedItemPaths([]);
            setSelectionAnchorPath(null);
          })}
        >
          {isPinned(contextMenu.item.path) ? t.menuUnpin : t.menuPin}
        </button>
        <button
          className="context-menu-item danger"
          onClick={() => void runContextAction(async () => {
            const paths = contextMenu.multiSelection ? selectedPathsInOrder : [contextMenu.item.path];
            for (const p of paths) {
              await moveToTrash(p);
            }
          })}
        >
          {t.menuTrash}
        </button>
      </div>
    </div>
  );
}
