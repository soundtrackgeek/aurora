import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Connections {
  networkMode: boolean;
  catalogPath: string;
  syncFolder: string;
  musicRoots: { catalogRoot: string; mountedRoot: string }[];
}

export function ConnectionSettingsPanel() {
  const [draft, setDraft] = useState<Connections | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let cancelled = false;
    void invoke<Connections>("connection_settings").then(value => {
      if (!cancelled) setDraft(value);
    }).catch((e: unknown) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, []);
  async function save() {
    setBusy(true); setError(""); setMessage("");
    try {
      await invoke("save_connection_settings", { settings: draft });
      setMessage("Saved. Quit and reopen Aurora to apply these connections.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  if (!draft) return <p role="status">{error || "Loading connections…"}</p>;
  return <section className="connection-settings">
    <p>Keep your catalog on this computer. Mount music and sync folders in Finder before opening Aurora.</p>
    <label><input type="checkbox" checked={draft.networkMode} onChange={e => setDraft({ ...draft, networkMode: e.target.checked })} /> Network Mode — browse and play; music-file edits are disabled</label>
    <label>Local catalog file<input value={draft.catalogPath} placeholder="Blank uses Music Library’s local database" onChange={e => setDraft({ ...draft, catalogPath: e.target.value })} /></label>
    <label>Sync folder<input value={draft.syncFolder} placeholder="Mounted _musicbackup folder; blank disables sync on Mac" onChange={e => setDraft({ ...draft, syncFolder: e.target.value })} /></label>
    <p>Sync exchanges verified Aurora state and listening-history snapshots. The music catalog is updated separately. An unavailable share leaves local data intact.</p>
    <h3>Music and artwork roots</h3>
    {draft.musicRoots.map((mapping, index) => <div className="connection-settings__mapping" key={index}>
      <label>Catalog root<input aria-label={`Catalog root ${index + 1}`} value={mapping.catalogRoot} onChange={e => setDraft({ ...draft, musicRoots: draft.musicRoots.map((m, i) => i === index ? { ...m, catalogRoot: e.target.value } : m) })} /></label>
      <label>Mounted folder<input aria-label={`Mounted folder ${index + 1}`} value={mapping.mountedRoot} onChange={e => setDraft({ ...draft, musicRoots: draft.musicRoots.map((m, i) => i === index ? { ...m, mountedRoot: e.target.value } : m) })} /></label>
      <button type="button" onClick={() => setDraft({ ...draft, musicRoots: draft.musicRoots.filter((_, i) => i !== index) })}>Remove</button>
    </div>)}
    <button type="button" disabled={draft.musicRoots.length >= 32} onClick={() => setDraft({ ...draft, musicRoots: [...draft.musicRoots, { catalogRoot: "", mountedRoot: "" }] })}>Add root</button>
    {error && <p className="settings-error" role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    <button type="button" className="button button--primary" disabled={busy} onClick={() => void save()}>{busy ? "Saving…" : "Save connections"}</button>
  </section>;
}
