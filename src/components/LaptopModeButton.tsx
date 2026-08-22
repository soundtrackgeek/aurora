import { Cloud, CloudOff, Laptop, Monitor, RefreshCw, TriangleAlert } from "lucide-react";
import type { LaptopModeStatus } from "../laptopMode";

interface LaptopModeButtonProps {
  status: LaptopModeStatus | null;
  busy: boolean;
  error: string | null;
  onToggle: () => void;
}

function syncIcon(status: LaptopModeStatus | null, busy: boolean) {
  if (busy || status?.syncState === "pending") return <RefreshCw className="is-spinning" aria-hidden="true" />;
  if (status?.syncState === "conflict") return <TriangleAlert aria-hidden="true" />;
  if (status?.syncState === "unavailable") return <CloudOff aria-hidden="true" />;
  return <Cloud aria-hidden="true" />;
}

export function LaptopModeButton({ status, busy, error, onToggle }: LaptopModeButtonProps) {
  const enabled = status?.laptopMode ?? false;
  const syncState = error ? "conflict" : (status?.syncState ?? "pending");
  const message = error ?? status?.settingWarning ?? status?.message ?? "Checking Aurora state sync…";
  const label = enabled ? "Disable Laptop Mode" : "Enable Laptop Mode";

  return (
    <div className={`laptop-mode-control is-${syncState}`}>
      <button
        type="button"
        className={enabled ? "is-active" : undefined}
        aria-label={`${label}. ${message}`}
        aria-describedby="laptop-mode-details"
        aria-pressed={enabled}
        disabled={busy || status === null}
        onClick={onToggle}
      >
        {enabled ? <Laptop aria-hidden="true" /> : <Monitor aria-hidden="true" />}
        <span className="laptop-mode-control__dot" aria-hidden="true" />
      </button>
      <div className="laptop-mode-popover" id="laptop-mode-details" role="tooltip">
        <div className="laptop-mode-popover__heading">
          <span>{syncIcon(status, busy)}</span>
          <div><strong>{status?.modeLabel ?? "Device Mode"}</strong><small>{enabled ? "Catalog paths remapped" : "Catalog paths unchanged"}</small></div>
        </div>
        <p>{message}</p>
        <div className="laptop-mode-mappings">
          {status?.mappings.map((mapping) => (
            <div key={mapping.desktopRoot}>
              <span>{mapping.desktopRoot}</span>
              <strong>→</strong>
              <span className={mapping.available ? "is-available" : "is-unavailable"}>{mapping.activeRoot}</span>
            </div>
          ))}
        </div>
        {status?.remotePath && <small className="laptop-mode-popover__path">State snapshot · {status.remotePath}</small>}
      </div>
    </div>
  );
}
