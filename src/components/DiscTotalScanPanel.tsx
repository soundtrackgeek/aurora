import { useEffect, useState } from "react";
import { Disc3, ShieldCheck } from "lucide-react";
import { describeDiscTotalScan, loadDiscTotalScanStatus, type DiscTotalScanStatus } from "../discTotals";

const POLL_MS = 5000;

/** Progress of the background Disc Total read behind the `disc:` search. */
export function DiscTotalScanPanel() {
  const [status, setStatus] = useState<DiscTotalScanStatus | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [desktop, setDesktop] = useState(true);
  useEffect(() => {
    let active = true;
    const refresh = () => void loadDiscTotalScanStatus()
      .then((value) => {
        if (!active) return;
        if (value === null) setDesktop(false);
        else { setStatus(value); setFailure(null); }
      })
      .catch((error: unknown) => { if (active) setFailure(error instanceof Error ? error.message : String(error)); });
    refresh();
    const timer = window.setInterval(refresh, POLL_MS);
    return () => { active = false; window.clearInterval(timer); };
  }, []);
  const complete = status !== null && status.catalogFiles > 0 && status.readFiles >= status.catalogFiles;
  const percent = status && status.catalogFiles > 0 ? Math.floor((status.readFiles / status.catalogFiles) * 1000) / 10 : 0;
  const problem = failure ?? status?.error ?? null;
  return <section className="display-settings__section disc-total-scan" aria-labelledby="disc-total-scan-heading">
    <header>
      <span><Disc3 aria-hidden="true" /><span><strong id="disc-total-scan-heading">Disc Total scan</strong><small>Reads Disc Total from your MP3s for the <code>disc:</code> search. Music Library&apos;s catalog does not store it.</small></span></span>
      <span className={`metadata-connection${complete ? " is-connected" : ""}`}>{complete ? <><ShieldCheck aria-hidden="true" /> Complete</> : status ? `${percent}%` : desktop ? "Checking…" : "Desktop app only"}</span>
    </header>
    {status && <>
      <progress className="disc-total-scan__progress" max={Math.max(status.catalogFiles, 1)} value={status.readFiles} aria-label="Disc Total scan progress" />
      <p className="disc-total-scan__summary" role="status">{describeDiscTotalScan(status)}</p>
      <p className="metadata-vault-note">Files that have not been read yet match no <code>disc:</code> term. Files you edit in Aurora are read again within seconds; other changes are picked up every six hours.</p>
    </>}
    {problem && <p className="disc-total-scan__error">{problem}</p>}
  </section>;
}
