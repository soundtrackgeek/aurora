import { cloneElement, useContext, useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactElement } from "react";
import { ListPlus } from "lucide-react";
import type { Track } from "../../library";
import { authorPlaylist, listMusicLibraryPlaylists, type SavedPlaylistSummary } from "../../playlists";

import { PlaylistAuthoringContext, type PlaylistSelection } from "./playlistAuthoringContext";

/** Keep the existing row DOM and navigation handlers while adding keyboard context access. */
export function PlaylistRow({ selection, children }: { selection: PlaylistSelection; children: ReactElement<{onContextMenu?: (e:MouseEvent<HTMLElement>)=>void; onKeyDown?: (e:KeyboardEvent<HTMLElement>)=>void}> }) {
  const open=useContext(PlaylistAuthoringContext);
  if(!open)return children;
  return cloneElement(children,{
    onContextMenu:e=>{e.preventDefault();e.stopPropagation();open(selection);},
    onKeyDown:e=>{if(e.key==="ContextMenu"||(e.shiftKey&&e.key==="F10")){e.preventDefault();e.stopPropagation();open(selection);}else children.props.onKeyDown?.(e);},
  });
}

export function AddToPlaylistButton({ track, albumId, label, loadTrack }: { track?: Track; albumId?: string; label: string; loadTrack?: ()=>Promise<Track> }) {
  const open = useContext(PlaylistAuthoringContext);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState<string|null>(null);
  if (!open) return null;
  return <><button type="button" disabled={busy} className="add-to-playlist" title="Add to playlist" aria-label={`Add ${label} to playlist`} onDoubleClick={e=>e.stopPropagation()} onKeyDown={e=>e.stopPropagation()} onClick={e=>{e.stopPropagation();if(loadTrack){setBusy(true);setError(null);void loadTrack().then(t=>open({tracks:[t],label})).catch((r:unknown)=>setError(String(r))).finally(()=>setBusy(false));}else open({tracks:track?[track]:undefined,albumIds:albumId?[albumId]:undefined,label});}}><ListPlus aria-hidden="true" /></button>{error&&<span role="alert">{error}</span>}</>;
}

export function PlaylistAuthoringDialog({ selection, onClose, onSaved }: { selection: PlaylistSelection; onClose: ()=>void; onSaved: (id:number)=>void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [playlists,setPlaylists] = useState<SavedPlaylistSummary[]>([]);
  const [target,setTarget] = useState("");
  const [name,setName] = useState("");
  const [busy,setBusy] = useState(false);
  const [loading,setLoading] = useState(true);
  const [error,setError] = useState<string|null>(null);
  useEffect(()=>{dialog.current?.showModal();let cancelled=false;void listMusicLibraryPlaylists().then(items=>{if(!cancelled)setPlaylists(items.filter(p=>p.editable));}).catch((r:unknown)=>{if(!cancelled)setError(String(r));}).finally(()=>{if(!cancelled)setLoading(false);});return()=>{cancelled=true;};},[]);
  return <dialog ref={dialog} className="playlist-composer" aria-label="Add to playlist" onCancel={e=>{if(busy)e.preventDefault();else onClose();}}><form className="playlist-rule-editor" onSubmit={e=>{
    e.preventDefault();if(busy)return;setBusy(true);setError(null);
    const chosen=playlists.find(p=>p.id===Number(target));
    void authorPlaylist({action:chosen?"append":"create",id:chosen?.id,expectedUpdatedAt:chosen?.updatedAt,name:chosen?undefined:name,tracks:selection.tracks,albumIds:selection.albumIds}).then(saved=>onSaved(saved.id)).catch((r:unknown)=>setError(String(r))).finally(()=>setBusy(false));
  }}><h2>Add to playlist</h2><p>{selection.label}. Songs are appended in the selected order, including repeats. Smart playlists and mixtapes use their own rules.</p><fieldset disabled={busy||loading}><label>Playlist<select value={target} onChange={e=>setTarget(e.target.value)}><option value="">New playlist</option>{playlists.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>{!target&&<label>Name<input autoFocus required maxLength={120} value={name} onChange={e=>setName(e.target.value)} /></label>}</fieldset>{loading&&<p role="status">Loading playlists…</p>}{error&&<p role="alert">{error}</p>}<div className="saved-playlists__actions"><button type="submit" disabled={busy||loading||(!target&&!name.trim())}>{busy?"Saving…":target?"Add songs":"Save playlist"}</button><button type="button" disabled={busy} onClick={onClose}>Cancel</button></div></form></dialog>;
}
