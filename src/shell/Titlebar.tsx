import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { call } from "../utils/ipc";
import { Icon } from "../ui/Icon";

const appWindow = getCurrentWindow();

export function useMaximized(): boolean {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | undefined;

    const sync = () => {
      void appWindow
        .isMaximized()
        .then((value) => {
          if (!disposed) setMaximized(value);
        })
        .catch(() => undefined);
    };

    sync();
    void appWindow.onResized(sync).then((unlisten) => {
      if (disposed) unlisten();
      else dispose = unlisten;
    });

    return () => {
      disposed = true;
      dispose?.();
    };
  }, []);

  return maximized;
}

type TitlebarProps = {
  maximized: boolean;
  /** Items for the "..." menu in the top-left. */
  onOpenSettings?: () => void;
};

export function Titlebar({ maximized, onOpenSettings }: TitlebarProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) setMenuOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  const run = (action: () => void) => () => {
    setMenuOpen(false);
    action();
  };

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar__menu" ref={menuRef}>
        <button
          className="icon-btn"
          type="button"
          aria-label="Menu"
          aria-expanded={menuOpen}
          onClick={() => setMenuOpen((open) => !open)}
        >
          <Icon name="dots" />
        </button>

        {menuOpen ? (
          <div className="menu">
            <button className="menu__item" type="button" onClick={run(() => void call("show_overlay"))}>
              <Icon name="layout" size={16} />
              Quick overlay
            </button>
            {onOpenSettings ? (
              <button className="menu__item" type="button" onClick={run(onOpenSettings)}>
                <Icon name="sliders" size={16} />
                Settings
              </button>
            ) : null}
            <button className="menu__item" type="button" onClick={run(() => void appWindow.hide())}>
              <Icon name="minus" size={16} />
              Hide to tray
            </button>
            <div className="menu__sep" />
            <button
              className="menu__item menu__item--danger"
              type="button"
              onClick={run(() => void call("quit_app"))}
            >
              <Icon name="power" size={16} />
              Quit TrackMLN
            </button>
          </div>
        ) : null}
      </div>

      <div className="titlebar__drag" data-tauri-drag-region />

      <div className="titlebar__controls">
        <button className="win-btn" type="button" aria-label="Minimize" onClick={() => void appWindow.minimize()}>
          <Icon name="minus" size={16} strokeWidth={1.3} />
        </button>
        <button
          className="win-btn"
          type="button"
          aria-label={maximized ? "Restore" : "Maximize"}
          onClick={() => void appWindow.toggleMaximize()}
        >
          <Icon name={maximized ? "restore" : "square"} size={15} strokeWidth={1.3} />
        </button>
        <button className="win-btn win-btn--close" type="button" aria-label="Close" onClick={() => void appWindow.close()}>
          <Icon name="x" size={17} strokeWidth={1.3} />
        </button>
      </div>
    </div>
  );
}
