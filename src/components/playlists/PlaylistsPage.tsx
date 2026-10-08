import { ListMusic, Play, RefreshCw, Shuffle } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { displayTrackArtist, formatCount, formatDuration, type Track } from "../../library";
import { loadMusicLibraryPlaylist, refreshSmartPlaylist, type PlaylistPageContext, type SavedPlaylistDetail, type SavedPlaylistSummary } from "../../playlists";
import { SmartPlaylistEditor } from "./SmartPlaylistEditor";

type Props = {
  active: boolean;
  playlists: SavedPlaylistSummary[];
  selectedId: number | null;
  listLoading: boolean;
  listError: string | null;
  onSelect: (id: number) => void;
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
  const selectedRef = useRef(selectedId);
  const selectedRevision = playlists.find(item => item.id === selectedId)?.updatedAt;
  useLayoutEffect(() => { selectedRef.current=selectedId; },[selectedId]);

  useEffect(() => {
    if (!active || selectedId === null) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setLoading(true);
      setError(null);
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
    const previous=detail; setLoading(true); setError(null);
    try {
      const next=await loadMusicLibraryPlaylist(previous.id,previous.nextCursor,previous.revision);
      if(selectedRef.current === previous.id) { setDetail({...next,tracks:[...previous.tracks,...next.tracks]});setVisibleCount(v=>v+100); }
    } catch(reason) { if(selectedRef.current===previous.id) setError(String(reason)); }
    finally { if(selectedRef.current===previous.id) setLoading(false); }
  }
  async function refreshRules() {
    if(selectedId===null)return; const id=selectedId; setLoading(true);setError(null);
    try { await refreshSmartPlaylist(id);onRefresh();if(selectedRef.current===id)setDetailRevision(v=>v+1); }
    catch(reason){ if(selectedRef.current===id){setError(String(reason));setLoading(false);} }
  }
  function saved(id:number) {
    setEditor(null);onRefresh();onSelect(id);setDetailRevision(v=>v+1);
    }

  const selected = playlists.find((item) => item.id === selectedId);
  return <section className="saved-playlists" aria-label="Music Library playlists">
    <header className="saved-playlists__header">
      <div><span className="eyebrow">Music Library</span><h1>Playlists</h1><p>Your saved playlists, in their original song order.</p></div>
      <div className="saved-playlists__actions"><button type="button" onClick={()=>setEditor("new")}>Create Smart playlist</button><button type="button" onClick={() => { onRefresh(); setDetailRevision((value) => value + 1); }} aria-label="Refresh playlists"><RefreshCw aria-hidden="true" /> Refresh</button></div>
    </header>
    {editor && (editor === "new" || detail?.id===selectedId) && <SmartPlaylistEditor key={editor === "edit" ? selectedId : "new"} id={editor === "edit" ? selectedId??undefined : undefined} revision={editor==="edit"?detail?.revision:undefined} name={editor === "edit" ? selected?.name : undefined} request={editor === "edit" ? detail?.request??undefined : undefined} settings={editor === "edit" ? detail?.smartSettings??undefined : undefined} onSaved={saved} onCancel={()=>setEditor(null)} />}
    {playbackError && <p role="alert">{playbackError}</p>}
    {listError && <p role="alert" className="error-message">{listError}</p>}
    {listLoading && playlists.length === 0 && <p role="status">Loading playlists…</p>}
    {!listLoading && !listError && playlists.length === 0 && <p className="saved-playlists__empty">No saved playlists are available in this Music Library catalog yet.</p>}
    <div className="saved-playlists__layout">
      <nav aria-label="Saved playlists" className="saved-playlists__list">
        {playlists.map((item) => <button type="button" key={item.id} title={item.name} className={item.id === selectedId ? "is-active" : undefined} aria-current={item.id === selectedId ? "true" : undefined} onClick={() => { setVisibleCount(100); onSelect(item.id); }}>
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
            {selected?.smart && <><button type="button" disabled={loading} onClick={()=>void refreshRules()}>Refresh rules</button>{detail.request && <button type="button" onClick={()=>setEditor("edit")}>Edit rules</button>}<small>{detail.smartSettings?.refreshPolicy === "manual" ? "Manual refresh" : "Refreshes when Music Library changes"}</small></>}
            <button type="button" className="saved-playlists__play" onClick={() => void startAt(0)} disabled={detail.tracks.length === 0}><Play aria-hidden="true" /> Play playlist</button>
            <button type="button" className="saved-playlists__play" onClick={() => void startAt(0, true)} disabled={detail.tracks.length === 0}><Shuffle aria-hidden="true" /> Shuffle playlist</button>
          </div>
          <ol className="saved-playlists__tracks">
            {detail.tracks.slice(0, visibleCount).map((track, index) => <li key={`${track.trackKey}-${index}`}>
              <button type="button" onClick={() => void startAt(index)} aria-label={`Play ${track.title} from here`}>
                <span className="saved-playlists__number">{index + 1}</span>
                <span className="saved-playlists__song"><strong>{track.title}</strong><small>{displayTrackArtist(track)} · {track.album}</small></span>
                <span className="saved-playlists__duration">{formatDuration(track.durationSeconds)}</span>
                <Play aria-hidden="true" className="saved-playlists__row-play" />
              </button>
            </li>)}
          </ol>
          {visibleCount < detail.tracks.length && <button type="button" className="saved-playlists__more" onClick={() => setVisibleCount((count) => count + 100)}>Show more songs ({formatCount(Math.min(visibleCount, detail.tracks.length))} of {formatCount(detail.tracks.length)})</button>}
          {detail.nextCursor != null && visibleCount >= detail.tracks.length && <button type="button" disabled={loading} className="saved-playlists__more" onClick={()=>void more()}>Load next 100 songs</button>}
        </>}
      </div>
    </div>
  </section>;
}
