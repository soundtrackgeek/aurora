import { useEffect, useState } from "react";
import { loadJevSettings, saveJevCredentials, testJevConnection, type JevSettings } from "../tonight";
import "./JevSettingsPanel.css";

export function JevSettingsPanel() {
  const [status, setStatus] = useState<JevSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  useEffect(() => { let active = true; void loadJevSettings().then(value => { if (active) setStatus(value); }).catch(() => { if (active) setMessage("Could not read Jev settings."); }); return () => { active = false; }; }, []);
  async function run(action: "save" | "clear" | "test") {
    setBusy(true); setMessage("");
    try {
      if (action === "test") setMessage(await testJevConnection());
      else {
        const next = await saveJevCredentials(action === "save" ? { mode: "save", apiKey: apiKey.trim() } : { mode: "clear" });
        setStatus(next); setApiKey("");
        setMessage(action === "save" ? "OpenRouter key saved securely." : next.source === "environment" ? "Saved key removed. The development environment key is still available." : "Saved OpenRouter key removed.");
      }
    } catch (error) { setMessage(String(error instanceof Error ? error.message : error)); }
    finally { setBusy(false); }
  }
  return <section className="display-settings__section" aria-labelledby="jev-settings-title">
    <header><span><span><strong id="jev-settings-title">Jev &amp; OpenRouter</strong><small>Helps choose Tonight’s Album in Ratings.</small></span></span><span className={`metadata-connection${status?.configured ? " is-connected" : ""}`}>{status?.source === "environment" ? "Development key" : status?.configured ? "Key saved" : "Not configured"}</span></header>
    <label className="metadata-token-field"><span>OpenRouter API key</span><input type="password" autoComplete="off" value={apiKey} disabled={busy || status?.source === "preview"} onChange={event => setApiKey(event.target.value)} placeholder={status?.configured ? "Saved securely · enter a replacement" : "Enter your OpenRouter key"} /></label>
    <p className="metadata-vault-note">Keys stay in your operating system credential vault. Only when requested, Aurora sends a small album shortlist, genre metadata, preference/progress labels and your listening description to OpenRouter and TypeSafe. File paths and listening history stay local. Model: {status?.model ?? "typesafe/jev-1.13"}.</p>
    <div className="artist-settings-actions"><button type="button" className="button button--primary" disabled={busy || !apiKey.trim() || status?.source === "preview"} onClick={() => void run("save")}>Save OpenRouter key</button><button type="button" className="button" disabled={busy || !status?.configured || Boolean(apiKey.trim())} onClick={() => void run("test")}>{busy ? "Working…" : "Test Jev connection"}</button>{status?.source === "vault" && <button type="button" className="button" disabled={busy} onClick={() => void run("clear")}>Remove OpenRouter key</button>}</div>
    {status?.source === "preview" && <p>Configure Jev in the desktop app. This browser preview never stores API keys.</p>}
    {message && <p role="status">{message}</p>}
  </section>;
}
