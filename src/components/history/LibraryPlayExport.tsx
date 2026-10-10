import { useEffect, useState } from "react";
import { loadLibraryPlayExportStatus, type LibraryPlayExportStatus } from "../../history";
import { formatCount } from "../../library";

/** Shows whether registered plays are reaching Music Library's listening history. */
export function LibraryPlayExport() {
  const [status, setStatus] = useState<LibraryPlayExportStatus | null>(null);
  useEffect(() => {
    let cancelled = false;
    void loadLibraryPlayExportStatus()
      .then((next) => { if (!cancelled) setStatus(next); })
      .catch(() => { if (!cancelled) setStatus(null); });
    return () => { cancelled = true; };
  }, []);
  if (!status) return null;
  const detail = status.lastError
    ?? (status.lastExportedAtMs === null
      ? "Plays from all devices are sent to Music Library every few minutes on this PC."
      : `${formatCount(status.sentPlays)} plays sent · last sent ${new Date(status.lastExportedAtMs).toLocaleString()}`);
  return (
    <p className={`history-library-export${status.lastError ? " history-library-export--error" : ""}`} role="status">
      <strong>Music Library listening history</strong> <span>{detail}</span>
    </p>
  );
}
