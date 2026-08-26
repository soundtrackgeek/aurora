import {
  ChevronRight,
  Disc3,
  Heart,
  LoaderCircle,
  Play,
  RefreshCw,
  Sparkles,
  Star,
} from "lucide-react";
import { type CSSProperties, useMemo, useState } from "react";
import { displayTrackArtist, type Track } from "../../library";
import type {
  CompletionKind,
  RatingAlbum,
  RatingAlbumPage,
  RatingBand,
  RatingMode,
  RatingsOverview,
} from "../../ratings";
import type { LoveState } from "../../tags";
import { Artwork } from "../Artwork";
import { InlineLoveControl, InlineRatingControl } from "../InlineTagControls";
import "./RatingsStudio.css";

export type RatingsLoadState = "loading" | "ready" | "error";

interface RatingsStudioProps {
  overview: RatingsOverview | null;
  page: RatingAlbumPage | null;
  selectedAlbum: RatingAlbum | null;
  albumTracks: Track[];
  loadState: RatingsLoadState;
  pageState: RatingsLoadState;
  errorMessage: string | null;
  pageError: string | null;
  queueBusy: boolean;
  refreshing: boolean;
  queueMessage: string | null;
  busyTrackKeys: ReadonlySet<string>;
  onCompletionChange: (kind: CompletionKind) => void;
  onSelectAlbum: (album: RatingAlbum) => void;
  onGoToAlbum: (album: RatingAlbum) => void;
  onSelectTrack: (track: Track) => void;
  onPlayTrack: (track: Track) => void;
  onRatingChange: (track: Track, rating: number | null) => void;
  onLoveChange: (track: Track, state: LoveState) => void;
  onPlayCollection: (mode: RatingMode, rating: number | null) => void;
  onExploreCollection: (mode: RatingMode, rating: number | null) => void;
  onPlayUnrated: (album: RatingAlbum) => void;
  onRefresh: () => void;
  onRetry: () => void;
  onRetryPage: () => void;
}

const wholeRatings: ReadonlyArray<number | null> = [null, 1, 2, 3, 4, 5];

function formatCount(value: number): string {
  return new Intl.NumberFormat().format(value);
}

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "—";
  const minutes = Math.max(0, Math.floor(seconds / 60));
  const remainder = Math.max(0, Math.floor(seconds % 60));
  return `${minutes}:${remainder.toString().padStart(2, "0")}`;
}

function albumAsTrack(album: RatingAlbum): Track {
  return {
    id: `rating-album:${album.id}`,
    trackKey: `rating-album:${album.id}`,
    albumId: album.id,
    title: album.title,
    artist: album.artist,
    album: album.title,
    originalYear: album.originalYear,
    releaseYear: album.releaseYear,
    publisher: album.publisher ?? null,
    rating: album.effectiveRating,
    loved: album.lovedTracks > 0,
    loveState: album.lovedTracks > 0 ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: album.durationSeconds,
    genre: album.genre,
    playCount: null,
  };
}

function bandFor(bands: readonly RatingBand[], rating: number | null): RatingBand {
  return bands.find((band) => band.rating === rating) ?? { rating, count: 0 };
}

function bandLabel(rating: number | null): string {
  return rating === null ? "Unrated" : `${rating} ${rating === 1 ? "star" : "stars"}`;
}

const pyramidLevels = [5, 4, 5, 6, 7, 7] as const;
const constellationPalette = [
  { accent: "#d7e0e9", glow: "rgba(215, 224, 233, .34)", saturation: ".28" },
  { accent: "#d5b27b", glow: "rgba(213, 178, 123, .34)", saturation: ".6" },
  { accent: "#4ddbeb", glow: "rgba(77, 219, 235, .38)", saturation: ".86" },
  { accent: "#5bbef5", glow: "rgba(91, 190, 245, .4)", saturation: ".92" },
  { accent: "#8f96ff", glow: "rgba(143, 150, 255, .42)", saturation: "1" },
  { accent: "#d75df5", glow: "rgba(215, 93, 245, .48)", saturation: "1.08" },
] as const;

function pyramidRows(levels: number): number[] {
  return Array.from({ length: levels }, (_, index) => index + 1);
}

