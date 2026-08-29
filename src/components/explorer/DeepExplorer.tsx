import {
  Album,
  AlertTriangle,
  AudioLines,
  ChevronDown,
  ChevronRight,
  Disc3,
  Gauge,
  FolderOutput,
  Heart,
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
import { albumCoverUrl, displayTrackArtist, formatCount, formatDuration, type Artist, type Track, type YearBasis } from "../../library";
import { Artwork } from "../Artwork";
import { ArtistPortrait } from "../ArtistPortrait";
import { libraryIntakeAdapter, type LibraryIntakePreview } from "../../ingest";
import { loadInboxSettings } from "../../inbox";
import { InlineLoveControl, InlineRatingControl } from "../InlineTagControls";
import type { CatalogChartRank } from "../../charts";
import { CatalogChartRanks } from "../charts/CatalogChartRanks";
import { CountryFlag } from "../CountryFlag";
import { applyWindowsSelection, type SelectionModifiers } from "./windowsSelection";
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
  originCountryCode?: string | null;
  originCountryName?: string | null;
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

export type ExplorerSelection =
  | { kind: "tracks"; tracks: readonly Track[] }
  | { kind: "albums"; albums: readonly ExplorerAlbum[] };

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
  trackChartRanks?: Readonly<Record<string, readonly CatalogChartRank[]>>;
  albumChartRanks?: Readonly<Record<string, readonly CatalogChartRank[]>>;
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
  onAlbumMovedToInbox?: () => boolean | void | Promise<boolean | void>;
  onSelectionChange?: (selection: ExplorerSelection) => void;
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
    { value: "added", primary: "newest", primaryLabel: "Added · newest", reverse: "oldest", reverseLabel: "Added · oldest" },
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
  chartRanks,
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
  chartRanks?: Readonly<Record<string, readonly CatalogChartRank[]>>;
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
            <th className="deep-explorer-table__position is-numeric">{compact ? "Track" : "Year"}</th>
            <th>Genre</th>
            <th>Publisher</th>
            <th className="is-numeric">Time</th>
            <th className="is-numeric">Plays</th>
            <th>Rating</th>
            <th aria-label="Love" />
            {onDeleteTrack ? <th className="deep-explorer-table__delete-heading" aria-label="Delete" /> : null}
          </tr>
        </thead>
        <tbody>
          {tracks.map((track, index) => {
            const selected = multiSelectedTrackKeys
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
                      <span className="deep-explorer-track-heading">
                        <strong>{track.title}</strong>
                        <CatalogChartRanks kind="track" ranks={chartRanks?.[track.id]} />
                        {compact && track.lastFmAlbumRank ? (
                          <span
                            className="deep-explorer-track-popular"
                            aria-label={`Number ${track.lastFmAlbumRank} of this album's top 3 Last.fm tracks`}
                            title={`#${track.lastFmAlbumRank} on Last.fm`}
                          >
                            🔥
                          </span>
                        ) : null}
                        {compact ? <small className="deep-explorer-track-artist">[{displayTrackArtist(track)}]</small> : null}
                      </span>
                      {track.tagSyncState ? <small>Pending tag import</small> : null}
                    </span>
                  </span>
                </td>
                <td>{displayTrackArtist(track)}</td>
                <td>{track.album}</td>
                <td className="deep-explorer-table__position is-numeric">
                  {compact
                    ? track.trackNumber === null || track.trackNumber === undefined
                      ? "—"
                      : String(track.trackNumber).padStart(2, "0")
                    : track.originalYear ?? "—"}
                </td>
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
  selectedAlbumIds,
  onSelectAlbum,
  onSelectionGesture,
  detailAlbumId,
  detail,
  chartRanks,
}: {
  albums: readonly ExplorerAlbum[];
  selectedAlbumId: string | null;
  selectedAlbumIds?: ReadonlySet<string>;
  onSelectAlbum: (album: ExplorerAlbum | null) => void;
  onSelectionGesture?: (album: ExplorerAlbum, index: number, modifiers: SelectionModifiers) => boolean;
  detailAlbumId: string | null;
  detail: ReactNode;
  chartRanks?: Readonly<Record<string, readonly CatalogChartRank[]>>;
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
            const index = albums.indexOf(album);
            const selected = selectedAlbumIds ? selectedAlbumIds.has(album.id) : selectedAlbumId === album.id;
            const expanded = selectedAlbumId === album.id;
            return (
              <button
                type="button"
                className={`deep-explorer-album${selected ? " is-selected" : ""}`}
                aria-pressed={selected}
                aria-expanded={expanded}
                onClick={(event) => {
                  const remainsSelected = onSelectionGesture?.(album, index, {
                    ctrl: event.ctrlKey || event.metaKey,
                    shift: event.shiftKey,
                  }) ?? true;
                  onSelectAlbum(remainsSelected ? album : null);
                }}
                key={album.id}
              >
                <AlbumArtwork album={album} />
                <span className="deep-explorer-album__copy">
                  <strong>{album.title}</strong>
                  <span className="deep-explorer-album__artist">
                    <CountryFlag code={album.originCountryCode} name={album.originCountryName} />
                    <span>{album.artist}</span>
                  </span>
                  <small className="deep-explorer-album__metadata">
                    {album.originalYear ?? "Year unknown"} <span aria-hidden="true">—</span> {album.genre ?? "Genre unknown"} <span aria-hidden="true">—</span> {album.publisher ?? "Publisher unknown"}
                  </small>
                  <small className="deep-explorer-album__length">
                    {formatCount(album.totalTracks)} tracks <span aria-hidden="true">—</span> {formatDuration(album.durationSeconds)}
                    <CatalogChartRanks kind="album" ranks={chartRanks?.[album.id]} />
                  </small>
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
  onAlbumMovedToInbox,
  onSelectionChange,
  trackChartRanks,
  albumChartRanks,
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
  onAlbumMovedToInbox?: () => boolean | void | Promise<boolean | void>;
  onSelectionChange?: (selection: ExplorerSelection) => void;
  trackChartRanks?: Readonly<Record<string, readonly CatalogChartRank[]>>;
  albumChartRanks?: Readonly<Record<string, readonly CatalogChartRank[]>>;
}) {
  const [selectedTrackKeys, setSelectedTrackKeys] = useState<ReadonlySet<string>>(() => new Set(
    tracks.filter((track) => track.id === selectedTrackId).map((track) => track.trackKey),
  ));
  const [selectionAnchorKey, setSelectionAnchorKey] = useState<string | null>(() => (
    tracks.find((track) => track.id === selectedTrackId)?.trackKey ?? null
  ));
  const [deleteTargets, setDeleteTargets] = useState<readonly Track[]>([]);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [movePreview, setMovePreview] = useState<LibraryIntakePreview | null>(null);
  const [moveBusy, setMoveBusy] = useState(false);
  const [moveError, setMoveError] = useState<string | null>(null);

  const visibleSelectedTrackKeys = useMemo(() => {
    const visibleKeys = new Set(tracks.map((track) => track.trackKey));
    return new Set([...selectedTrackKeys].filter((key) => visibleKeys.has(key)));
  }, [selectedTrackKeys, tracks]);

  function selectWithModifiers(track: Track, _index: number, modifiers: { ctrl: boolean; shift: boolean }) {
    const next = applyWindowsSelection(
      tracks.map((candidate) => candidate.trackKey),
      selectedTrackKeys,
      selectionAnchorKey,
      track.trackKey,
      modifiers,
    );
    setSelectedTrackKeys(next.selectedKeys);
    setSelectionAnchorKey(next.anchorKey);
    onSelectionChange?.({
      kind: "tracks",
      tracks: tracks.filter((candidate) => next.selectedKeys.has(candidate.trackKey)),
    });
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

  async function requestMoveToInbox() {
    if (!onAlbumMovedToInbox || moveBusy) return;
    setMoveBusy(true);
    setMoveError(null);
    try {
      const settings = await loadInboxSettings();
      if (settings.monitoredFolders.length === 0) {
        throw new Error("Add a monitored folder in Aurora Inbox before moving an album back.");
      }
      const inboxPath = settings.monitoredFolders.length === 1
        ? settings.monitoredFolders[0]
        : await libraryIntakeAdapter.selectFolder();
      if (!inboxPath) return;
      const normalized = (value: string) => value.replace(/\//g, "\\").replace(/\\+$/, "").toLocaleLowerCase();
      if (!settings.monitoredFolders.some((folder) => normalized(folder) === normalized(inboxPath))) {
        throw new Error("Choose one of the monitored folders configured in Aurora Inbox.");
      }
      setMovePreview(await libraryIntakeAdapter.previewMoveToInbox({ albumId: album.id, inboxPath }));
    } catch (error) {
      setMoveError(error instanceof Error ? error.message : String(error));
    } finally {
      setMoveBusy(false);
    }
  }

  async function applyMoveToInbox() {
    if (!movePreview || moveBusy) return;
    setMoveBusy(true);
    setMoveError(null);
    try {
      await libraryIntakeAdapter.apply({ planId: movePreview.planId, sessionId: movePreview.sessionId });
      setMovePreview(null);
      await onAlbumMovedToInbox?.();
      onClose();
    } catch (error) {
      setMoveError(error instanceof Error ? error.message : String(error));
    } finally {
      setMoveBusy(false);
    }
  }

  return (
    <aside className={`deep-explorer-album-detail${closing ? " is-closing" : ""}`} aria-label={`${album.title} album details`}>
      <header>
        <AlbumArtwork album={album} detail />
        <div>
          <span className="deep-explorer-kicker">Album detail</span>
          <h3>{album.title}</h3>
          <p className="deep-explorer-album-detail__artist">
            <CountryFlag code={album.originCountryCode} name={album.originCountryName} />
            <span>{album.artist}</span>
          </p>
          <span className="deep-explorer-album-publisher">
            {album.genre ?? "Genre unknown"} <span aria-hidden="true">—</span> {album.publisher ?? "Publisher unknown"}
          </span>
          <small>
            {album.originalYear ?? "Year unknown"} · {formatCount(album.totalTracks)} tracks · {formatDuration(album.durationSeconds)}
            {tracksTruncated ? " · first 100 shown" : ""}
            <CatalogChartRanks kind="album" ranks={albumChartRanks?.[album.id]} />
          </small>
          <span className="deep-explorer-album-score">
            <AlbumRatingStars rating={album.rating} />
            <span aria-hidden="true">·</span>
            <span><Gauge aria-hidden="true" /> Album Score {album.albumScore === null ? "—" : album.albumScore.toFixed(1)}</span>
          </span>
        </div>
        {onAlbumMovedToInbox ? <button type="button" className="deep-explorer-move-inbox" disabled={moveBusy} onClick={() => void requestMoveToInbox()}><FolderOutput aria-hidden="true" />{moveBusy && !movePreview ? "Preparing…" : "Move to Inbox"}</button> : null}
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
            chartRanks={trackChartRanks}
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
      {movePreview ? (
        <dialog className="deep-explorer-delete-dialog deep-explorer-move-dialog" open aria-modal="true" aria-labelledby="move-album-title">
          <span className="deep-explorer-delete-dialog__icon"><FolderOutput aria-hidden="true" /></span>
          <div>
            <h4 id="move-album-title">Move “{album.title}” back to Inbox?</h4>
            <p>Aurora will copy and verify all {movePreview.trackCount} tracks at <strong>{movePreview.albums[0]?.destinationPath}</strong>, remove this album from the Music Library catalog, then delete the old library folder only after the catalog commit succeeds.</p>
            {moveError ? <p className="deep-explorer-delete-dialog__error" role="alert">{moveError}</p> : null}
          </div>
          <div className="deep-explorer-delete-dialog__actions">
            <button type="button" disabled={moveBusy} onClick={() => { setMovePreview(null); setMoveError(null); }}>Cancel</button>
            <button type="button" disabled={moveBusy} autoFocus onClick={() => void applyMoveToInbox()}>{moveBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <FolderOutput aria-hidden="true" />}{moveBusy ? "Moving…" : "Move album to Inbox"}</button>
          </div>
        </dialog>
      ) : moveError ? <p className="deep-explorer-album-detail__error" role="alert">{moveError}</p> : null}
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
            <ArtistPortrait artist={artist.name} className="deep-explorer-artist__avatar" />
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
    trackChartRanks = {},
    albumChartRanks = {},
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
    onAlbumMovedToInbox,
    onSelectionChange,
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
  const [selectedTrackKeys, setSelectedTrackKeys] = useState<ReadonlySet<string>>(() => new Set(
    tracks.filter((track) => track.id === selectedTrackId).map((track) => track.trackKey),
  ));
  const [trackSelectionAnchorKey, setTrackSelectionAnchorKey] = useState<string | null>(() => (
    tracks.find((track) => track.id === selectedTrackId)?.trackKey ?? null
  ));
  const [selectedAlbumIds, setSelectedAlbumIds] = useState<ReadonlySet<string>>(() => new Set(
    selectedAlbumId ? [selectedAlbumId] : [],
  ));
  const [albumSelectionAnchorId, setAlbumSelectionAnchorId] = useState<string | null>(selectedAlbumId);
  const selectionResetReadyRef = useRef(false);
  const loadMoreSentinelRef = useRef<HTMLDivElement>(null);
  const loadMoreCallbackRef = useRef(onLoadMore);
  const lastRequestedLoadedRef = useRef<number | null>(null);

  useEffect(() => {
    loadMoreCallbackRef.current = onLoadMore;
  }, [onLoadMore]);

  useEffect(() => {
    lastRequestedLoadedRef.current = null;
  }, [filters, view]);

  useEffect(() => {
    if (loadState !== "ready") lastRequestedLoadedRef.current = null;
  }, [loadState]);

  useEffect(() => {
    if (!selectionResetReadyRef.current) {
      selectionResetReadyRef.current = true;
      return;
    }
    setSelectedTrackKeys(new Set());
    setTrackSelectionAnchorKey(null);
    setSelectedAlbumIds(new Set());
    setAlbumSelectionAnchorId(null);
  }, [filters, view]);

  useEffect(() => () => {
    if (closeTimerRef.current !== null) window.clearTimeout(closeTimerRef.current);
  }, []);

  useEffect(() => {
    const sentinel = loadMoreSentinelRef.current;
    if (
      !sentinel
      || loadState !== "ready"
      || !pageInfo.hasMore
      || pageInfo.isLoadingMore
      || !loadMoreCallbackRef.current
      || typeof IntersectionObserver === "undefined"
    ) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry?.isIntersecting || lastRequestedLoadedRef.current === pageInfo.loaded) return;
      lastRequestedLoadedRef.current = pageInfo.loaded;
      loadMoreCallbackRef.current?.();
    }, { rootMargin: "0px 0px 160px", threshold: 0 });
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [loadState, pageInfo.hasMore, pageInfo.isLoadingMore, pageInfo.loaded]);

  function selectOrToggleAlbum(album: ExplorerAlbum | null) {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
    if (!album) {
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

  function selectTracks(track: Track, _index: number, modifiers: SelectionModifiers) {
    const next = applyWindowsSelection(
      tracks.map((candidate) => candidate.trackKey),
      selectedTrackKeys,
      trackSelectionAnchorKey,
      track.trackKey,
      modifiers,
    );
    setSelectedTrackKeys(next.selectedKeys);
    setTrackSelectionAnchorKey(next.anchorKey);
    onSelectionChange?.({
      kind: "tracks",
      tracks: tracks.filter((candidate) => next.selectedKeys.has(candidate.trackKey)),
    });
  }

  function selectAlbums(album: ExplorerAlbum, _index: number, modifiers: SelectionModifiers): boolean {
    if (!modifiers.ctrl && !modifiers.shift && selectedAlbumId === album.id) {
      setSelectedAlbumIds(new Set());
      setAlbumSelectionAnchorId(null);
      onSelectionChange?.({ kind: "albums", albums: [] });
      return false;
    }
    const next = applyWindowsSelection(
      albums.map((candidate) => candidate.id),
      selectedAlbumIds,
      albumSelectionAnchorId,
      album.id,
      modifiers,
    );
    setSelectedAlbumIds(next.selectedKeys);
    setAlbumSelectionAnchorId(next.anchorKey);
    onSelectionChange?.({
      kind: "albums",
      albums: albums.filter((candidate) => next.selectedKeys.has(candidate.id)),
    });
    return next.selectedKeys.has(album.id);
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
            multiSelectedTrackKeys={selectedTrackKeys}
            onSelectionGesture={selectTracks}
            chartRanks={trackChartRanks}
          />
        ) : view === "albums" ? (
          <AlbumGrid
            albums={albums}
            selectedAlbumId={selectedAlbumId}
            selectedAlbumIds={selectedAlbumIds}
            onSelectAlbum={selectOrToggleAlbum}
            onSelectionGesture={selectAlbums}
            detailAlbumId={detailAlbum?.id ?? null}
            chartRanks={albumChartRanks}
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
                onAlbumMovedToInbox={onAlbumMovedToInbox}
                onSelectionChange={onSelectionChange}
                trackChartRanks={trackChartRanks}
                albumChartRanks={albumChartRanks}
              />
            ) : null}
          />
        ) : (
          <ArtistList artists={artists} selectedArtistId={selectedArtistId} onSelectArtist={onSelectArtist} />
        )}
        {loadState === "ready" && resultCount > 0 && pageInfo.hasMore && onLoadMore ? (
          <div className="deep-explorer-load-sentinel" ref={loadMoreSentinelRef} aria-hidden="true" />
        ) : null}
      </div>

      {loadState === "ready" && resultCount > 0 ? (
        <footer className="deep-explorer-pagination">
          <span>
            Loaded <strong>{formatCount(pageInfo.loaded)}</strong>{pageInfo.hasMore ? " · more available" : ""}
          </span>
          {pageInfo.hasMore && onLoadMore ? (
            <span className="deep-explorer-pagination__loading" aria-live="polite">
              {pageInfo.isLoadingMore ? <><LoaderCircle className="is-spinning" aria-hidden="true" />Loading next 50…</> : "Scroll for the next 50"}
            </span>
          ) : (
            <span className="deep-explorer-pagination__end">End of this result set</span>
          )}
        </footer>
      ) : null}
    </section>
  );
}
