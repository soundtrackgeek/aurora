import { ListMusic, Play, RefreshCw, Shuffle } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { displayTrackArtist, formatCount, formatDuration, type Track } from "../../library";
import { authorPlaylist, exportPlaylist, previewPlaylistImport, savePlaylistImport, loadMusicLibraryPlaylist, refreshSmartPlaylist, type PlaylistImportPreview, type PlaylistEdit, type PlaylistPageContext, type SavedPlaylistDetail, type SavedPlaylistSummary } from "../../playlists";
import { SmartPlaylistEditor } from "./SmartPlaylistEditor";
import { AddToPlaylistButton, PlaylistRow } from "./PlaylistAuthoring";

type Props = {
  active: boolean;
  playlists: SavedPlaylistSummary[];
  selectedId: number | null;
  listLoading: boolean;
  listError: string | null;
  onSelect: (id: number | null) => void;
  onRefresh: () => void;
  onPlay: (tracks: Track[], index: number, shuffle?: boolean, context?: PlaylistPageContext) => Promise<boolean>;
  playbackError?: string | null;
};

export function PlaylistsPage({ active, playlists, selectedId, listLoading, listError, onSelect, onRefresh, onPlay, playbackError }: Props) {
  const [detail, setDetail] = useState<SavedPlaylistDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [queueMessage, setQueueMessage] = useState<string | null>(null);
  const [detailRevision, setDetailRevision] = useState(0);
  const [visibleCount, setVisibleCount] = useState(100);
  const [editor,setEditor] = useState<"new" | "edit" | null>(null);
  const [authorEditor,setAuthorEditor] = useState<{kind:"create"|"rename"|"delete"|"import";id?:number;revision?:string;preview?:PlaylistImportPreview}|null>(null);
  const [name,setName] = useState("");
  const [busy,setBusy] = useState(false);
  const selectedRef = useRef(selectedId);
  const detailGeneration = useRef(0);
  const selectedRevision = playlists.find(item => item.id === selectedId)?.updatedAt;
  useLayoutEffect(() => { selectedRef.current=selectedId; },[selectedId]);

  useEffect(() => {
    ++detailGeneration.current;
    if (!active || selectedId === null) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setLoading(true);
      setError(null);
      setQueueMessage(null);
      void loadMusicLibraryPlaylist(selectedId).then((next) => {
        if (!cancelled) { setDetail(next); setLoading(false); }
      }).catch((reason: unknown) => {
        if (!cancelled) { setDetail(null); setLoading(false); setError(reason instanceof Error ? reason.message : String(reason)); }
      });
    }, 0);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [active, selectedId, selectedRevision, detailRevision]);

  async function startAt(index: number, shuffle = false) {
    if (!detail || detail.id !== selectedId) return;
    setQueueMessage(null);
    try {
      let result: boolean;
      if (detail.revision) {
        const seed = shuffle ? Math.floor(Math.random()*2147483647) : undefined;
        const page = shuffle ? await loadMusicLibraryPlaylist(detail.id,null,detail.revision,seed) : detail;
        if (selectedRef.current !== detail.id) return;
        result = await onPlay(page.tracks,shuffle?0:index,false,{id:detail.id,cursor:page.nextCursor??null,revision:detail.revision,shuffleSeed:seed});
      } else result = await (shuffle ? onPlay(detail.tracks,index,true) : onPlay(detail.tracks,index));
      if (!result) {
      setQueueMessage("Could not start playback. Check the player for details.");
      }
    } catch(reason) { setQueueMessage(String(reason)); }
  }
  async function more() {
    if(detail?.nextCursor == null || loading) return;
    const previous=detail; const generation=detailGeneration.current;setLoading(true); setError(null);
    try {
      const next=await loadMusicLibraryPlaylist(previous.id,previous.nextCursor,previous.revision);
      if(selectedRef.current === previous.id&&generation===detailGeneration.current) { setDetail({...next,tracks:[...previous.tracks,...next.tracks],positions:[...(previous.positions??[]),...(next.positions??[])]});setVisibleCount(v=>v+100); }
    } catch(reason) { if(selectedRef.current===previous.id&&generation===detailGeneration.current) setError(String(reason)); }
    finally { if(selectedRef.current===previous.id&&generation===detailGeneration.current) setLoading(false); }
  }
  async function refreshRules() {
    if(selectedId===null)return; const id=selectedId; setLoading(true);setError(null);
    try { await refreshSmartPlaylist(id);onRefresh();if(selectedRef.current===id)setDetailRevision(v=>v+1); }
    catch(reason){ if(selectedRef.current===id){setError(String(reason));setLoading(false);} }
  }
  function saved(id:number) {
    setEditor(null);onRefresh();onSelect(id);setDetailRevision(v=>v+1);
    }

  async function edit(input: PlaylistEdit) {
    if(busy)return;setBusy(true);setError(null);
    try { const result=await authorPlaylist(input);setAuthorEditor(null);onRefresh();if(selectedRef.current===input.id || input.action==="create"){if(input.action==="create")onSelect(result.id);else if(input.action==="delete"){setDetail(null);setQueueMessage(null);onSelect(playlists.find(item=>item.id!==input.id)?.id??null);}setDetailRevision(v=>v+1);} }
    catch(reason){setError(String(reason));}
    finally{setBusy(false);}
  }
  async function importFile() {
    if(busy)return;setBusy(true);setError(null);
    try {const preview=await previewPlaylistImport();if(preview){setName(preview.name);setAuthorEditor({kind:"import",preview});}}
    catch(reason){setError(String(reason));}finally{setBusy(false);}
  }
  async function saveImport() {
    if(!authorEditor?.preview||busy)return;setBusy(true);setError(null);
    try {const result=await savePlaylistImport(authorEditor.preview,name);setAuthorEditor(null);saved(result.id);}
    catch(reason){setError(String(reason));}finally{setBusy(false);}
  }
  async function exportFile() {
    if(!detail?.revision||busy)return;setBusy(true);setError(null);
    try{if(await exportPlaylist(detail.id,detail.revision))setQueueMessage("M3U8 exported in saved song order.");}
    catch(reason){setError(String(reason));}finally{setBusy(false);}
  }

  const selected = playlists.find((item) => item.id === selectedId);
  return <section className="saved-playlists" aria-label="Music Library playlists">
    <header className="saved-playlists__header">
      <div><span className="eyebrow">Music Library</span><h1>Playlists</h1><p>Your saved playlists, in their original song order.</p></div>
      <div className="saved-playlists__actions"><button type="button" disabled={busy} onClick={()=>{setEditor(null);setName("");setAuthorEditor({kind:"create"});}}>Create playlist</button><button type="button" disabled={busy} onClick={()=>{setAuthorEditor(null);setEditor("new");}}>Create Smart playlist</button><button type="button" disabled={busy} onClick={()=>void importFile()}>Import M3U8</button><button type="button" disabled={busy} onClick={() => { onRefresh(); setDetailRevision((value) => value + 1); }} aria-label="Refresh playlists"><RefreshCw aria-hidden="true" /> Refresh</button></div>
    </header>
    {editor && (editor === "new" || detail?.id===selectedId) && <SmartPlaylistEditor key={editor === "edit" ? selectedId : "new"} id={editor === "edit" ? selectedId??undefined : undefined} revision={editor==="edit"?detail?.revision:undefined} name={editor === "edit" ? selected?.name : undefined} request={editor === "edit" ? detail?.request??undefined : undefined} settings={editor === "edit" ? detail?.smartSettings??undefined : undefined} onSaved={saved} onCancel={()=>setEditor(null)} />}
    {authorEditor && (authorEditor.id == null || authorEditor.id === selectedId) && <form className="playlist-rule-editor" aria-label="Playlist authoring" onSubmit={e=>{e.preventDefault();if(authorEditor.kind==="import")void saveImport();else void edit({action:authorEditor.kind,id:authorEditor.id,expectedUpdatedAt:authorEditor.revision,name});}}>
      <h2>{authorEditor.kind==="delete"?`Delete “${name}”?`:authorEditor.kind==="rename"?"Rename playlist":authorEditor.kind==="import"?"Review M3U8 import":"Create playlist"}</h2>
      {authorEditor.kind==="delete"?<p>Deletes the shared playlist from Aurora and Music Library. Music files are kept.</p>:<label>Name<input autoFocus required maxLength={120} disabled={busy} value={name} onChange={e=>setName(e.target.value)} /></label>}
      {authorEditor.preview&&<><p>{authorEditor.preview.trackCount} songs resolved in the catalog. Repeated songs keep their positions.</p><ol>{authorEditor.preview.samples.map((sample,i)=><li key={i}>{sample}</li>)}</ol></>}
      <div className="saved-playlists__actions"><button type="submit" disabled={busy||!name.trim()}>{busy?"Saving…":authorEditor.kind==="delete"?"Delete shared playlist":"Save playlist"}</button><button type="button" disabled={busy} onClick={()=>setAuthorEditor(null)}>Cancel</button></div>
    </form>}
    {playbackError && <p role="alert">{playbackError}</p>}
    {listError && <p role="alert" className="error-message">{listError}</p>}
    {listLoading && playlists.length === 0 && <p role="status">Loading playlists…</p>}
    {!listLoading && !listError && playlists.length === 0 && <p className="saved-playlists__empty">No saved playlists are available in this Music Library catalog yet.</p>}
    <div className="saved-playlists__layout">
      <nav aria-label="Saved playlists" className="saved-playlists__list">
        {playlists.map((item) => <button type="button" disabled={busy} key={item.id} title={item.name} className={item.id === selectedId ? "is-active" : undefined} aria-current={item.id === selectedId ? "true" : undefined} onClick={() => { setVisibleCount(100);setAuthorEditor(null);setEditor(null); onSelect(item.id); }}>
          <ListMusic aria-hidden="true" /><span><strong>{item.name}</strong><small>{item.smart?"Smart · ":""}{formatCount(item.trackCount)} songs</small></span>
        </button>)}
      </nav>
      <div className="saved-playlists__detail">
        {selected && <header><h2>{selected.name}</h2><p>{detail?.description || selected.description}</p><small>{formatCount(selected.trackCount)} saved songs{detail?.missingCount ? ` · ${formatCount(detail.missingCount)} unavailable in the current catalog` : ""}</small></header>}
        {loading && <p role="status">Loading songs…</p>}
        {error && <p role="alert" className="error-message">{error}</p>}
        {queueMessage && <p role="status" className="error-message">{queueMessage}</p>}
        {!selected && playlists.length > 0 && <p>Choose a playlist to see its songs.</p>}
        {detail && detail.id === selectedId && !loading && <>
          <div className="saved-playlists__actions">
            <button type="button" disabled={busy||!detail.revision} onClick={()=>{setEditor(null);setName(detail.name);setAuthorEditor({kind:"rename",id:detail.id,revision:detail.revision});}}>Rename</button>
            <button type="button" disabled={busy||!detail.revision} onClick={()=>{setEditor(null);setName(detail.name);setAuthorEditor({kind:"delete",id:detail.id,revision:detail.revision});}}>Delete playlist</button>
            <button type="button" disabled={busy||!detail.revision} onClick={()=>void exportFile()}>Export M3U8</button>
            {selected?.smart && <><button type="button" disabled={loading} onClick={()=>void refreshRules()}>Refresh rules</button>{detail.request && <button type="button" onClick={()=>setEditor("edit")}>Edit rules</button>}<small>{detail.smartSettings?.refreshPolicy === "manual" ? "Manual refresh" : "Refreshes when Music Library changes"}</small></>}
            <button type="button" className="saved-playlists__play" onClick={() => void startAt(0)} disabled={detail.tracks.length === 0}><Play aria-hidden="true" /> Play playlist</button>
            <button type="button" className="saved-playlists__play" onClick={() => void startAt(0, true)} disabled={detail.tracks.length === 0}><Shuffle aria-hidden="true" /> Shuffle playlist</button>
          </div>
          <ol className="saved-playlists__tracks">
            {detail.tracks.slice(0, visibleCount).map((track, index) => <li key={`${track.trackKey}-${index}`}>
              <PlaylistRow selection={{tracks: [track], label: track.title}}><button type="button" onClick={() => void startAt(index)} aria-label={`Play ${track.title} from here`}>
                <span className="saved-playlists__number">{(detail.positions?.[index]??index) + 1}</span>
                <span className="saved-playlists__song"><strong>{track.title}</strong><small>{displayTrackArtist(track)} · {track.album}</small></span>
                <span className="saved-playlists__duration">{formatDuration(track.durationSeconds)}</span>
                <Play aria-hidden="true" className="saved-playlists__row-play" />
              </button></PlaylistRow>
              <span className="playlist-song-actions"><AddToPlaylistButton track={track} label={track.title} />{detail.editable&&detail.revision&&detail.positions?.[index]!=null&&<>
                <button type="button" aria-label={`Move ${track.title} up`} disabled={busy||detail.positions[index]===0} onClick={()=>void edit({action:"move",id:detail.id,expectedUpdatedAt:detail.revision,from:detail.positions?.[index],to:(detail.positions?.[index]??0)-1})}>↑</button>
                <button type="button" aria-label={`Move ${track.title} down`} disabled={busy||detail.positions[index]>=detail.trackCount-1} onClick={()=>void edit({action:"move",id:detail.id,expectedUpdatedAt:detail.revision,from:detail.positions?.[index],to:(detail.positions?.[index]??0)+1})}>↓</button>
                <button type="button" aria-label={`Remove ${track.title} from playlist`} disabled={busy} onClick={()=>void edit({action:"remove",id:detail.id,expectedUpdatedAt:detail.revision,from:detail.positions?.[index]})}>×</button>
              </>}</span>
            </li>)}
          </ol>
          {visibleCount < detail.tracks.length && <button type="button" className="saved-playlists__more" onClick={() => setVisibleCount((count) => count + 100)}>Show more songs ({formatCount(Math.min(visibleCount, detail.tracks.length))} of {formatCount(detail.tracks.length)})</button>}
          {detail.nextCursor != null && visibleCount >= detail.tracks.length && <button type="button" disabled={loading} className="saved-playlists__more" onClick={()=>void more()}>Load next 100 songs</button>}
        </>}
      </div>
    </div>
  </section>;
}