function Constellation({
  overview,
  mode,
  selectedRating,
  onModeChange,
  onSelect,
}: {
  overview: RatingsOverview;
  mode: RatingMode;
  selectedRating: number | null;
  onModeChange: (mode: RatingMode) => void;
  onSelect: (rating: number | null) => void;
}) {
  const bands = mode === "tracks" ? overview.trackBands : overview.albumBands;
  const visible = wholeRatings.map((rating) => bandFor(bands, rating));
  const covers = [...overview.initialPage.albums, ...overview.fiveStarAlbums]
    .filter((album, index, albums) => albums.findIndex((candidate) => candidate.id === album.id) === index);
  return <section className="rating-constellation" aria-label={`${mode === "tracks" ? "Track" : "Album"} rating constellation`}>
    <header className="rating-constellation__heading">
      <div><h1>Ratings <span>Taste Constellation</span></h1><p>A cosmic map of your music, from unrated to all-time favorites.</p></div>
      <div className="rating-mode" role="tablist" aria-label="Rating scope">
        <button type="button" role="tab" aria-selected={mode === "tracks"} onClick={() => onModeChange("tracks")}>Track ratings</button>
        <button type="button" role="tab" aria-selected={mode === "albums"} onClick={() => onModeChange("albums")}>Album ratings</button>
      </div>
    </header>
    <div className="constellation-stage">
      {visible.map((band, bandIndex) => {
        const selected = band.rating === selectedRating;
        const palette = constellationPalette[bandIndex];
        let coverOffset = 0;
        return <button
          type="button"
          className={`constellation-band${selected ? " is-selected" : ""}`}
          style={{
            "--constellation-accent": palette.accent,
            "--constellation-glow": palette.glow,
            "--constellation-saturation": palette.saturation,
          } as CSSProperties}
          aria-pressed={selected}
          aria-label={`${bandLabel(band.rating)}, ${formatCount(band.count)}`}
          onClick={() => onSelect(band.rating)}
          key={band.rating ?? "unrated"}
        >
          <span className="constellation-band__covers" aria-hidden="true">
            <span className="constellation-pyramid">
              {pyramidRows(pyramidLevels[bandIndex]).map((rowSize, rowIndex) => <span className="constellation-pyramid__row" key={rowSize}>
                {Array.from({ length: rowSize }, (_, tileIndex) => {
                  const index = coverOffset++;
                  const album = covers[(bandIndex * 5 + index) % Math.max(1, covers.length)];
                  return album ? <span className="constellation-cover" key={`${album.id}:${rowIndex}:${tileIndex}`}><Artwork track={albumAsTrack(album)} /></span> : null;
                })}
              </span>)}
            </span>
          </span>
          <span className="constellation-band__axis"><i /></span>
          <strong>{bandLabel(band.rating)}</strong>
          <em>{formatCount(band.count)}</em>
        </button>;
      })}
    </div>
    <div className="constellation-half-steps" aria-label="Half-star rating points">
      {[0.5, 1.5, 2.5, 3.5, 4.5].map((rating) => {
        const band = bandFor(bands, rating);
        return <button type="button" aria-pressed={selectedRating === rating} onClick={() => onSelect(rating)} key={rating}><span>{rating} ★</span><small>{formatCount(band.count)}</small></button>;
      })}
    </div>
  </section>;
}

const completionTabs: ReadonlyArray<{ kind: CompletionKind; label: string }> = [
  { kind: "almostComplete", label: "Almost complete" },
  { kind: "partiallyRated", label: "Partially rated" },
  { kind: "unrated", label: "Unrated albums" },
];

function Feedback({ detail, error, onRetry }: { detail: boolean; error: string | null; onRetry: () => void }) {
  if (!error) return <div className="ratings-feedback" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" /><strong>{detail ? "Opening this completion lane…" : "Mapping your taste constellation…"}</strong></div>;
  return <div className="ratings-feedback ratings-feedback--error" role="alert"><Disc3 aria-hidden="true" /><strong>{error}</strong><button type="button" onClick={onRetry}><RefreshCw aria-hidden="true" /> Try again</button></div>;
}

