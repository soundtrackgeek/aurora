import {
  Album,
  AlertTriangle,
  AudioLines,
  ChevronRight,
  Disc3,
  Heart,
  ListMusic,
  LoaderCircle,
  Music2,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Star,
  UsersRound,
  X,
} from "lucide-react";
import { type CSSProperties, type KeyboardEvent, useMemo, useRef, useState } from "react";
import { albumCoverUrl, formatCount, formatDuration, type Artist, type Track } from "../../library";
import { Artwork } from "../Artwork";
import { InlineLoveControl, InlineRatingControl } from "../InlineTagControls";
import "./DeepExplorer.css";

export type ExplorerView = "tracks" | "albums" | "artists";
export type ExplorerRatingFilter = "all" | "unrated" | 0.5 | 1 | 1.5 | 2 | 2.5 | 3 | 3.5 | 4 | 4.5 | 5;
export type ExplorerLoveFilter = "all" | Track["loveState"];
export type ExplorerSort = "newest" | "titleAsc" | "artistAsc" | "albumAsc" | "releaseYearDesc" | "ratingDesc" | "trackCountDesc";
export type ExplorerLoadState = "loading" | "ready" | "error";

export interface ExplorerFilters {
  query: string;
  rating: ExplorerRatingFilter;
  love: ExplorerLoveFilter;
  yearFrom: number | null;
  yearTo: number | null;
  genre: string | null;
  artist: string | null;
  sort: ExplorerSort;
}

export interface ExplorerAlbum {
  id: string;
  title: string;
  artist: string;
  releaseYear: number | null;
  rating: number | null;
  totalTracks: number;
  durationSeconds: number | null;
  genre: string | null;
  lovedTracks: number;
}

export interface ExplorerPageInfo {
  loaded: number;
  hasMore: boolean;
  isLoadingMore: boolean;
}

export interface DeepExplorerProps {
  view: ExplorerView;
  filters: ExplorerFilters;
  tracks: readonly Track[];
  albums: readonly ExplorerAlbum[];
  artists: readonly Artist[];
  genres: readonly string[];
  artistOptions?: readonly string[];
  selectedTrackId: string | null;
  selectedAlbumId: string | null;
  selectedArtistId: string | null;
  albumTracks: readonly Track[];
  albumTracksTruncated?: boolean;
  loadState: ExplorerLoadState;
  errorMessage?: string | null;
  albumDetailState?: ExplorerLoadState;
  pageInfo: ExplorerPageInfo;
  busyTrackKeys?: ReadonlySet<string>;
  onViewChange: (view: ExplorerView) => void;
  onFiltersChange: (filters: ExplorerFilters) => void;
  onSelectTrack: (track: Track) => void;
  onActivateTrack?: (track: Track) => void;
  onSelectAlbum: (album: ExplorerAlbum | null) => void;
  onSelectArtist: (artist: Artist | null) => void;
  onLoadMore?: () => void;
  onRetry?: () => void;
  onClearFilters?: () => void;
  onRatingChange?: (track: Track, rating: number) => void;
  onLoveChange?: (track: Track, loveState: Track["loveState"]) => void;
}

const EMPTY_BUSY_TRACK_KEYS: ReadonlySet<string> = new Set();

const viewTabs: ReadonlyArray<{ id: ExplorerView; label: string; icon: typeof Music2 }> = [
  { id: "tracks", label: "Tracks", icon: Music2 },
  { id: "albums", label: "Albums", icon: Album },
  { id: "artists", label: "Artists", icon: UsersRound },
];

const sortOptions: Record<ExplorerView, ReadonlyArray<{ value: ExplorerSort; label: string }>> = {
  tracks: [
    { value: "newest", label: "Newest in library" },
    { value: "titleAsc", label: "Title · A–Z" },
    { value: "artistAsc", label: "Artist · A–Z" },
    { value: "albumAsc", label: "Album · A–Z" },
    { value: "releaseYearDesc", label: "Release year · newest" },
    { value: "ratingDesc", label: "Rating · high first" },
  ],
  albums: [
    { value: "releaseYearDesc", label: "Release year · newest" },
    { value: "titleAsc", label: "Album · A–Z" },
    { value: "artistAsc", label: "Artist · A–Z" },
    { value: "ratingDesc", label: "Rating · high first" },
  ],
  artists: [
    { value: "artistAsc", label: "Artist · A–Z" },
    { value: "trackCountDesc", label: "Most tracks" },
  ],
};

