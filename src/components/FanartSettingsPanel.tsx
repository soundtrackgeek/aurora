import { useEffect, useState } from "react";
import { Image, ShieldCheck } from "lucide-react";
import { loadFanartSettings, saveFanartCredentials, type FanartSettings } from "../artistPage";

export function FanartSettingsPanel() {
  const [status, setStatus] = useState<FanartSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [personalKey, setPersonalKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => { let active = true; void loadFanartSettings().then((value) => { if (active) setStatus(value); }).catch(() => { if (active) setMessage("Could not read fanart.tv settings."); }); return () => { active = false; }; }, []);
  async function save(clear = false) {
    setBusy(true); setMessage(null);
    try {
      setStatus(await saveFanartCredentials(clear ? { mode: "clear" } : { mode: "save", apiKey: apiKey.trim(), personalKey: personalKey.trim() || null }));
      setApiKey(""); setPersonalKey(""); setMessage(clear ? "Saved fanart.tv credentials removed." : "fanart.tv credentials saved securely. Refresh the artist page to load artwork.");
    } catch (error) { setMessage(error instanceof Error ? error.message : String(error)); }
    finally { setBusy(false); }
  }
  return <section className="display-settings__section" aria-labelledby="fanart-settings-heading">
    <header><span><Image aria-hidden="true" /><span><strong id="fanart-settings-heading">fanart.tv</strong><small>Artist backgrounds and portraits, matched by MusicBrainz ID.</small></span></span><span className={`metadata-connection${status?.configured ? " is-connected" : ""}`}>{status?.configured ? <><ShieldCheck aria-hidden="true" /> {status.personalKeyConfigured ? "Connected · personal key" : "Connected"}</> : "Not connected"}</span></header>
    <div className="metadata-consumer-fields">
      <label className="metadata-token-field"><span>fanart.tv project API key</span><input type="password" autoComplete="off" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={status?.configured ? "Saved securely · enter a replacement" : "Enter your project API key"} /></label>
      <label className="metadata-token-field"><span>Personal / VIP key (optional)</span><input type="password" autoComplete="off" value={personalKey} onChange={(event) => setPersonalKey(event.target.value)} placeholder="Enter your personal key" /></label>
    </div>
    <p className="metadata-vault-note">Keys stay in your operating system credential vault. To replace credentials, enter the project key and the personal key you want to use; leave the personal field empty to remove it.</p>
    <div className="artist-settings-actions"><button type="button" className="button button--primary" disabled={busy || !apiKey.trim()} onClick={() => void save()}>{busy ? "Saving…" : "Save fanart.tv credentials"}</button>{status?.configured && <button type="button" className="button" disabled={busy} onClick={() => void save(true)}>Remove fanart.tv credentials</button>}</div>
    {message && <p role="status">{message}</p>}
  </section>;
}
