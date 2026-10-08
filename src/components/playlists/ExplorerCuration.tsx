import { useEffect, useRef, useState } from "react";
import type { ExplorerFilters, ExplorerSelection, ExplorerView } from "../explorer/DeepExplorer";
import { savePlaylistSelection } from "../../playlists";
import { saveExplorerView, deleteSavedView, type SavedView } from "../../savedViews";
import { sharedRequestFromExplorer, emptySharedRequest } from "../../sharedPlaylistRules";
import { SmartPlaylistEditor } from "./SmartPlaylistEditor";

export type PlaylistDraft = { kind: "static" | "smart"; view: ExplorerView; filters: ExplorerFilters; selection: ExplorerSelection };
export function PlaylistComposer({ draft, onClose, onSaved }: { draft: PlaylistDraft; onClose: () => void; onSaved: (id:number) => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  const [name,setName] = useState("");
  const [busy,setBusy] = useState(false);
  const [error,setError] = useState<string|null>(null);
  useEffect(() => { ref.current?.showModal(); },[]);
  let request = emptySharedRequest(); request.view=draft.view === "albums" ? "albums" : "tracks"; let warning: string | undefined;
  try { request = sharedRequestFromExplorer(draft.view,draft.filters); } catch (reason) { warning=String(reason); }
  const count = draft.selection.kind === "tracks" ? `${draft.selection.tracks.length} selected songs` : `${draft.selection.albums.length} selected albums, in album order`;
  return <dialog ref={ref} className="playlist-composer" aria-label={draft.kind === "smart" ? "Create Smart playlist" : "Create playlist"} onCancel={e => { if(busy) e.preventDefault(); else onClose(); }}>
    {draft.kind === "smart" ? <SmartPlaylistEditor request={request} warning={warning} onSaved={onSaved} onCancel={onClose} onBusyChange={setBusy} /> : <form className="playlist-rule-editor" onSubmit={e => {
      e.preventDefault(); setBusy(true); setError(null);
      void savePlaylistSelection(name,draft.selection.kind === "tracks" ? draft.selection.tracks : [],draft.selection.kind === "albums" ? draft.selection.albums.map(a=>a.id) : []).then(saved=>onSaved(saved.id)).catch((reason:unknown)=>setError(String(reason))).finally(()=>setBusy(false));
    }}><h2>Create playlist</h2><p>{count}. Music Library saves the same playlist for both apps.</p><label>Name<input autoFocus required maxLength={120} disabled={busy} value={name} onChange={e=>setName(e.target.value)} /></label>{error && <p role="alert">{error}</p>}<div className="saved-playlists__actions"><button disabled={busy || !name.trim()} type="submit">{busy?"Saving…":"Save playlist"}</button><button disabled={busy} type="button" onClick={onClose}>Cancel</button></div></form>}
  </dialog>;
}

export function SaveViewControl({ view, filters, views, onChanged }: { view:ExplorerView; filters:ExplorerFilters; views:SavedView[]; onChanged:()=>void }) {
  const [open,setOpen]=useState(false); const [name,setName]=useState(""); const [id,setId]=useState<number|null>(null);
  const [busy,setBusy]=useState(false); const [error,setError]=useState<string|null>(null);
  return <div className="save-view-control"><button type="button" onClick={()=>setOpen(v=>!v)}>Save view</button>{open && <form aria-label="Save explorer view" onSubmit={e=>{
    e.preventDefault(); setBusy(true); setError(null); void saveExplorerView({id,name,view,filters}).then(()=>{onChanged();setOpen(false);}).catch((r:unknown)=>setError(String(r))).finally(()=>setBusy(false));
  }}><label>Saved view<select disabled={busy} value={id??""} onChange={e=>{const v=views.find(v=>v.id===Number(e.target.value));setId(v?.id??null);setName(v?.name??"");}}><option value="">New view</option>{views.map(v=><option key={v.id} value={v.id}>{v.name}</option>)}</select></label><label>View name<input required maxLength={120} value={name} onChange={e=>setName(e.target.value)} /></label><p>Saves the current {view} query, filters, and sort. Pinned in the sidebar.</p>{error&&<p role="alert">{error}</p>}<button disabled={busy||!name.trim()} type="submit">{id?"Update saved view":"Save current view"}</button>{id&&<button disabled={busy} type="button" onClick={()=>{setBusy(true);void deleteSavedView(id).then(()=>{onChanged();setId(null);setName("");}).catch((r:unknown)=>setError(String(r))).finally(()=>setBusy(false));}}>Delete saved view</button>}<button disabled={busy} type="button" onClick={()=>setOpen(false)}>Cancel</button></form>}</div>;
}
