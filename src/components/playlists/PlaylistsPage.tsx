import { ListMusic, Play, RefreshCw, Shuffle } from "lucide-react";
import { useEffect, useState } from "react";
import { displayTrackArtist, formatCount, formatDuration, type Track } from "../../library";
import { loadMusicLibraryPlaylist, type SavedPlaylistDetail, type SavedPlaylistSummary } from "../../playlists";

type Props = {
  active: boolean;
  playlists: SavedPlaylistSummary[];
  selectedId: number | null;
  listLoading: boolean;
  listError: string | null;
  onSelect: (id: number) => void;
  onRefresh: () => void;
  onPlay: (tracks: Track[], index: number, shuffle?: boolean) => Promise<boolean>;
};

export function PlaylistsPage({ active, playlists, selectedId, listLoading, listError, onSelect, onRefresh, onPlay }: Props) {
  const [detail, setDetail] = useState<SavedPlaylistDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [queueMessage, setQueueMessage] = useState<string | null>(null);
  const [detailRevision, setDetailRevision] = useState(0);
  const [visibleCount, setVisibleCount] = useState(100);

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
  }, [active, selectedId, detailRevision]);

  async function startAt(index: number, shuffle = false) {
    if (!detail) return;
    setQueueMessage(null);
    if (!await (shuffle ? onPlay(detail.tracks, index, true) : onPlay(detail.tracks, index))) {
      setQueueMessage("Could not start playback. Check the player for details.");
    }
  }

  const selected = playlists.find((item) => item.id === selectedId);
  return <section className="saved-playlists" aria-label="Music Library playlists">
    <header className="saved-playlists__header">
      <div><span className="eyebrow">Music Library</span><h1>Playlists</h1><p>Your saved playlists, in their original song order.</p></div>
      <button type="button" onClick={() => { onRefresh(); setDetailRevision((value) => value + 1); }} aria-label="Refresh playlists"><RefreshCw aria-hidden="true" /> Refresh</button>
    </header>
    {listError && <p role="alert" className="error-message">{listError}</p>}
    {listLoading && playlists.length === 0 && <p role="status">Loading playlists…</p>}
    {!listLoading && !listError && playlists.length === 0 && <p className="saved-playlists__empty">No saved playlists are available in this Music Library catalog yet.</p>}
    <div className="saved-playlists__layout">
      <nav aria-label="Saved playlists" className="saved-playlists__list">
        {playlists.map((item) => <button type="button" key={item.id} title={item.name} className={item.id === selectedId ? "is-active" : undefined} aria-current={item.id === selectedId ? "true" : undefined} onClick={() => { setVisibleCount(100); onSelect(item.id); }}>
          <ListMusic aria-hidden="true" /><span><strong>{item.name}</strong><small>{formatCount(item.trackCount)} songs</small></span>
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
        </>}
      </div>
    </div>
  </section>;
}