function CompletionWorkspace({ props }: { props: RatingsStudioProps }) {
  const { overview, page, selectedAlbum, albumTracks } = props;
  if (!overview) return null;
  return <section className="completion-workspace" aria-labelledby="completion-heading">
    <header className="completion-tabs">
      <div>
        <p className="eyebrow">Album completion studio</p>
        <h2 id="completion-heading">Finish what you love.</h2>
      </div>
      <div className="completion-tabs__controls">
        <div role="tablist" aria-label="Album completion state">
          {completionTabs.map((tab) => <button
            type="button"
            role="tab"
            aria-selected={page?.kind === tab.kind}
            onClick={() => props.onCompletionChange(tab.kind)}
            key={tab.kind}
          >{tab.label} <span>{formatCount(overview.completion[tab.kind])}</span></button>)}
        </div>
        <button
          type="button"
          className="button button--quiet completion-refresh"
          disabled={props.refreshing}
          aria-busy={props.refreshing}
          onClick={props.onRefresh}
        >
          <RefreshCw className={props.refreshing ? "is-spinning" : undefined} aria-hidden="true" />
          {props.refreshing ? "Refreshing…" : "Refresh"}
        </button>
      </div>
    </header>
    {props.pageState !== "ready" || !page ? <Feedback detail error={props.pageError} onRetry={props.onRetryPage} /> : <>
      <div className="completion-shelf" aria-label={`${formatCount(page.total)} ${completionTabs.find((tab) => tab.kind === page.kind)?.label.toLocaleLowerCase()} albums`}>
        {page.albums.map((album) => <button type="button" className={selectedAlbum?.id === album.id ? "is-selected" : undefined} aria-pressed={selectedAlbum?.id === album.id} onClick={() => props.onSelectAlbum(album)} key={album.id}>
          <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
          <strong>{album.title}</strong><span>{album.artist}</span>
          <small>{album.remainingTracks} {album.remainingTracks === 1 ? "rating" : "ratings"} left</small>
        </button>)}
      </div>
      {selectedAlbum ? <div className="completion-detail">
        <div className="completion-detail__album">
          <Artwork track={albumAsTrack(selectedAlbum)} size="large" decorative={false} />
          <div className="completion-detail__summary"><h3>{selectedAlbum.title}</h3><p>{selectedAlbum.artist}</p><span>{selectedAlbum.originalYear ?? "Year unknown"} · {selectedAlbum.genre ?? "Unknown genre"} · {selectedAlbum.totalTracks} tracks</span>
            <div className="completion-progress"><i style={{ width: `${Math.round(selectedAlbum.ratedTracks / Math.max(1, selectedAlbum.totalTracks) * 100)}%` }} /><span>{selectedAlbum.ratedTracks} of {selectedAlbum.totalTracks} rated</span></div>
            <dl><div><dt>Current mean</dt><dd>{selectedAlbum.provisionalRating === null ? "—" : `${selectedAlbum.provisionalRating.toFixed(2)} ★ provisional`}</dd></div><div><dt>Album Score</dt><dd>{selectedAlbum.albumScore === null ? "Available when the effective album rating is valid" : selectedAlbum.albumScore.toFixed(1)}</dd></div></dl>
          </div>
          <div className="completion-detail__actions">
            <button type="button" className="button button--quiet" onClick={() => props.onGoToAlbum(selectedAlbum)}>Go to Album <ChevronRight aria-hidden="true" /></button>
            <button type="button" className="button button--primary" disabled={props.queueBusy || selectedAlbum.remainingTracks === 0} onClick={() => props.onPlayUnrated(selectedAlbum)}>{props.queueBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play unrated tracks</button>
          </div>
        </div>
        <div className="completion-tracks" role="table" aria-label={`${selectedAlbum.title} tracks`}>
          {albumTracks.slice(0, 10).map((track, index) => <div role="row" className="completion-track" onClick={() => props.onSelectTrack(track)} onDoubleClick={() => props.onPlayTrack(track)} tabIndex={0} key={track.trackKey}>
            <span>{index + 1}</span><span className="completion-track__title"><strong>{track.title}</strong><small>[{displayTrackArtist(track)}]</small></span>
            <InlineRatingControl title={track.title} rating={track.rating} busy={props.busyTrackKeys.has(track.trackKey)} allowClear onRatingChange={(rating) => props.onRatingChange(track, rating)} />
            <InlineLoveControl title={track.title} loveState={track.loveState} busy={props.busyTrackKeys.has(track.trackKey)} onLoveChange={(state) => props.onLoveChange(track, state)} />
            <small>{formatDuration(track.durationSeconds)}</small>
          </div>)}
        </div>
      </div> : null}
    </>}
  </section>;
}

export function RatingsStudio(props: RatingsStudioProps) {
  const [mode, setMode] = useState<RatingMode>("tracks");
  const [selectedRating, setSelectedRating] = useState<number | null>(5);
  const selectedBand = useMemo(() => props.overview
    ? bandFor(mode === "tracks" ? props.overview.trackBands : props.overview.albumBands, selectedRating)
    : null, [mode, props.overview, selectedRating]);
  if (props.loadState !== "ready" || !props.overview) {
    return <section className="ratings-studio"><Feedback detail={false} error={props.errorMessage} onRetry={props.onRetry} /></section>;
  }
  return <section className="ratings-studio">
    <Constellation overview={props.overview} mode={mode} selectedRating={selectedRating} onModeChange={(next) => { setMode(next); setSelectedRating(5); }} onSelect={setSelectedRating} />
    <div className="rating-actions">
      <div><Star aria-hidden="true" /><span><strong>{bandLabel(selectedRating)}</strong><small>{formatCount(selectedBand?.count ?? 0)} {mode === "tracks" ? "tracks" : "albums"}</small></span></div>
      {mode === "tracks" && selectedRating === 5 ? <button type="button" className="five-star-collection" onClick={() => props.onPlayCollection(mode, selectedRating)}><Star aria-hidden="true" /> 5 Star Collection <ChevronRight aria-hidden="true" /></button> : null}
      <button type="button" className="button button--primary" disabled={props.queueBusy || !selectedBand?.count} onClick={() => props.onPlayCollection(mode, selectedRating)}>{props.queueBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play collection</button>
      <button type="button" className="button button--quiet" onClick={() => props.onExploreCollection(mode, selectedRating)}><Sparkles aria-hidden="true" /> Explore</button>
      {props.queueMessage ? <span role="status">{props.queueMessage}</span> : null}
    </div>
    <CompletionWorkspace props={props} />
    <footer className="ratings-formula"><Heart aria-hidden="true" /><span><strong>Album Score stays numeric.</strong> Effective album stars feed Music Library's formula alongside 5-star time and Love; partial means remain provisional.</span></footer>
  </section>;
}

export function RatingAlbumInspector({ album, busy, onPlay }: { album: RatingAlbum; busy: boolean; onPlay: (album: RatingAlbum) => void }) {
  const completion = Math.round(album.ratedTracks / Math.max(1, album.totalTracks) * 100);
  return <div className="rating-album-inspector">
    <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
    <div className="rating-album-inspector__heading"><div><h2>{album.title}</h2><p>{album.artist}</p></div>{album.lovedTracks > 0 ? <span><Heart aria-hidden="true" /> {formatCount(album.lovedTracks)}</span> : null}</div>
    <dl className="metadata-list">
      <div><dt>Original Year</dt><dd>{album.originalYear ?? "—"}</dd></div>
      <div><dt>Release Year</dt><dd>{album.releaseYear ?? "—"}</dd></div>
      <div className="publisher-metadata"><dt>Publisher</dt><dd>{album.publisher ?? "Unknown"}</dd></div>
      <div><dt>Track completion</dt><dd>{completion}% · {album.ratedTracks}/{album.totalTracks}</dd></div>
      <div><dt>Album rating</dt><dd>{album.effectiveRating === null ? "—" : `${album.effectiveRating.toFixed(2)} ★`}</dd></div>
      {album.effectiveRating === null && album.provisionalRating !== null ? <div><dt>Current mean</dt><dd>{album.provisionalRating.toFixed(2)} ★ provisional</dd></div> : null}
      <div><dt>Album Score</dt><dd>{album.albumScore === null ? "—" : album.albumScore.toFixed(1)}</dd></div>
      <div><dt>Genre</dt><dd>{album.genre ?? "Unknown"}</dd></div>
      <div><dt>Duration</dt><dd>{formatDuration(album.durationSeconds)}</dd></div>
    </dl>
    <button type="button" className="button button--primary rating-album-inspector__play" disabled={busy || album.remainingTracks === 0} onClick={() => onPlay(album)}>{busy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play unrated tracks</button>
    <p><Sparkles aria-hidden="true" /> Album Score uses Music Library's exact formula. Provisional means never enter the album-rating constellation.</p>
  </div>;
}
