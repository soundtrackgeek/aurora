import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TonehavnSettingsPanel } from "./TonehavnSettingsPanel";

interface Connections {
  networkMode: boolean;
  catalogPath: string;
  syncFolder: string;
  musicRoots: { catalogRoot: string; mountedRoot: string; smbShare?: string | null }[];
}

interface RootStatus {
  catalogRoot: string;
  mountedRoot: string;
  smbShare: string | null;
  activeRoot: string;
  available: boolean;
}

export function ConnectionSettingsPanel() {
  const [draft, setDraft] = useState<Connections | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [rootStatuses, setRootStatuses] = useState<RootStatus[]>([]);
  useEffect(() => {
    let cancelled = false;
    void invoke<Connections>("connection_settings").then(value => {
      if (!cancelled) setDraft(value);
    }).catch((e: unknown) => { if (!cancelled) setError(String(e)); });
    void invoke<RootStatus[]>("connection_root_statuses").then(value => {
      if (!cancelled) setRootStatuses(value);
    }).catch(() => {});
    return () => { cancelled = true; };
  }, []);
  async function refreshStatuses() {
    setRootStatuses(await invoke<RootStatus[]>("connection_root_statuses"));
  }
  async function save() {
    setBusy(true); setError(""); setMessage("");
    try {
      await invoke("save_connection_settings", { settings: draft });
      await refreshStatuses();
      setMessage("Saved. Quit and reopen Aurora to apply these connections.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function connect() {
    setBusy(true); setError(""); setMessage("");
    try {
      await invoke("save_connection_settings", { settings: draft });
      const opened = await invoke<number>("connect_music_shares");
      setMessage(opened ? `Finder is opening ${opened} music ${opened === 1 ? "share" : "shares"}. Sign in if prompted. Restart Aurora if you changed any mapping.` : "All configured music shares are already mounted. Restart Aurora if you changed any mapping.");
      await new Promise(resolve => setTimeout(resolve, 1500));
      await refreshStatuses();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  if (!draft) return <p role="status">{error || "Loading connections…"}</p>;
  return <section className="connection-settings">
    <p>Keep your catalog on this computer. Aurora opens configured music shares in Finder on launch; mount the sync folder separately.</p>
    <label><input type="checkbox" checked={draft.networkMode} onChange={e => setDraft({ ...draft, networkMode: e.target.checked })} /> Network Mode — play over SMB; rating and Love edits use Tonehavn</label>
    <label>Local catalog file<input value={draft.catalogPath} placeholder="Blank uses Music Library’s local database" onChange={e => setDraft({ ...draft, catalogPath: e.target.value })} /></label>
    <label>Sync folder<input value={draft.syncFolder} placeholder="Mounted _musicbackup folder; blank disables sync on Mac" onChange={e => setDraft({ ...draft, syncFolder: e.target.value })} /></label>
    <p>Sync exchanges verified Aurora state and listening-history snapshots. The music catalog is updated separately. An unavailable share leaves local data intact.</p>
    <h3>Music and artwork roots</h3>
    {draft.musicRoots.map((mapping, index) => <div className="connection-settings__mapping" key={index}>
      <label>Catalog root<input aria-label={`Catalog root ${index + 1}`} value={mapping.catalogRoot} onChange={e => setDraft({ ...draft, musicRoots: draft.musicRoots.map((m, i) => i === index ? { ...m, catalogRoot: e.target.value } : m) })} /></label>
      <label>Mounted folder<input aria-label={`Mounted folder ${index + 1}`} value={mapping.mountedRoot} onChange={e => setDraft({ ...draft, musicRoots: draft.musicRoots.map((m, i) => i === index ? { ...m, mountedRoot: e.target.value } : m) })} /></label>
      <label>SMB share<input aria-label={`SMB share ${index + 1}`} placeholder="smb://host/share" value={mapping.smbShare ?? ""} onChange={e => setDraft({ ...draft, musicRoots: draft.musicRoots.map((m, i) => i === index ? { ...m, smbShare: e.target.value || null } : m) })} /></label>
      <button type="button" onClick={() => setDraft({ ...draft, musicRoots: draft.musicRoots.filter((_, i) => i !== index) })}>Remove</button>
      {rootStatuses[index]?.catalogRoot === mapping.catalogRoot && rootStatuses[index]?.mountedRoot === mapping.mountedRoot && rootStatuses[index]?.smbShare === (mapping.smbShare ?? null) && <small className={`connection-settings__status ${rootStatuses[index].available ? "is-available" : "is-unavailable"}`}>{rootStatuses[index].available ? "Connected" : "Unavailable"} · {rootStatuses[index].activeRoot}</small>}
    </div>)}
    <button type="button" disabled={draft.musicRoots.length >= 32} onClick={() => setDraft({ ...draft, musicRoots: [...draft.musicRoots, { catalogRoot: "", mountedRoot: "", smbShare: null }] })}>Add root</button>
    <p>For Mac SMB roots, enter the share URL alongside its saved mount path. Aurora checks the share identity, so Finder can change the /Volumes suffix safely.</p>
    {draft.networkMode && <div className="connection-settings__actions"><button type="button" disabled={busy || !draft.musicRoots.some(mapping => mapping.smbShare)} onClick={() => void connect()}>Connect music shares</button><button type="button" disabled={busy} onClick={() => void refreshStatuses().catch(e => setError(String(e)))}>Refresh status</button></div>}
    {error && <p className="settings-error" role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    <button type="button" className="button button--primary" disabled={busy} onClick={() => void save()}>{busy ? "Saving…" : "Save connections"}</button>
    <TonehavnSettingsPanel />
  </section>;
}