const ratingOptions: ReadonlyArray<Exclude<ExplorerRatingFilter, "all" | "unrated">> = [
  5, 4.5, 4, 3.5, 3, 2.5, 2, 1.5, 1, 0.5,
];

function numericInputValue(value: number | null): string {
  return value === null ? "" : String(value);
}

function numericFilterValue(value: string): number | null {
  if (!value) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function ratingFilterValue(value: string): ExplorerRatingFilter {
  if (value === "all" || value === "unrated") return value;
  return Number.parseFloat(value) as Exclude<ExplorerRatingFilter, "all" | "unrated">;
}

function AlbumArtwork({ album, detail = false }: { album: ExplorerAlbum; detail?: boolean }) {
  const source = albumCoverUrl(album.id, detail ? 512 : 256);
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const { initials, seed } = useMemo(() => {
    const nextSeed = [...album.artist].reduce(
      (sum, character) => sum + (character.codePointAt(0) ?? 0),
      0,
    );
    const words = album.artist.match(/[\p{L}\p{N}]+/gu) ?? ["?"];
    const nextInitials = words.length === 1
      ? words[0].slice(0, 2).toLocaleUpperCase()
      : words.slice(0, 2).map((word) => word[0]).join("").toLocaleUpperCase();
    return { initials: nextInitials, seed: nextSeed };
  }, [album.artist]);

  return (
    <span
      className={`deep-explorer-album-artwork${detail ? " is-detail" : ""}`}
      style={{ "--art-seed": seed } as CSSProperties}
      aria-label={`${album.title} cover`}
    >
      {source && source !== failedSource ? (
        <img src={source} alt="" onError={() => setFailedSource(source)} />
      ) : (
        <>
          <strong aria-hidden="true">{initials}</strong>
          <AudioLines aria-hidden="true" />
        </>
      )}
    </span>
  );
}

function StaticRating({ rating }: { rating: number | null }) {
  return (
    <span className="deep-explorer-rating" aria-label={rating === null ? "Unrated" : `${rating} stars`}>
      <Star aria-hidden="true" className={rating !== null ? "is-rated" : undefined} />
      <span>{rating === null ? "—" : rating.toFixed(1)}</span>
    </span>
  );
}

function TrackTable({
  tracks,
  selectedTrackId,
  busyTrackKeys,
  onSelectTrack,
  onActivateTrack,
  onRatingChange,
  onLoveChange,
  compact = false,
}: {
  tracks: readonly Track[];
  selectedTrackId: string | null;
  busyTrackKeys: ReadonlySet<string>;
  onSelectTrack: (track: Track) => void;
  onActivateTrack?: (track: Track) => void;
  onRatingChange?: (track: Track, rating: number) => void;
  onLoveChange?: (track: Track, loveState: Track["loveState"]) => void;
  compact?: boolean;
}) {
  const rowRefs = useRef(new Map<string, HTMLTableRowElement>());
  const selectionIsVisible = tracks.some((track) => track.id === selectedTrackId);

  function handleRowKeyDown(event: KeyboardEvent<HTMLTableRowElement>, track: Track, index: number) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Home" || event.key === "End") {
      event.preventDefault();
      const nextIndex = event.key === "Home"
        ? 0
        : event.key === "End"
          ? tracks.length - 1
          : Math.min(tracks.length - 1, Math.max(0, index + (event.key === "ArrowDown" ? 1 : -1)));
      const nextTrack = tracks[nextIndex];
      if (nextTrack) {
        onSelectTrack(nextTrack);
        rowRefs.current.get(nextTrack.id)?.focus();
      }
      return;
    }
    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      onSelectTrack(track);
      if (event.key === "Enter") onActivateTrack?.(track);
    }
  }

  return (
    <div className={`deep-explorer-table-wrap${compact ? " is-compact" : ""}`}>
      <table className="deep-explorer-table" role="grid" aria-label={compact ? "Album tracks" : "Library tracks"}>
        <thead>
          <tr>
            <th aria-label="Selection" />
            <th>Title</th>
            <th>Artist</th>
            <th>Album</th>
            <th>Year</th>
            <th>Genre</th>
            <th className="is-numeric">Time</th>
            <th className="is-numeric">Plays</th>
            <th>Rating</th>
            <th aria-label="Love" />
          </tr>
        </thead>
        <tbody>
          {tracks.map((track, index) => {
            const selected = track.id === selectedTrackId;
            const busy = busyTrackKeys.has(track.trackKey);
            return (
              <tr
                key={track.trackKey}
                ref={(node) => {
                  if (node) rowRefs.current.set(track.id, node);
                  else rowRefs.current.delete(track.id);
                }}
                className={selected ? "is-selected" : undefined}
                aria-selected={selected}
                tabIndex={selected || (!selectionIsVisible && index === 0) ? 0 : -1}
                onClick={() => onSelectTrack(track)}
                onDoubleClick={() => onActivateTrack?.(track)}
                onKeyDown={(event) => handleRowKeyDown(event, track, index)}
              >
                <td className="deep-explorer-table__signal">
                  <span aria-hidden="true" />
                </td>
                <td>
                  <span className="deep-explorer-track-title">
                    <Artwork track={track} />
                    <span>
                      <strong>{track.title}</strong>
                      {track.tagSyncState ? <small>Pending tag import</small> : null}
                    </span>
                  </span>
                </td>
                <td>{track.artist}</td>
                <td>{track.album}</td>
                <td className="is-numeric">{track.releaseYear ?? "—"}</td>
                <td>{track.genre ?? "—"}</td>
                <td className="is-numeric">{formatDuration(track.durationSeconds)}</td>
                <td className="is-numeric">{track.playCount === null ? "—" : formatCount(track.playCount)}</td>
                <td className="deep-explorer-table__rating">
                  {onRatingChange ? (
                    <InlineRatingControl
                      title={track.title}
                      rating={track.rating}
                      busy={busy}
                      onRatingChange={(rating) => onRatingChange(track, rating)}
                    />
                  ) : (
                    <StaticRating rating={track.rating} />
                  )}
                </td>
                <td className="deep-explorer-table__love">
                  {onLoveChange ? (
                    <InlineLoveControl
                      title={track.title}
                      loveState={track.loveState}
                      busy={busy}
                      onLoveChange={(loveState) => onLoveChange(track, loveState)}
                    />
                  ) : (
                    <Heart className={track.loved ? "is-loved" : undefined} aria-label={track.loved ? "Loved" : "Not loved"} />
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function ExplorerFeedback({
  kind,
  message,
  onRetry,
  onClear,
}: {
  kind: "loading" | "empty" | "error";
  message?: string | null;
  onRetry?: () => void;
  onClear?: () => void;
}) {
  if (kind === "loading") {
    return (
      <div className="deep-explorer-feedback" role="status" aria-live="polite">
        <LoaderCircle className="is-spinning" aria-hidden="true" />
        <strong>Opening the deep catalog…</strong>
        <span>Fetching one bounded page from your library.</span>
      </div>
    );
  }
  if (kind === "error") {
    return (
      <div className="deep-explorer-feedback is-error" role="alert">
        <AlertTriangle aria-hidden="true" />
        <strong>Explorer could not load</strong>
        <span>{message || "The library source did not return this page."}</span>
        {onRetry ? <button type="button" onClick={onRetry}><RefreshCw aria-hidden="true" />Retry</button> : null}
      </div>
    );
  }
  return (
    <div className="deep-explorer-feedback">
      <Disc3 aria-hidden="true" />
      <strong>No matches in this orbit</strong>
      <span>Try widening the year, rating, artist, or Love filters.</span>
      {onClear ? <button type="button" onClick={onClear}><X aria-hidden="true" />Clear filters</button> : null}
    </div>
  );
}

function AlbumGrid({
  albums,
  selectedAlbumId,
  onSelectAlbum,
}: {
  albums: readonly ExplorerAlbum[];
  selectedAlbumId: string | null;
  onSelectAlbum: (album: ExplorerAlbum | null) => void;
}) {
  return (
    <div className="deep-explorer-albums" aria-label="Albums">
      {albums.map((album) => (
        <button
          type="button"
          className={`deep-explorer-album${selectedAlbumId === album.id ? " is-selected" : ""}`}
          aria-pressed={selectedAlbumId === album.id}
          onClick={() => onSelectAlbum(album)}
          key={album.id}
        >
          <AlbumArtwork album={album} />
          <span className="deep-explorer-album__copy">
            <strong>{album.title}</strong>
            <span>{album.artist}</span>
            <small>{album.releaseYear ?? "Year unknown"} · {formatCount(album.totalTracks)} tracks</small>
          </span>
          <ChevronRight aria-hidden="true" />
        </button>
      ))}
    </div>
  );
}

function AlbumDetail({
  album,
  tracks,
  tracksTruncated,
  state,
  selectedTrackId,
  busyTrackKeys,
  onClose,
  onSelectTrack,
  onActivateTrack,
  onRetry,
  onRatingChange,
  onLoveChange,
}: {
  album: ExplorerAlbum;
  tracks: readonly Track[];
  tracksTruncated: boolean;
  state: ExplorerLoadState;
  selectedTrackId: string | null;
  busyTrackKeys: ReadonlySet<string>;
  onClose: () => void;
  onSelectTrack: (track: Track) => void;
  onActivateTrack?: (track: Track) => void;
  onRetry?: () => void;
  onRatingChange?: (track: Track, rating: number) => void;
  onLoveChange?: (track: Track, loveState: Track["loveState"]) => void;
}) {
  return (
    <aside className="deep-explorer-album-detail" aria-label={`${album.title} album details`}>
      <header>
        <AlbumArtwork album={album} detail />
        <div>
          <span className="deep-explorer-kicker">Album detail</span>
          <h3>{album.title}</h3>
          <p>{album.artist}</p>
          <small>
            {album.releaseYear ?? "Year unknown"} · {formatCount(album.totalTracks)} tracks · {formatDuration(album.durationSeconds)}
            {tracksTruncated ? " · first 100 shown" : ""}
          </small>
        </div>
        <button type="button" className="deep-explorer-icon-button" aria-label="Close album details" onClick={onClose}>
          <X aria-hidden="true" />
        </button>
      </header>
      {state === "loading" ? (
        <ExplorerFeedback kind="loading" />
      ) : state === "error" ? (
        <ExplorerFeedback kind="error" onRetry={onRetry} />
      ) : tracks.length === 0 ? (
        <ExplorerFeedback kind="empty" />
      ) : (
        <TrackTable
          tracks={tracks}
          selectedTrackId={selectedTrackId}
          busyTrackKeys={busyTrackKeys}
          onSelectTrack={onSelectTrack}
          onActivateTrack={onActivateTrack}
          onRatingChange={onRatingChange}
          onLoveChange={onLoveChange}
          compact
        />
      )}
    </aside>
  );
}

function ArtistList({
  artists,
  selectedArtistId,
  onSelectArtist,
}: {
  artists: readonly Artist[];
  selectedArtistId: string | null;
  onSelectArtist: (artist: Artist | null) => void;
}) {
  let largestTrackCount = 1;
  for (const artist of artists) largestTrackCount = Math.max(largestTrackCount, artist.trackCount);

  return (
    <ol className="deep-explorer-artists" aria-label="Artists">
      {artists.map((artist, index) => (
        <li key={artist.id}>
          <button
            type="button"
            className={selectedArtistId === artist.id ? "is-selected" : undefined}
            aria-pressed={selectedArtistId === artist.id}
            onClick={() => onSelectArtist(artist)}
          >
            <span className="deep-explorer-artist__rank">{String(index + 1).padStart(2, "0")}</span>
            <span className="deep-explorer-artist__avatar" aria-hidden="true">{artist.name.slice(0, 2).toLocaleUpperCase()}</span>
            <span className="deep-explorer-artist__copy">
              <strong>{artist.name}</strong>
              <small>{formatCount(artist.albumCount)} albums · {formatCount(artist.trackCount)} tracks</small>
              <span aria-hidden="true"><i style={{ width: `${Math.max(4, (artist.trackCount / largestTrackCount) * 100)}%` }} /></span>
            </span>
            <span className="deep-explorer-artist__plays">
              {artist.playCount === null ? "No play history" : `${formatCount(artist.playCount)} plays`}
            </span>
            <ChevronRight aria-hidden="true" />
          </button>
        </li>
      ))}
    </ol>
  );
}

function resultCountForView(view: ExplorerView, props: Pick<DeepExplorerProps, "tracks" | "albums" | "artists">): number {
  if (view === "tracks") return props.tracks.length;
  if (view === "albums") return props.albums.length;
  return props.artists.length;
}

export function DeepExplorer(props: DeepExplorerProps) {
  const {
    view,
    filters,
    tracks,
    albums,
    artists,
    genres,
    selectedTrackId,
    selectedAlbumId,
    selectedArtistId,
    albumTracks,
    albumTracksTruncated = false,
    loadState,
    errorMessage,
    albumDetailState = "ready",
    pageInfo,
    busyTrackKeys = EMPTY_BUSY_TRACK_KEYS,
    onViewChange,
    onFiltersChange,
    onSelectTrack,
    onActivateTrack,
    onSelectAlbum,
    onSelectArtist,
    onLoadMore,
    onRetry,
    onClearFilters,
    onRatingChange,
    onLoveChange,
  } = props;
  const selectedAlbum = albums.find((album) => album.id === selectedAlbumId) ?? null;
  const boundedArtistOptions = useMemo(
    () => props.artistOptions ?? artists.slice(0, 200).map((artist) => artist.name),
    [artists, props.artistOptions],
  );
  const resultCount = resultCountForView(view, props);

  function updateFilters(patch: Partial<ExplorerFilters>) {
    onFiltersChange({ ...filters, ...patch });
  }

  return (
    <section className="deep-explorer" aria-labelledby="deep-explorer-title">
      <header className="deep-explorer__header">
        <div>
          <span className="deep-explorer-kicker">Deep Explorer</span>
          <h2 id="deep-explorer-title">Move through the whole library</h2>
          <p>Dense, bounded pages built for a million-track collection.</p>
        </div>
        <div className="deep-explorer__result" aria-live="polite">
          <strong>{formatCount(pageInfo.loaded)}</strong>
          <span>{view} loaded</span>
        </div>
      </header>

      <div className="deep-explorer-tabs" role="tablist" aria-label="Explorer views">
        {viewTabs.map((tab) => {
          const Icon = tab.icon;
          return (
            <button
              type="button"
              role="tab"
              id={`deep-explorer-tab-${tab.id}`}
              aria-controls={`deep-explorer-panel-${tab.id}`}
              aria-selected={view === tab.id}
              onClick={() => onViewChange(tab.id)}
              key={tab.id}
            >
              <Icon aria-hidden="true" />
              {tab.label}
            </button>
          );
        })}
      </div>

      <div className="deep-explorer-filters" aria-label="Explorer filters">
        <label className="deep-explorer-search">
          <span className="sr-only">Search within explorer</span>
          <Search aria-hidden="true" />
          <input
            type="search"
            value={filters.query}
            placeholder="Search title, album, artist…"
            onChange={(event) => updateFilters({ query: event.currentTarget.value })}
          />
        </label>
        {view === "tracks" ? <>
          <label>
            <span>Rating</span>
            <select value={String(filters.rating)} onChange={(event) => updateFilters({ rating: ratingFilterValue(event.currentTarget.value) })}>
              <option value="all">All ratings</option>
              <option value="unrated">Unrated</option>
              {ratingOptions.map((rating) => <option value={rating} key={rating}>{rating.toFixed(1)} stars</option>)}
            </select>
          </label>
          <label>
            <span>Love</span>
            <select value={filters.love} onChange={(event) => updateFilters({ love: event.currentTarget.value as ExplorerLoveFilter })}>
              <option value="all">Any state</option>
              <option value="loved">Loved</option>
              <option value="neutral">Neutral</option>
              <option value="banned">Banned</option>
            </select>
          </label>
        </> : null}
        {view !== "artists" ? <fieldset className="deep-explorer-year">
            <legend>Release year</legend>
            <input
              type="number"
              inputMode="numeric"
              min="1000"
              max="9999"
              aria-label="Release year from"
              placeholder="From"
              value={numericInputValue(filters.yearFrom)}
              onChange={(event) => updateFilters({ yearFrom: numericFilterValue(event.currentTarget.value) })}
            />
            <span aria-hidden="true">–</span>
            <input
              type="number"
              inputMode="numeric"
              min="1000"
              max="9999"
              aria-label="Release year to"
              placeholder="To"
              value={numericInputValue(filters.yearTo)}
              onChange={(event) => updateFilters({ yearTo: numericFilterValue(event.currentTarget.value) })}
            />
          </fieldset> : null}
        <label>
          <span>Genre</span>
          <select value={filters.genre ?? ""} onChange={(event) => updateFilters({ genre: event.currentTarget.value || null })}>
            <option value="">All genres</option>
            {genres.map((genre) => <option value={genre} key={genre}>{genre}</option>)}
          </select>
        </label>
        {view !== "artists" ? <label>
          <span>Artist</span>
          <select value={filters.artist ?? ""} onChange={(event) => updateFilters({ artist: event.currentTarget.value || null })}>
            <option value="">All artists</option>
            {filters.artist && !boundedArtistOptions.includes(filters.artist) ? <option value={filters.artist}>{filters.artist}</option> : null}
            {boundedArtistOptions.map((artist) => <option value={artist} key={artist}>{artist}</option>)}
          </select>
        </label> : null}
        <label>
          <span>Sort</span>
          <select value={filters.sort} onChange={(event) => updateFilters({ sort: event.currentTarget.value as ExplorerSort })}>
            {sortOptions[view].map((option) => <option value={option.value} key={option.value}>{option.label}</option>)}
          </select>
        </label>
        {onClearFilters ? (
          <button type="button" className="deep-explorer-clear" onClick={onClearFilters}>
            <SlidersHorizontal aria-hidden="true" />Reset
          </button>
        ) : null}
      </div>

      <div
        className={`deep-explorer__body${view === "albums" && selectedAlbum ? " has-album-detail" : ""}`}
        id={`deep-explorer-panel-${view}`}
        role="tabpanel"
        aria-labelledby={`deep-explorer-tab-${view}`}
      >
        {loadState === "loading" ? (
          <ExplorerFeedback kind="loading" />
        ) : loadState === "error" ? (
          <ExplorerFeedback kind="error" message={errorMessage} onRetry={onRetry} />
        ) : resultCount === 0 ? (
          <ExplorerFeedback kind="empty" onClear={onClearFilters} />
        ) : view === "tracks" ? (
          <TrackTable
            tracks={tracks}
            selectedTrackId={selectedTrackId}
            busyTrackKeys={busyTrackKeys}
            onSelectTrack={onSelectTrack}
            onActivateTrack={onActivateTrack}
            onRatingChange={onRatingChange}
            onLoveChange={onLoveChange}
          />
        ) : view === "albums" ? (
          <>
            <AlbumGrid albums={albums} selectedAlbumId={selectedAlbumId} onSelectAlbum={onSelectAlbum} />
            {selectedAlbum ? (
              <AlbumDetail
                album={selectedAlbum}
                tracks={albumTracks}
                tracksTruncated={albumTracksTruncated}
                state={albumDetailState}
                selectedTrackId={selectedTrackId}
                busyTrackKeys={busyTrackKeys}
                onClose={() => onSelectAlbum(null)}
                onSelectTrack={onSelectTrack}
                onActivateTrack={onActivateTrack}
                onRetry={onRetry}
                onRatingChange={onRatingChange}
                onLoveChange={onLoveChange}
              />
            ) : null}
          </>
        ) : (
          <ArtistList artists={artists} selectedArtistId={selectedArtistId} onSelectArtist={onSelectArtist} />
        )}
      </div>

      {loadState === "ready" && resultCount > 0 ? (
        <footer className="deep-explorer-pagination">
          <span>
            Loaded <strong>{formatCount(pageInfo.loaded)}</strong>{pageInfo.hasMore ? " · more available" : ""}
          </span>
          {pageInfo.hasMore && onLoadMore ? (
            <button type="button" disabled={pageInfo.isLoadingMore} onClick={onLoadMore}>
              {pageInfo.isLoadingMore ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <ListMusic aria-hidden="true" />}
              {pageInfo.isLoadingMore ? "Loading next page…" : "Load 50 more"}
            </button>
          ) : (
            <span className="deep-explorer-pagination__end">End of this result set</span>
          )}
        </footer>
      ) : null}
    </section>
  );
}
