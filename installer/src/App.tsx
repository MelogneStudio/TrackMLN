import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import installerConfig from "./installer-config.json";
import { Icon } from "./ui/Icon";
const trackmlnLogo = new URL("../../assets/trackmln.svg", import.meta.url).href;

type InstallResult = {
  installDir: string;
  installedExe: string;
  shortcutPath: string;
  startupKey: string;
  selfDeleteScheduled: boolean;
};

export default function App() {
  const appWindow = getCurrentWindow();
  const versionLabel = `v${installerConfig.appVersion}`;
  const [deleteInstaller, setDeleteInstaller] = useState(true);
  const [status, setStatus] = useState<"idle" | "installing" | "done" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<InstallResult | null>(null);
  const installPath = "%APPDATA%\\TrackMLN\\TrackMLN.exe";

  const copyInstallPath = async () => {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(installPath);
        return;
      }
    } catch { /* fall through */ }
    const el = document.createElement("textarea");
    el.value = installPath;
    el.style.cssText = "position:fixed;opacity:0";
    document.body.appendChild(el);
    el.select();
    document.execCommand("copy");
    document.body.removeChild(el);
  };

  const handleWindowAction = async (action: "minimize" | "close") => {
    try {
      if (action === "minimize") { await appWindow.minimize(); return; }
      await appWindow.close();
    } catch (e) {
      setStatus("error");
      setError(`Window ${action} failed. ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const handleDragStart = async () => {
    try { await appWindow.startDragging(); }
    catch (e) {
      setStatus("error");
      setError(`Drag failed. ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const handleInstall = async () => {
    setStatus("installing");
    setError(null);
    try {
      const installResult = await invoke<InstallResult>("install", {
        options: { deleteInstallerAfterFinish: deleteInstaller }
      });
      setResult(installResult);
      setStatus("done");
      if (installResult.selfDeleteScheduled) {
        window.setTimeout(() => void appWindow.close(), 1200);
      }
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <main className="shell">
      <section className="window">
        <header
          className="titlebar"
          data-tauri-drag-region
          onMouseDown={() => void handleDragStart()}
        >
          <div className="titlebar__brand" data-tauri-drag-region>
            <img src={trackmlnLogo} alt="" />
            <strong>TrackMLN</strong>
          </div>
          <div className="titlebar__actions" onMouseDown={(e) => e.stopPropagation()}>
            <button onClick={() => void handleWindowAction("minimize")} type="button" aria-label="Minimize">
              <Icon name="minus" size={16} strokeWidth={1.3} />
            </button>
            <button onClick={() => void handleWindowAction("close")} type="button" aria-label="Close" className="btn-close">
              <Icon name="x" size={16} strokeWidth={1.3} />
            </button>
          </div>
        </header>

        <div className="body">
          <div className="content">
            <div className="install-header">
              <img className="app-mark" src={trackmlnLogo} alt="" />
              <span className="eyebrow">Ready to install</span>
              <h1>TrackMLN Installation</h1>
              <p>Quick local copy, no internet required.</p>
            </div>

            <div className="destination">
              <span className="eyebrow">Install location</span>
              <div className="path-row">
                <textarea
                  aria-label="Install destination path"
                  className="path-field"
                  onFocus={(e) => e.currentTarget.select()}
                  readOnly
                  rows={1}
                  value={installPath}
                />
                <button className="btn-copy" onClick={() => void copyInstallPath()} type="button">Copy</button>
              </div>
            </div>

            <div className="feature-row" aria-label="What the installer will do">
              <div className="feature"><Icon name="check" size={15} /><span>Start Menu shortcut</span></div>
              <div className="feature"><Icon name="check" size={15} /><span>Launches after install</span></div>
              <div className="feature"><Icon name="check" size={15} /><span>Current user startup</span></div>
            </div>

            <label className="option">
              <input
                checked={deleteInstaller}
                onChange={(e) => setDeleteInstaller(e.target.checked)}
                type="checkbox"
              />
              <span>Delete installer after finishing</span>
            </label>
          </div>

          <footer className="footer">
            <div className="footer__meta"><span>{versionLabel}</span><i /> <span>Melogne Studio</span></div>
            <div className="cta-row">
              <button
                className="btn-install"
                disabled={status === "installing"}
                onClick={handleInstall}
                type="button"
              >
                {status === "installing" ? "Installing…" : <><span>Install</span><Icon name="arrowRight" size={17} /></>}
              </button>

              {status === "error" && error && (
                <div className="status-panel status-panel--error">
                  <strong>Installation failed.</strong>
                  <p>{error}</p>
                </div>
              )}

              {status === "done" && result && (
                <div className="status-panel status-panel--success">
                  <strong>Installation complete.</strong>
                  <p>TrackMLN launched from <code>{result.installedExe}</code>.</p>
                  <p>{result.selfDeleteScheduled
                    ? "Installer will close and remove itself."
                    : "Installer kept (self-delete is off)."}</p>
                </div>
              )}
            </div>
          </footer>
        </div>
      </section>
    </main>
  );
}
