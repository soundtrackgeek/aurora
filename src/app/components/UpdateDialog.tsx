import { Download } from "lucide-react";

export function UpdateDialog({ version, phase, progress, message, onInstall, onDismiss }: {
  version: string | null;
  phase: string;
  progress: number | null;
  message: string | null;
  onInstall: () => void;
  onDismiss: () => void;
}) {
  const isWorking = phase === "checking" || phase === "downloading" || phase === "installing";
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
        <div className="update-dialog__icon"><Download aria-hidden="true" /></div>
        <div>
          <p className="eyebrow">Aurora update</p>
          <h2 id="update-title">{phase === "checking" ? "Checking for updates…" : phase === "upToDate" ? "Aurora is up to date" : phase === "error" ? "Update couldn’t complete" : `Version ${version ?? "unknown"} is ready`}</h2>
          {phase === "checking" && <p>Looking for a newer Aurora release…</p>}
          {phase === "upToDate" && <p>{message || "You’re running the latest Aurora version."}</p>}
        </div>
        {isWorking && phase !== "checking" && (
          <div className="update-progress" aria-live="polite">
            <div className="update-progress__track"><span style={{ width: `${progress ?? 12}%` }} /></div>
            <span>{phase === "installing" ? "Installing…" : progress === null ? "Downloading…" : `Downloading ${progress}%`}</span>
          </div>
        )}
        {phase === "error" && <p className="update-error" role="alert">{message}</p>}
        <div className="update-dialog__actions">
          <button type="button" className="button button--quiet" onClick={onDismiss} disabled={isWorking}>{version ? "Later" : "Close"}</button>
          {version && <button type="button" className="button button--primary" onClick={onInstall} disabled={isWorking}>
            {isWorking ? "Updating…" : "Install and restart"}
          </button>}
        </div>
      </section>
    </div>
  );
}
