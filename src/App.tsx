import {
  Activity,
  Album,
  AudioLines,
  BadgeCheck,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleUserRound,
  Clock3,
  Disc3,
  Download,
  FlaskConical,
  Forward,
  Gauge,
  Heart,
  LibraryBig,
  Menu,
  MoreHorizontal,
  Music2,
  Pause,
  Play,
  RefreshCw,
  Repeat2,
  Search,
  Settings,
  Shuffle,
  Sparkles,
  Star,
  Tags,
  UsersRound,
  Volume2,
  X,
} from "lucide-react";
import { type CSSProperties, type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import {
  filterTracks,
  formatCount,
  formatDuration,
  isTauriRuntime,
  loadArtistTracks,
  loadLibrarySnapshot,
  searchLibraryTracks,
  type Artist,
  type LibrarySnapshot,
  type Track,
} from "./library";
import { useAuroraUpdater } from "./updater";

const navigation = [
  { label: "Universe", icon: Sparkles },
  { label: "Library", icon: LibraryBig },
  { label: "Albums", icon: Album },
  { label: "Artists", icon: UsersRound },
  { label: "Genres", icon: Disc3 },
  { label: "Songs", icon: Music2 },
  { label: "Ratings", icon: Star },
  { label: "Tags", icon: Tags },
  { label: "History", icon: Clock3 },
];

const previewPlaylists = [
  { label: "5 Star Collection", count: "rating view", icon: Star },
  { label: "Night Drive", count: "smart playlist", icon: Music2 },
  { label: "Unplayed", count: "listening queue", icon: Disc3 },
];

function Artwork({ track, size = "small" }: { track: Track; size?: "small" | "large" }) {
  const seed = [...track.artist].reduce((sum, character) => sum + (character.codePointAt(0) ?? 0), 0);
  const words = track.artist.match(/[\p{L}\p{N}]+/gu) ?? ["?"];
  const initials = words.length === 1
    ? words[0].slice(0, 2).toLocaleUpperCase()
    : words.slice(0, 2).map((word) => word[0]).join("").toLocaleUpperCase();
  return (
    <div className={`artwork artwork--${size}`} style={{ "--art-seed": seed } as CSSProperties} aria-hidden="true">
      <span>{initials}</span>
      <AudioLines />
    </div>
  );
}

function Rating({ value }: { value: number | null }) {
  return (
    <span className="rating" aria-label={value === null ? "Unrated" : `${value} out of 5 stars`}>
      {[1, 2, 3, 4, 5].map((star) => (
        <Star key={star} aria-hidden="true" className={value !== null && value >= star ? "is-filled" : undefined} />
      ))}
    </span>
  );
}

function EmptyInspector() {
  return (
    <div className="inspector-empty">
      <Disc3 aria-hidden="true" />
      <h2>Select a track</h2>
      <p>Ratings and file-backed Love editing arrive in the next section.</p>
    </div>
  );
}

function Universe({ artists, activeArtist, onSelect }: { artists: Artist[]; activeArtist: string | null; onSelect: (artist: Artist) => void }) {
  const visibleArtists = artists.slice(0, 8);
  return (
    <section className="universe" aria-labelledby="universe-title">
      <div className="universe__heading">
        <p className="eyebrow">Your listening gravity</p>
        <h1 id="universe-title">Welcome back, Jørn.</h1>
        <p>Choose a world to focus the library.</p>
      </div>
      <div className="universe__quote">
        <p>“Music is the shorthand of emotion.”</p>
        <span>— Leo Tolstoy</span>
      </div>
      <div className="solar-system" aria-label="Top artists in your music universe">
        <span className="sun" aria-hidden="true" />
        {[0, 1, 2, 3].map((orbit) => <span className={`orbit orbit--${orbit + 1}`} key={orbit} aria-hidden="true" />)}
        {visibleArtists.map((artist, index) => (
          <button
            type="button"
            className={`planet planet--${index + 1}${activeArtist === artist.name ? " is-active" : ""}`}
            key={artist.id}
            onClick={() => onSelect(artist)}
            aria-label={`Explore ${artist.name}, ${formatCount(artist.trackCount)} tracks`}
            aria-pressed={activeArtist === artist.name}
          >
            <span className="planet__dot" />
            <span className="planet__label">{artist.name}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function UpdateDialog({ version, phase, progress, message, onInstall, onDismiss }: {
  version: string | null;
  phase: string;
  progress: number | null;
  message: string | null;
  onInstall: () => void;
  onDismiss: () => void;
}) {
  const isWorking = phase === "downloading" || phase === "installing";
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
        <div className="update-dialog__icon"><Download aria-hidden="true" /></div>
        <div>
          <p className="eyebrow">Aurora update</p>
          <h2 id="update-title">Version {version ?? "unknown"} is ready</h2>
          <p>{message || "Install the latest Aurora build now. The app will close, update in place, and restart."}</p>
        </div>
        {isWorking && (
          <div className="update-progress" aria-live="polite">
            <div className="update-progress__track"><span style={{ width: `${progress ?? 12}%` }} /></div>
            <span>{phase === "installing" ? "Installing…" : progress === null ? "Downloading…" : `Downloading ${progress}%`}</span>
          </div>
        )}
        {phase === "error" && <p className="update-error" role="alert">{message}</p>}
        <div className="update-dialog__actions">
          <button type="button" className="button button--quiet" onClick={onDismiss} disabled={isWorking}>Later</button>
          <button type="button" className="button button--primary" onClick={onInstall} disabled={isWorking}>
            {isWorking ? "Updating…" : "Install and restart"}
          </button>
        </div>
      </section>
    </div>
  );
}

function App() {
  const [snapshot, setSnapshot] = useState<LibrarySnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [activeArtist, setActiveArtist] = useState<string | null>(null);
  const [selectedTrack, setSelectedTrack] = useState<Track | null>(null);
  const [activeNav, setActiveNav] = useState("Universe");
  const [reloadToken, setReloadToken] = useState(0);
  const [exploredTracks, setExploredTracks] = useState<Track[] | null>(null);
  const [isExploring, setIsExploring] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const exploreRequestRef = useRef(0);
  const updater = useAuroraUpdater();

  useEffect(() => {
    let cancelled = false;
    void loadLibrarySnapshot()
      .then((nextSnapshot) => {
        if (cancelled) return;
        setSnapshot(nextSnapshot);
        setSelectedTrack(nextSnapshot.tracks[0] ?? null);
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoadError(error instanceof Error ? error.message : String(error));
      });
    return () => { cancelled = true; };
  }, [reloadToken]);

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    }
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  useEffect(() => {
    if (!snapshot) return;
    const requestId = ++exploreRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      const request = query.trim()
        ? searchLibraryTracks(query)
        : activeArtist
          ? loadArtistTracks(activeArtist)
          : Promise.resolve<Track[] | null>(null);
      setIsExploring(Boolean(query.trim() || activeArtist));
      void request
        .then((nextTracks) => {
          if (cancelled || requestId !== exploreRequestRef.current) return;
          setExploredTracks(nextTracks);
          setIsExploring(false);
          if (nextTracks?.length) setSelectedTrack(nextTracks[0]);
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== exploreRequestRef.current) return;
          console.warn("Aurora exploration request failed", error);
          setExploredTracks([]);
          setIsExploring(false);
        });
    }, query.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [snapshot, query, activeArtist]);

  const tracks = useMemo(
    () => exploredTracks ?? filterTracks(snapshot?.tracks ?? [], query, isTauriRuntime() ? null : activeArtist),
    [exploredTracks, snapshot?.tracks, query, activeArtist],
  );

  function selectArtist(artist: Artist) {
    const nextArtist = activeArtist === artist.name ? null : artist.name;
    setActiveArtist(nextArtist);
    if (nextArtist) {
      const firstTrack = snapshot?.tracks.find((track) => track.artist === nextArtist);
      if (firstTrack) setSelectedTrack(firstTrack);
    }
  }

  function submitSearch(event: FormEvent) {
    event.preventDefault();
  }

  const summary = snapshot?.summary;
  const stats = [
    { label: "Songs", value: summary?.songs, detail: "indexed", icon: Music2, tone: "violet" },
    { label: "Albums", value: summary?.albums, detail: "releases", icon: Album, tone: "cyan" },
    { label: "Artists", value: summary?.artists, detail: "worlds", icon: UsersRound, tone: "amber" },
    { label: "Loved", value: summary?.loved, detail: "favorites", icon: Heart, tone: "rose" },
  ];

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand__mark"><AudioLines aria-hidden="true" /></div>
          <div><strong>Aurora</strong><span>your music, your universe</span></div>
        </div>

        <p className="sidebar__label">Navigation</p>
        <nav className="primary-nav" aria-label="Primary">
          {navigation.map(({ label, icon: Icon }) => (
            <button
              type="button"
              key={label}
              className={activeNav === label ? "is-active" : undefined}
              onClick={() => setActiveNav(label)}
            >
              <Icon aria-hidden="true" />
              <span>{label}</span>
              {activeNav === label && <ChevronRight className="nav-chevron" aria-hidden="true" />}
            </button>
          ))}
        </nav>

        <div className="sidebar__section-heading"><p className="sidebar__label">Playlists</p><button type="button" aria-label="Add playlist" disabled>+</button></div>
        <div className="playlists">
          {previewPlaylists.map(({ label, count, icon: Icon }) => (
            <button type="button" key={label} disabled>
              <Icon aria-hidden="true" /><span><strong>{label}</strong><small>{count}</small></span>
            </button>
          ))}
        </div>

        <div className="profile">
          <CircleUserRound aria-hidden="true" />
          <span><strong>Jørn</strong><small>Aurora 0.1.0</small></span>
          <Settings aria-hidden="true" />
        </div>
      </aside>

      <header className="topbar">
        <form className="search" role="search" onSubmit={submitSearch}>
          <Search aria-hidden="true" />
          <input ref={searchRef} value={query} onChange={(event) => { setQuery(event.target.value); if (event.target.value) setActiveArtist(null); }} placeholder="Search your universe…" aria-label="Search your music universe" />
          {query ? <button type="button" aria-label="Clear search" onClick={() => setQuery("")}><X aria-hidden="true" /></button> : <kbd>Ctrl K</kbd>}
        </form>
        <div className="topbar__actions">
          <button type="button" aria-label="Audio tools" disabled><AudioLines aria-hidden="true" /></button>
          <button type="button" aria-label="Labs" disabled><FlaskConical aria-hidden="true" /></button>
          {updater.state.version && <button type="button" className="update-badge" onClick={updater.showPrompt}><Download aria-hidden="true" /> Update {updater.state.version}</button>}
          <button type="button" aria-label="Settings" disabled><Settings aria-hidden="true" /></button>
        </div>
      </header>

      <main className="main-content">
        <div className="main-scroll">
          {snapshot ? (
            <>
              <Universe artists={snapshot.artists} activeArtist={activeArtist} onSelect={selectArtist} />
              <section className="stats" aria-label="Library overview">
                {stats.map(({ label, value, detail, icon: Icon, tone }) => (
                  <article className={`stat stat--${tone}`} key={label}>
                    <div className="stat__icon"><Icon aria-hidden="true" /></div>
                    <div><span>{label}</span><strong>{formatCount(value ?? 0)}</strong><small>{detail}</small></div>
                    <Activity className="stat__spark" aria-hidden="true" />
                  </article>
                ))}
                <article className="source-card">
                  <Gauge aria-hidden="true" />
                  <div><span>Source</span><strong>{snapshot.sourceState === "connected" ? "Live" : "Preview"}</strong><small>{snapshot.sourceLabel}</small></div>
                  {snapshot.sourceState === "connected" && <BadgeCheck aria-label="Connected read-only" />}
                </article>
              </section>

              <section className="library-panel" aria-labelledby="library-heading">
                <div className="library-toolbar">
                  <div><p className="eyebrow">Explore</p><h2 id="library-heading">{activeArtist ?? "All songs"}</h2></div>
                  <div className="toolbar-controls">
                    {activeArtist && <button type="button" className="filter-chip is-active" onClick={() => setActiveArtist(null)}>{activeArtist}<X aria-hidden="true" /></button>}
                    <button type="button" className="filter-chip" disabled>Genre <ChevronDown aria-hidden="true" /></button>
                    <button type="button" className="filter-chip" disabled>Release year <ChevronDown aria-hidden="true" /></button>
                    <button type="button" className="view-toggle is-active" aria-label="Table view"><Menu aria-hidden="true" /></button>
                  </div>
                </div>

                <div className="track-table-wrap">
                  <table className="track-table">
                    <thead><tr><th className="track-index">#</th><th>Title</th><th>Artist</th><th>Album</th><th>Year</th><th>Rating</th><th><Clock3 aria-label="Duration" /></th><th>Genre</th><th><Heart aria-label="Loved" /></th><th /></tr></thead>
                    <tbody>
                      {tracks.map((track, index) => (
                        <tr key={track.id} className={selectedTrack?.id === track.id ? "is-selected" : undefined} onClick={() => setSelectedTrack(track)}>
                          <td className="track-index"><span>{index + 1}</span><Play aria-hidden="true" /></td>
                          <td><div className="track-title"><Artwork track={track} /><strong>{track.title}</strong></div></td>
                          <td>{track.artist}</td><td>{track.album}</td><td>{track.releaseYear ?? "—"}</td>
                          <td><Rating value={track.rating} /></td><td>{formatDuration(track.durationSeconds)}</td><td>{track.genre ?? "—"}</td>
                          <td>{track.loved ? <Heart className="loved" aria-label="Loved" /> : <Heart aria-label="Not loved" />}</td>
                          <td><MoreHorizontal aria-label="More actions" /></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {tracks.length === 0 && <div className="empty-results" aria-live="polite">{isExploring ? <RefreshCw className="is-spinning" aria-hidden="true" /> : <Search aria-hidden="true" />}<h3>{isExploring ? "Searching the catalog…" : "No tracks in this view"}</h3><p>{isExploring ? "Aurora is querying the indexed source." : "Clear the search or artist filter to keep exploring."}</p></div>}
                </div>
              </section>
            </>
          ) : loadError ? (
            <section className="load-state load-state--error" role="alert">
              <Disc3 aria-hidden="true" /><p className="eyebrow">Library unavailable</p><h1>Aurora kept your database untouched.</h1><p>{loadError}</p>
              <button type="button" className="button button--primary" onClick={() => { setLoadError(null); setReloadToken((value) => value + 1); }}><RefreshCw aria-hidden="true" /> Try again</button>
            </section>
          ) : (
            <section className="load-state" aria-live="polite"><div className="loading-orbit"><Disc3 aria-hidden="true" /></div><p>Opening your music universe read-only…</p></section>
          )}
        </div>
      </main>

      <aside className="inspector">
        <div className="inspector-tabs" role="tablist" aria-label="Track details">
          <button type="button" role="tab" aria-selected="true">Track</button>
          <button type="button" role="tab" aria-selected="false" disabled>Album</button>
          <button type="button" role="tab" aria-selected="false" disabled>Artist</button>
          <button type="button" role="tab" aria-selected="false" disabled>Lyrics</button>
        </div>
        {selectedTrack ? (
          <div className="inspector-scroll">
            <Artwork track={selectedTrack} size="large" />
            <div className="track-hero-copy"><h2>{selectedTrack.title}</h2><p>{selectedTrack.artist}</p><span>{selectedTrack.album}</span></div>
            <dl className="metadata-list">
              <div><dt>Rating</dt><dd><Rating value={selectedTrack.rating} /></dd></div>
              <div><dt>Love</dt><dd>{selectedTrack.loved ? <><Heart className="loved" aria-hidden="true" /> Loved</> : "Not loved"}</dd></div>
              <div><dt>Release year</dt><dd>{selectedTrack.releaseYear ?? "Unknown"}</dd></div>
              <div><dt>Genre</dt><dd>{selectedTrack.genre ?? "Unknown"}</dd></div>
              <div><dt>Last.fm plays</dt><dd>{selectedTrack.playCount === null ? "—" : formatCount(selectedTrack.playCount)}</dd></div>
              <div><dt>Duration</dt><dd>{formatDuration(selectedTrack.durationSeconds)}</dd></div>
            </dl>
            <div className="readonly-note"><BadgeCheck aria-hidden="true" /><span><strong>Read-only foundation</strong>File tag editing is deliberately locked in 0.1.0.</span></div>
          </div>
        ) : <EmptyInspector />}
      </aside>

      <footer className="player">
        {selectedTrack ? <div className="now-playing"><Artwork track={selectedTrack} /><div><strong>{selectedTrack.title}</strong><span>{selectedTrack.artist} · {selectedTrack.album}</span></div>{selectedTrack.loved && <Heart className="loved" aria-label="Loved" />}</div> : <div className="now-playing"><Disc3 aria-hidden="true" /><span>No track selected</span></div>}
        <div className="transport" aria-label="Playback controls unavailable in 0.1.0">
          <button type="button" aria-label="Shuffle" disabled><Shuffle aria-hidden="true" /></button>
          <button type="button" aria-label="Previous" disabled><ChevronLeft aria-hidden="true" /></button>
          <button type="button" className="transport__play" aria-label="Play" disabled><Pause aria-hidden="true" /></button>
          <button type="button" aria-label="Next" disabled><ChevronRight aria-hidden="true" /></button>
          <button type="button" aria-label="Repeat" disabled><Repeat2 aria-hidden="true" /></button>
        </div>
        <div className="timeline" aria-hidden="true"><span>0:00</span><div><i /></div><span>{formatDuration(selectedTrack?.durationSeconds ?? null)}</span></div>
        <div className="volume"><Volume2 aria-hidden="true" /><div><span /></div><small>70</small><Forward aria-hidden="true" /></div>
      </footer>

      {updater.state.isPromptOpen && <UpdateDialog version={updater.state.version} phase={updater.state.phase} progress={updater.state.progress} message={updater.state.message} onInstall={() => void updater.install()} onDismiss={updater.dismiss} />}
    </div>
  );
}

export default App;
