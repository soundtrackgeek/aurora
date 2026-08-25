import {
  Album,
  AlertTriangle,
  AudioLines,
  ChevronDown,
  ChevronRight,
  Disc3,
  Gauge,
  Heart,
  ListMusic,
  LoaderCircle,
  Music2,
  RefreshCw,
  SlidersHorizontal,
  Star,
  Trash2,
  UsersRound,
  X,
} from "lucide-react";
import {
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { albumCoverUrl, formatCount, formatDuration, type Artist, type Track, type YearBasis } from "../../library";
import { Artwork } from "../Artwork";
import { InlineLoveControl, InlineRatingControl } from "../InlineTagControls";
import "./DeepExplorer.css";

export type ExplorerView = "tracks" | "albums" | "artists";
export type ExplorerRatingFilter = "all" | "unrated" | 0.5 | 1 | 1.5 | 2 | 2.5 | 3 | 3.5 | 4 | 4.5 | 5;
export type ExplorerLoveFilter = "all" | Track["loveState"];
export type ExplorerSort =
  | "newest"
  | "oldest"
  | "titleAsc"
  | "titleDesc"
  | "artistAsc"
  | "artistDesc"
  | "albumAsc"
  | "albumDesc"
  | "yearAsc"
  | "yearDesc"
  | "releaseYearAsc"
  | "releaseYearDesc"
  | "ratingAsc"
  | "ratingDesc"
  | "trackCountAsc"
  | "trackCountDesc";
export type ExplorerLoadState = "loading" | "ready" | "error";

export interface ExplorerFilters {
  query: string;
  rating: ExplorerRatingFilter;
  love: ExplorerLoveFilter;
  yearFrom: number | null;
  yearTo: number | null;
  yearBasis: YearBasis;
  yearMissing: boolean;
  genre: string | null;
  artist: string | null;
  sort: ExplorerSort;
}

export interface ExplorerAlbum {
  id: string;
  title: string;
  artist: string;
  originalYear?: number | null;
  releaseYear: number | null;
  publisher?: string | null;
  rating: number | null;
  totalTracks: number;
  durationSeconds: number | null;
  genre: string | null;
  lovedTracks: number;
  ratedTracks: number;
  albumScore: number | null;
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
  selectedTrackId: string | null;
  currentTrackKey?: string | null;
  playbackActive?: boolean;
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
  onDeleteTracks?: (tracks: readonly Track[]) => Promise<void>;
}

const EMPTY_BUSY_TRACK_KEYS: ReadonlySet<string> = new Set();

const viewTabs: ReadonlyArray<{ id: ExplorerView; label: string; icon: typeof Music2 }> = [
  { id: "tracks", label: "Tracks", icon: Music2 },
  { id: "albums", label: "Albums", icon: Album },
  { id: "artists", label: "Artists", icon: UsersRound },
];

type ExplorerSortCriterion = "added" | "title" | "artist" | "album" | "year" | "releaseYear" | "rating" | "trackCount";

interface ExplorerSortOption {
  value: ExplorerSortCriterion;
  primary: ExplorerSort;
  primaryLabel: string;
  reverse: ExplorerSort;
  reverseLabel: string;
}

const sortOptions: Record<ExplorerView, readonly ExplorerSortOption[]> = {
  tracks: [
    { value: "added", primary: "newest", primaryLabel: "Added · newest", reverse: "oldest", reverseLabel: "Added · oldest" },
    { value: "title", primary: "titleAsc", primaryLabel: "Title · A–Z", reverse: "titleDesc", reverseLabel: "Title · Z–A" },
    { value: "artist", primary: "artistAsc", primaryLabel: "Artist · A–Z", reverse: "artistDesc", reverseLabel: "Artist · Z–A" },
    { value: "album", primary: "albumAsc", primaryLabel: "Album · A–Z", reverse: "albumDesc", reverseLabel: "Album · Z–A" },
    { value: "year", primary: "yearDesc", primaryLabel: "Year · newest", reverse: "yearAsc", reverseLabel: "Year · oldest" },
    { value: "releaseYear", primary: "releaseYearDesc", primaryLabel: "Release year · newest", reverse: "releaseYearAsc", reverseLabel: "Release year · oldest" },
    { value: "rating", primary: "ratingDesc", primaryLabel: "Rating · high first", reverse: "ratingAsc", reverseLabel: "Rating · low first" },
  ],
  albums: [
    { value: "year", primary: "yearDesc", primaryLabel: "Year · newest", reverse: "yearAsc", reverseLabel: "Year · oldest" },
    { value: "releaseYear", primary: "releaseYearDesc", primaryLabel: "Release year · newest", reverse: "releaseYearAsc", reverseLabel: "Release year · oldest" },
    { value: "title", primary: "titleAsc", primaryLabel: "Album · A–Z", reverse: "titleDesc", reverseLabel: "Album · Z–A" },
    { value: "artist", primary: "artistAsc", primaryLabel: "Artist · A–Z", reverse: "artistDesc", reverseLabel: "Artist · Z–A" },
    { value: "rating", primary: "ratingDesc", primaryLabel: "Rating · high first", reverse: "ratingAsc", reverseLabel: "Rating · low first" },
  ],
  artists: [
    { value: "artist", primary: "artistAsc", primaryLabel: "Artist · A–Z", reverse: "artistDesc", reverseLabel: "Artist · Z–A" },
    { value: "trackCount", primary: "trackCountDesc", primaryLabel: "Most tracks", reverse: "trackCountAsc", reverseLabel: "Fewest tracks" },
  ],
};

function activeSortOption(view: ExplorerView, sort: ExplorerSort): ExplorerSortOption {
  return sortOptions[view].find((option) => option.primary === sort || option.reverse === sort)
    ?? sortOptions[view][0];
}

function nextSort(view: ExplorerView, current: ExplorerSort, criterion: ExplorerSortCriterion): ExplorerSort {
  const option = sortOptions[view].find((candidate) => candidate.value === criterion)
    ?? sortOptions[view][0];
  if (current === option.primary) return option.reverse;
  if (current === option.reverse) return option.primary;
  return option.primary;
}

interface SortControlProps {
  view: ExplorerView;
  current: ExplorerSort;
  onChange: (sort: ExplorerSort) => void;
}

function SortControl({ view, current, onChange }: SortControlProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const selectedOption = activeSortOption(view, current);
  const selectedLabel = current === selectedOption.reverse
    ? selectedOption.reverseLabel
    : selectedOption.primaryLabel;

  useEffect(() => {
    if (!open) return undefined;

    rootRef.current
      ?.querySelector<HTMLButtonElement>('[role="menuitemradio"][aria-checked="true"]')
      ?.focus();

    function closeOnOutsidePointer(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    }

    function closeOnEscape(event: globalThis.KeyboardEvent) {
      if (event.key !== "Escape") return;
      setOpen(false);
      triggerRef.current?.focus();
    }

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  function moveMenuFocus(event: KeyboardEvent<HTMLDivElement>) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const items = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')];
    if (items.length === 0) return;

    event.preventDefault();
    const currentIndex = items.findIndex((item) => item === document.activeElement);
    if (event.key === "Home") {
      items[0].focus();
      return;
    }
    if (event.key === "End") {
      items[items.length - 1].focus();
      return;
    }
    const offset = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex = currentIndex < 0
      ? (offset > 0 ? 0 : items.length - 1)
      : (currentIndex + offset + items.length) % items.length;
    items[nextIndex].focus();
  }

  function chooseSort(criterion: ExplorerSortCriterion) {
    onChange(nextSort(view, current, criterion));
    setOpen(false);
    triggerRef.current?.focus();
  }

  return (
    <div className="deep-explorer-sort-control" ref={rootRef}>
      <span>Sort</span>
      <button
        type="button"
        className="deep-explorer-sort-trigger"
        ref={triggerRef}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls="deep-explorer-sort-menu"
        aria-label={`Sort: ${selectedLabel}`}
        onClick={() => setOpen((value) => !value)}
      >
        <span>{selectedLabel}</span>
        <ChevronDown aria-hidden="true" />
      </button>
      {open ? (
        <div
          className="deep-explorer-sort-menu"
          id="deep-explorer-sort-menu"
          role="menu"
          aria-label="Sort options"
          onKeyDown={moveMenuFocus}
        >
          {sortOptions[view].map((option) => {
            const isActive = option.primary === current || option.reverse === current;
            const label = isActive && current === option.reverse
              ? option.reverseLabel
              : option.primaryLabel;
            return (
              <button
                type="button"
                role="menuitemradio"
                aria-checked={isActive}
                onClick={() => chooseSort(option.value)}
                key={option.value}
              >
                {label}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
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

function AlbumRatingStars({ rating }: { rating: number | null }) {
  const visualRating = rating === null ? 0 : Math.round(rating * 2) / 2;
  return (
    <span
      className={`deep-explorer-album-rating${rating === null ? " is-unrated" : ""}`}
      aria-label={rating === null ? "Album unrated" : `Album rating ${rating.toFixed(1)} out of 5 stars`}
    >
      <span className="deep-explorer-album-rating__stars" aria-hidden="true">
        {[1, 2, 3, 4, 5].map((star) => {
          const fill = visualRating >= star ? "is-full" : visualRating === star - 0.5 ? "is-half" : "";
          return (
            <span className={`deep-explorer-album-rating__star ${fill}`} key={star}>
              <Star className="deep-explorer-album-rating__empty" />
              {fill ? <Star className="deep-explorer-album-rating__fill" /> : null}
            </span>
          );
        })}
      </span>
      <span>{rating === null ? "—" : rating.toFixed(1)}</span>
    </span>
  );
}

function TrackTable({
  tracks,
  selectedTrackId,
  currentTrackKey,
  playbackActive,
  busyTrackKeys,
  onSelectTrack,
  onActivateTrack,
  onRatingChange,
  onLoveChange,
  onDeleteTrack,
  multiSelectedTrackKeys,
  onSelectionGesture,
  compact = false,
}: {
  tracks: readonly Track[];
  selectedTrackId: string | null;
  currentTrackKey?: string | null;
  playbackActive?: boolean;
  busyTrackKeys: ReadonlySet<string>;
  onSelectTrack: (track: Track) => void;
  onActivateTrack?: (track: Track) => void;
  onRatingChange?: (track: Track, rating: number) => void;
  onLoveChange?: (track: Track, loveState: Track["loveState"]) => void;
  onDeleteTrack?: (track: Track) => void;
  multiSelectedTrackKeys?: ReadonlySet<string>;
  onSelectionGesture?: (track: Track, index: number, modifiers: { ctrl: boolean; shift: boolean }) => void;
  compact?: boolean;
}) {
  const rowRefs = useRef(new Map<string, HTMLTableRowElement>());
  const selectionIsVisible = tracks.some((track) => multiSelectedTrackKeys?.has(track.trackKey) || track.id === selectedTrackId);

  function selectTrack(track: Track, index: number, event?: Pick<MouseEvent | KeyboardEvent, "ctrlKey" | "metaKey" | "shiftKey">) {
    onSelectTrack(track);
    onSelectionGesture?.(track, index, {
      ctrl: Boolean(event?.ctrlKey || event?.metaKey),
      shift: Boolean(event?.shiftKey),
    });
  }

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
        selectTrack(nextTrack, nextIndex, event);
        rowRefs.current.get(nextTrack.id)?.focus();
      }
      return;
    }
    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      selectTrack(track, index, event);
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
            <th>Publisher</th>
            <th className="is-numeric">Time</th>
            <th className="is-numeric">Plays</th>
            <th>Rating</th>
            <th aria-label="Love" />
            {onDeleteTrack ? <th aria-label="Delete" /> : null}
          </tr>
        </thead>
        <tbody>
          {tracks.map((track, index) => {
            const selected = multiSelectedTrackKeys && multiSelectedTrackKeys.size > 0
              ? multiSelectedTrackKeys.has(track.trackKey)
              : track.id === selectedTrackId;
            const current = track.trackKey === currentTrackKey;
            const busy = busyTrackKeys.has(track.trackKey);
            return (
              <tr
                key={track.trackKey}
                ref={(node) => {
                  if (node) rowRefs.current.set(track.id, node);
                  else rowRefs.current.delete(track.id);
                }}
                className={`${selected ? "is-selected" : ""}${current ? " is-current-track" : ""}${current && playbackActive ? " is-playing" : ""}`.trim() || undefined}
                aria-selected={selected}
                aria-current={current ? "true" : undefined}
                tabIndex={selected || (!selectionIsVisible && index === 0) ? 0 : -1}
                onClick={(event) => selectTrack(track, index, event)}
                onDoubleClick={() => onActivateTrack?.(track)}
                onKeyDown={(event) => handleRowKeyDown(event, track, index)}
              >
                <td className="deep-explorer-table__signal">
                  <span aria-hidden="true" />
                  {current ? <span className="sr-only">{playbackActive ? "Currently playing" : "Current playback track"}</span> : null}
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
                <td className="is-numeric">{track.originalYear ?? "—"}</td>
                <td>{track.genre ?? "—"}</td>
                <td>{track.publisher ?? "—"}</td>
                <td className="is-numeric">{formatDuration(track.durationSeconds)}</td>
                <td className="is-numeric">{track.playCount === null ? "—" : formatCount(track.playCount)}</td>
                <td className="deep-explorer-table__rating">
                  {onRatingChange ? (
                    <InlineRatingControl
                      title={track.title}
                      rating={track.rating}
                      busy={busy}
                      onRatingChange={(rating) => {
                        if (rating !== null) onRatingChange(track, rating);
                      }}
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
                {onDeleteTrack ? (
                  <td className="deep-explorer-table__delete">
                    <button
                      type="button"
                      aria-label={`Delete ${track.title}`}
                      disabled={busy}
                      onClick={(event) => {
                        event.stopPropagation();
                        onDeleteTrack(track);
                      }}
                    >
                      <Trash2 aria-hidden="true" />
                    </button>
                  </td>
                ) : null}
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
  detailAlbumId,
  detail,
}: {
  albums: readonly ExplorerAlbum[];
  selectedAlbumId: string | null;
  onSelectAlbum: (album: ExplorerAlbum | null) => void;
  detailAlbumId: string | null;
  detail: ReactNode;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [columnCount, setColumnCount] = useState(1);

  const updateColumnCount = useCallback(() => {
    const root = rootRef.current;
    if (!root) return;
    const styles = getComputedStyle(root);
    const horizontalPadding = (Number.parseFloat(styles.paddingLeft) || 0) + (Number.parseFloat(styles.paddingRight) || 0);
    const gap = Number.parseFloat(styles.columnGap) || 1;
    const minimum = Number.parseFloat(styles.getPropertyValue("--aurora-album-grid-min")) || 145;
    const available = Math.max(0, root.clientWidth - horizontalPadding);
    const next = Math.max(1, Math.floor((available + gap) / (minimum + gap)));
    setColumnCount((current) => current === next ? current : next);
  }, []);

  useLayoutEffect(() => updateColumnCount());
  useEffect(() => {
    if (!rootRef.current || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(updateColumnCount);
    observer.observe(rootRef.current);
    return () => observer.disconnect();
  }, [updateColumnCount]);

  const rows = useMemo(() => {
    const next: ExplorerAlbum[][] = [];
    for (let index = 0; index < albums.length; index += columnCount) {
      next.push(albums.slice(index, index + columnCount));
    }
    return next;
  }, [albums, columnCount]);

  return (
    <div className="deep-explorer-albums" aria-label="Albums" ref={rootRef}>
      {rows.map((row) => (
        <div
          className="deep-explorer-album-row"
          style={{ "--album-columns": columnCount } as CSSProperties}
          key={row[0].id}
        >
          {row.map((album) => {
            const selected = selectedAlbumId === album.id;
            return (
              <button
                type="button"
                className={`deep-explorer-album${selected ? " is-selected" : ""}`}
                aria-pressed={selected}
                aria-expanded={selected}
                onClick={() => onSelectAlbum(selected ? null : album)}
                key={album.id}
              >
                <AlbumArtwork album={album} />
                <span className="deep-explorer-album__copy">
                  <strong>{album.title}</strong>
                  <span>{album.artist}</span>
                  <small>{album.originalYear ?? "Year unknown"} · {album.publisher ?? "Publisher unknown"} · {formatCount(album.totalTracks)} tracks</small>
                  <span className="deep-explorer-album__metrics">
                    <AlbumRatingStars rating={album.rating} />
                    <span aria-hidden="true">·</span>
                    <span aria-label={album.albumScore === null ? "Album Score unavailable" : `Album Score ${album.albumScore.toFixed(1)}`}>
                      Score {album.albumScore === null ? "—" : album.albumScore.toFixed(1)}
                    </span>
                  </span>
                </span>
                <ChevronRight aria-hidden="true" />
              </button>
            );
          })}
          {row.some((album) => album.id === detailAlbumId) ? detail : null}
        </div>
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
  currentTrackKey,
  playbackActive,
  closing = false,
  busyTrackKeys,
  onClose,
  onSelectTrack,
  onActivateTrack,
  onRetry,
  onRatingChange,
  onLoveChange,
  onDeleteTracks,
}: {
  album: ExplorerAlbum;
  tracks: readonly Track[];
  tracksTruncated: boolean;
  state: ExplorerLoadState;
  selectedTrackId: string | null;
  currentTrackKey?: string | null;
  playbackActive?: boolean;
  closing?: boolean;
  busyTrackKeys: ReadonlySet<string>;
  onClose: () => void;
  onSelectTrack: (track: Track) => void;
  onActivateTrack?: (track: Track) => void;
  onRetry?: () => void;
  onRatingChange?: (track: Track, rating: number) => void;
  onLoveChange?: (track: Track, loveState: Track["loveState"]) => void;
  onDeleteTracks?: (tracks: readonly Track[]) => Promise<void>;
}) {
  const [selectedTrackKeys, setSelectedTrackKeys] = useState<ReadonlySet<string>>(() => new Set(
    tracks.filter((track) => track.id === selectedTrackId).map((track) => track.trackKey),
  ));
  const [selectionAnchorKey, setSelectionAnchorKey] = useState<string | null>(null);
  const [deleteTargets, setDeleteTargets] = useState<readonly Track[]>([]);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const visibleSelectedTrackKeys = useMemo(() => {
    const visibleKeys = new Set(tracks.map((track) => track.trackKey));
    return new Set([...selectedTrackKeys].filter((key) => visibleKeys.has(key)));
  }, [selectedTrackKeys, tracks]);

  function selectWithModifiers(track: Track, index: number, modifiers: { ctrl: boolean; shift: boolean }) {
    if (modifiers.shift && selectionAnchorKey) {
      const anchorIndex = tracks.findIndex((candidate) => candidate.trackKey === selectionAnchorKey);
      if (anchorIndex >= 0) {
        const [start, end] = anchorIndex < index ? [anchorIndex, index] : [index, anchorIndex];
        const range = tracks.slice(start, end + 1).map((candidate) => candidate.trackKey);
        setSelectedTrackKeys(modifiers.ctrl
          ? (current) => new Set([...current, ...range])
          : new Set(range));
        return;
      }
    }
    if (modifiers.ctrl) {
      setSelectedTrackKeys((current) => {
        const next = new Set(current);
        if (next.has(track.trackKey)) next.delete(track.trackKey);
        else next.add(track.trackKey);
        return next;
      });
    } else {
      setSelectedTrackKeys(new Set([track.trackKey]));
    }
    setSelectionAnchorKey(track.trackKey);
  }

  function requestDelete(track?: Track) {
    if (!onDeleteTracks) return;
    if (track && !visibleSelectedTrackKeys.has(track.trackKey)) {
      setDeleteTargets([track]);
      return;
    }
    const selected = tracks.filter((candidate) => visibleSelectedTrackKeys.has(candidate.trackKey));
    if (selected.length > 0) setDeleteTargets(selected);
  }

  async function confirmDelete() {
    if (deleteTargets.length === 0 || !onDeleteTracks || deleteBusy) return;
    setDeleteBusy(true);
    setDeleteError(null);
    try {
      await onDeleteTracks(deleteTargets);
      const deletedKeys = new Set(deleteTargets.map((track) => track.trackKey));
      setSelectedTrackKeys((current) => new Set([...current].filter((key) => !deletedKeys.has(key))));
      setDeleteTargets([]);
    } catch (error) {
      setDeleteError(error instanceof Error ? error.message : String(error));
    } finally {
      setDeleteBusy(false);
    }
  }

  return (
    <aside className={`deep-explorer-album-detail${closing ? " is-closing" : ""}`} aria-label={`${album.title} album details`}>
      <header>
        <AlbumArtwork album={album} detail />
        <div>
          <span className="deep-explorer-kicker">Album detail</span>
          <h3>{album.title}</h3>
          <p>{album.artist}</p>
          <span className="deep-explorer-album-publisher">{album.publisher ?? "Publisher unknown"}</span>
          <small>
            {album.originalYear ?? "Year unknown"} · {formatCount(album.totalTracks)} tracks · {formatDuration(album.durationSeconds)}
            {tracksTruncated ? " · first 100 shown" : ""}
          </small>
          <span className="deep-explorer-album-score">
            <AlbumRatingStars rating={album.rating} />
            <span aria-hidden="true">·</span>
            <span><Gauge aria-hidden="true" /> Album Score {album.albumScore === null ? "—" : album.albumScore.toFixed(1)}</span>
          </span>
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
        <>
          {visibleSelectedTrackKeys.size > 1 ? (
            <div className="deep-explorer-selection-bar" role="status">
              <strong>{formatCount(visibleSelectedTrackKeys.size)} tracks selected</strong>
              <span>Use Ctrl to toggle or Shift to extend the range.</span>
              <button type="button" onClick={() => requestDelete()}><Trash2 aria-hidden="true" />Delete selected</button>
              <button type="button" onClick={() => setSelectedTrackKeys(new Set())}>Clear</button>
            </div>
          ) : null}
          <TrackTable
            tracks={tracks}
            selectedTrackId={selectedTrackId}
            currentTrackKey={currentTrackKey}
            playbackActive={playbackActive}
            busyTrackKeys={busyTrackKeys}
            onSelectTrack={onSelectTrack}
            onActivateTrack={onActivateTrack}
            onRatingChange={onRatingChange}
            onLoveChange={onLoveChange}
            onDeleteTrack={onDeleteTracks ? requestDelete : undefined}
            multiSelectedTrackKeys={visibleSelectedTrackKeys}
            onSelectionGesture={selectWithModifiers}
            compact
          />
        </>
      )}
      {deleteTargets.length > 0 ? (
        <dialog
          className="deep-explorer-delete-dialog"
          open
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="delete-track-title"
          aria-describedby="delete-track-description"
          onKeyDown={(event) => {
            if (event.key === "Escape" && !deleteBusy) setDeleteTargets([]);
          }}
          onCancel={(event) => {
            event.preventDefault();
            if (!deleteBusy) setDeleteTargets([]);
          }}
        >
          <span className="deep-explorer-delete-dialog__icon"><Trash2 aria-hidden="true" /></span>
          <div>
            <h4 id="delete-track-title">{deleteTargets.length === 1 ? `Delete “${deleteTargets[0].title}”?` : `Delete ${deleteTargets.length} selected tracks?`}</h4>
            <p id="delete-track-description">This permanently deletes {deleteTargets.length === 1 ? "the MP3" : `${deleteTargets.length} MP3 files`} from disk. Music Library will remove {deleteTargets.length === 1 ? "it" : "them"} from the catalog and record {deleteTargets.length === 1 ? "one deleted track" : `${deleteTargets.length} deleted tracks`} in Updates.</p>
            {deleteError ? <p className="deep-explorer-delete-dialog__error" role="alert">{deleteError}</p> : null}
          </div>
          <div className="deep-explorer-delete-dialog__actions">
            <button type="button" disabled={deleteBusy} onClick={() => setDeleteTargets([])}>Cancel</button>
            <button type="button" className="is-destructive" disabled={deleteBusy} autoFocus onClick={() => void confirmDelete()}>
              {deleteBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Trash2 aria-hidden="true" />}
              {deleteBusy ? "Deleting…" : deleteTargets.length === 1 ? "Delete track" : `Delete ${deleteTargets.length} tracks`}
            </button>
          </div>
        </dialog>
      ) : null}
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
    selectedTrackId,
    currentTrackKey = null,
    playbackActive = false,
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
    onDeleteTracks,
  } = props;
  const selectedAlbum = albums.find((album) => album.id === selectedAlbumId) ?? null;
  const [closingDetail, setClosingDetail] = useState<{
    album: ExplorerAlbum;
    tracks: readonly Track[];
    tracksTruncated: boolean;
  } | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const detailAlbum = selectedAlbum ?? closingDetail?.album ?? null;
  const resultCount = resultCountForView(view, props);

  useEffect(() => () => {
    if (closeTimerRef.current !== null) window.clearTimeout(closeTimerRef.current);
  }, []);

  function selectOrToggleAlbum(album: ExplorerAlbum | null) {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
    if (!album || album.id === selectedAlbumId) {
      if (selectedAlbum) {
        setClosingDetail({ album: selectedAlbum, tracks: [...albumTracks], tracksTruncated: albumTracksTruncated });
        onSelectAlbum(null);
        closeTimerRef.current = window.setTimeout(() => {
          setClosingDetail(null);
          closeTimerRef.current = null;
        }, 180);
      } else {
        setClosingDetail(null);
        onSelectAlbum(null);
      }
      return;
    }
    setClosingDetail(null);
    onSelectAlbum(album);
  }

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
        <SortControl
          view={view}
          current={filters.sort}
          onChange={(sort) => updateFilters({ sort })}
        />
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
            currentTrackKey={currentTrackKey}
            playbackActive={playbackActive}
            busyTrackKeys={busyTrackKeys}
            onSelectTrack={onSelectTrack}
            onActivateTrack={onActivateTrack}
            onRatingChange={onRatingChange}
            onLoveChange={onLoveChange}
          />
        ) : view === "albums" ? (
          <AlbumGrid
            albums={albums}
            selectedAlbumId={selectedAlbumId}
            onSelectAlbum={selectOrToggleAlbum}
            detailAlbumId={detailAlbum?.id ?? null}
            detail={detailAlbum ? (
              <AlbumDetail
                key={detailAlbum.id}
                album={detailAlbum}
                tracks={selectedAlbum ? albumTracks : closingDetail?.tracks ?? []}
                tracksTruncated={selectedAlbum ? albumTracksTruncated : closingDetail?.tracksTruncated ?? false}
                state={selectedAlbum ? albumDetailState : "ready"}
                selectedTrackId={selectedTrackId}
                currentTrackKey={currentTrackKey}
                playbackActive={playbackActive}
                closing={!selectedAlbum}
                busyTrackKeys={busyTrackKeys}
                onClose={() => selectOrToggleAlbum(null)}
                onSelectTrack={onSelectTrack}
                onActivateTrack={onActivateTrack}
                onRetry={onRetry}
                onRatingChange={onRatingChange}
                onLoveChange={onLoveChange}
                onDeleteTracks={onDeleteTracks}
              />
            ) : null}
          />
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
