import {
  BadgeCheck,
  Building2,
  CalendarRange,
  Disc3,
  LoaderCircle,
  Play,
  RefreshCw,
  Sparkles,
  Star,
} from "lucide-react";
import { memo, useMemo, useState } from "react";
import { Artwork } from "../Artwork";
import { formatCount, formatDuration, type Track } from "../../library";
import type {
  PublisherActivityBucket,
  PublisherAlbum,
  PublisherDetail,
  PublisherOverview,
  PublisherSummary,
  PublisherTimelineMode,
} from "../../publishers";
import "./PublisherSignalTimeline.css";

export type PublisherLoadState = "loading" | "ready" | "error";

interface PublisherSignalTimelineProps {
  overview: PublisherOverview | null;
  detail: PublisherDetail | null;
  loadState: PublisherLoadState;
  detailState: PublisherLoadState;
  errorMessage: string | null;
  detailError: string | null;
  selectedAlbumId: string | null;
  queueBusy: boolean;
  queueMessage: string | null;
  onSelectPublisher: (publisher: PublisherSummary) => void;
  onSelectAlbum: (album: PublisherAlbum) => void;
  onExplore: (publisher: string) => void;
  onPlayPublisher: (publisher: string) => void;
  onRetry: () => void;
  onRetryDetail: () => void;
}

const CHART_WIDTH = 720;
const CHART_HEIGHT = 72;
const BASELINE = 66;

function albumAsTrack(album: PublisherAlbum): Track {
  return {
    id: album.id,
    trackKey: `publisher-album:${album.id}`,
    albumId: album.id,
    title: album.title,
    artist: album.artist,
    album: album.title,
    originalYear: album.originalYear,
    releaseYear: album.releaseYear,
    publisher: album.publisher,
    rating: album.rating,
    loved: album.lovedTracks > 0,
    loveState: album.lovedTracks > 0 ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: album.durationSeconds,
    genre: album.genre,
    playCount: null,
  };
}

function activityFor(publisher: PublisherSummary, mode: PublisherTimelineMode) {
  return mode === "original" ? publisher.originalActivity : publisher.releaseActivity;
}

function areaPaths(buckets: readonly PublisherActivityBucket[], maximum: number) {
  if (!buckets.length) return { line: "", area: "" };
  const span = Math.max(1, buckets.length - 1);
  const points = buckets.map((bucket, index) => {
    const x = index / span * CHART_WIDTH;
    const height = Math.sqrt(bucket.albumCount / Math.max(1, maximum)) * 54;
    return [x, BASELINE - height] as const;
  });
  const line = points.map(([x, y], index) => `${index === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`).join(" ");
  return {
    line,
    area: `M0 ${BASELINE} ${line.replace(/^M/, "L")} L${CHART_WIDTH} ${BASELINE} Z`,
  };
}

const PublisherSignal = memo(function PublisherSignal({
  publisher,
  selected,
  mode,
  shareMaximum,
  onSelect,
}: {
  publisher: PublisherSummary;
  selected: boolean;
  mode: PublisherTimelineMode;
  shareMaximum: number;
  onSelect: () => void;
}) {
  const buckets = activityFor(publisher, mode);
  const ownMaximum = Math.max(1, ...buckets.map((bucket) => bucket.albumCount));
  const maximum = mode === "share" ? Math.max(1, shareMaximum) : ownMaximum;
  const paths = areaPaths(buckets, maximum);
  const gradientId = `publisher-signal-${publisher.name.toLocaleLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
  return (
    <button
      type="button"
      className={`publisher-signal${selected ? " is-selected" : ""}`}
      aria-pressed={selected}
      onClick={onSelect}
    >
      <span className="publisher-signal__identity">
        <span className="publisher-logo" aria-hidden="true">
          {publisher.logoUrl ? <img src={publisher.logoUrl} alt="" /> : <Disc3 />}
        </span>
        <span>
          <strong>{publisher.name}</strong>
          <small>Albums <b>{formatCount(publisher.albumCount)}</b></small>
          <small>Tracks <b>{formatCount(publisher.trackCount)}</b></small>
        </span>
      </span>
      <svg className="publisher-signal__chart" viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} role="img" aria-label={`${publisher.name}, ${formatCount(publisher.albumCount)} albums between ${publisher.firstYear ?? "an unknown year"} and ${publisher.lastYear ?? "an unknown year"}`}>
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={selected ? "#dc5eff" : "#23c8f2"} stopOpacity=".92" />
            <stop offset="1" stopColor={selected ? "#7c31ad" : "#08729e"} stopOpacity=".08" />
          </linearGradient>
        </defs>
        <path className="publisher-signal__grid" d={`M0 ${BASELINE} H${CHART_WIDTH}`} />
        <path className="publisher-signal__area" d={paths.area} fill={`url(#${gradientId})`} />
        <path className="publisher-signal__line" d={paths.line} />
        <circle className="publisher-signal__endpoint" cx={CHART_WIDTH - 4} cy={BASELINE - 2} r="4" />
      </svg>
    </button>
  );
});

function TimelineHeader({ mode, onChange }: { mode: PublisherTimelineMode; onChange: (mode: PublisherTimelineMode) => void }) {
  const tabs: ReadonlyArray<{ id: PublisherTimelineMode; label: string }> = [
    { id: "release", label: "Release activity" },
    { id: "original", label: "Original-year activity" },
    { id: "share", label: "Catalog share" },
  ];
  return (
    <header className="publisher-timeline__header">
      <div>
        <h1>Publisher Signal Timeline <span>{tabs.find((tab) => tab.id === mode)?.label}</span></h1>
        <p>Explore when publishers were active and discover their catalog across time.</p>
      </div>
      <div className="publisher-timeline__modes" role="tablist" aria-label="Publisher timeline view">
        {tabs.map((tab) => (
          <button type="button" role="tab" aria-selected={mode === tab.id} onClick={() => onChange(tab.id)} key={tab.id}>{tab.label}</button>
        ))}
      </div>
    </header>
  );
}

function TimelineTicks({ overview }: { overview: PublisherOverview }) {
  const allYears = overview.publishers.flatMap((publisher) => [publisher.firstYear, publisher.lastYear]).filter((year): year is number => year !== null);
  const first = Math.max(1900, Math.floor(Math.min(...allYears, 1950) / 10) * 10);
  const last = Math.max(first + 10, Math.max(...allYears, 2026));
  const ticks = [] as number[];
  for (let year = Math.ceil(first / 10) * 10; year <= last; year += 10) ticks.push(year);
  if (ticks[ticks.length - 1] !== last) ticks.push(last);
  return (
    <div className="publisher-timeline__ticks" aria-hidden="true">
      <span />
      <div>{ticks.map((year) => <i style={{ left: `${(year - first) / Math.max(1, last - first) * 100}%` }} key={year}>{year}</i>)}</div>
    </div>
  );
}

function groupHighlights(albums: readonly PublisherAlbum[]) {
  const byDecade = new Map<number, PublisherAlbum>();
  for (const album of albums) {
    const year = album.releaseYear ?? album.originalYear;
    if (year === null) continue;
    const decade = Math.floor(year / 10) * 10;
    if (!byDecade.has(decade)) byDecade.set(decade, album);
  }
  return [...byDecade.entries()].sort(([left], [right]) => left - right).slice(-8);
}

function PublisherSelection({
  detail,
  selectedAlbumId,
  queueBusy,
  queueMessage,
  onSelectAlbum,
  onExplore,
  onPlayPublisher,
}: Pick<PublisherSignalTimelineProps, "selectedAlbumId" | "queueBusy" | "queueMessage" | "onSelectAlbum" | "onExplore" | "onPlayPublisher"> & { detail: PublisherDetail }) {
  const publisher = detail.publisher;
  const highlights = useMemo(() => groupHighlights(detail.albums), [detail.albums]);
  return (
    <section className="publisher-selection" aria-labelledby="publisher-selection-title">
      <header>
        <div className="publisher-selection__identity">
          <span className="publisher-logo publisher-logo--large" aria-hidden="true">{publisher.logoUrl ? <img src={publisher.logoUrl} alt="" /> : <Building2 />}</span>
          <span>
            <span className="publisher-selection__name"><h2 id="publisher-selection-title">{publisher.name}</h2><em><BadgeCheck aria-hidden="true" /> Selected</em></span>
            <p>Publisher value preserved exactly as stored in your Music Library catalog.</p>
          </span>
        </div>
        <dl>
          <div><dt>Albums</dt><dd>{formatCount(publisher.albumCount)}</dd></div>
          <div><dt>Tracks</dt><dd>{formatCount(publisher.trackCount)}</dd></div>
          <div><dt>First release</dt><dd>{publisher.firstYear ?? "—"}</dd></div>
        </dl>
      </header>
      <div className="publisher-selection__actions">
        <button type="button" className="button button--quiet" onClick={() => onExplore(publisher.name)}><Sparkles aria-hidden="true" /> Explore publisher</button>
        <button type="button" className="button button--primary" disabled={queueBusy || publisher.trackCount === 0} onClick={() => onPlayPublisher(publisher.name)}>{queueBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play selection</button>
        {queueMessage ? <span role="status">{queueMessage}</span> : null}
      </div>
      <div className="publisher-highlights">
        <h3>Release highlights by decade</h3>
        <div>
          {highlights.map(([decade, album]) => (
            <button type="button" aria-pressed={selectedAlbumId === album.id} className={selectedAlbumId === album.id ? "is-selected" : undefined} onClick={() => onSelectAlbum(album)} key={`${decade}:${album.id}`}>
              <span>{decade}s</span>
              <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
              <strong>{album.title}</strong>
              <small>{album.artist}</small>
              <em>{album.releaseYear ?? album.originalYear ?? "Year unknown"}</em>
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}

function Feedback({ detail, state, error, onRetry }: { detail: boolean; state: PublisherLoadState; error: string | null; onRetry: () => void }) {
  if (state === "loading") return <div className="publisher-feedback" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" /><strong>{detail ? "Tuning this publisher signal…" : "Reading publisher signals…"}</strong><span>{detail ? "Collecting a bounded album shelf." : "Aggregating publisher activity without loading the full track list."}</span></div>;
  return <div className="publisher-feedback publisher-feedback--error" role="alert"><CalendarRange aria-hidden="true" /><strong>{detail ? "This publisher could not be opened." : "Publisher signals are unavailable."}</strong><span>{error ?? "Aurora could not read this bounded view."}</span><button type="button" onClick={onRetry}><RefreshCw aria-hidden="true" /> Try again</button></div>;
}

export function PublisherSignalTimeline(props: PublisherSignalTimelineProps) {
  const [mode, setMode] = useState<PublisherTimelineMode>("release");
  if (props.loadState !== "ready" || !props.overview) {
    return <section className="publisher-timeline"><Feedback detail={false} state={props.loadState} error={props.errorMessage} onRetry={props.onRetry} /></section>;
  }
  const selectedName = props.detail?.publisher.name ?? null;
  const shareMaximum = Math.max(1, ...props.overview.publishers.flatMap((publisher) => publisher.releaseActivity.map((bucket) => bucket.albumCount)));
  return (
    <section className="publisher-timeline" aria-label="Publishers">
      <TimelineHeader mode={mode} onChange={setMode} />
      <div className="publisher-signals">
        <TimelineTicks overview={props.overview} />
        {props.overview.publishers.map((publisher) => (
          <PublisherSignal
            publisher={publisher}
            selected={selectedName === publisher.name}
            mode={mode}
            shareMaximum={shareMaximum}
            onSelect={() => props.onSelectPublisher(publisher)}
            key={publisher.name}
          />
        ))}
      </div>
      {props.detailState !== "ready" || !props.detail
        ? <Feedback detail state={props.detailState} error={props.detailError} onRetry={props.onRetryDetail} />
        : <PublisherSelection
          detail={props.detail}
          selectedAlbumId={props.selectedAlbumId}
          queueBusy={props.queueBusy}
          queueMessage={props.queueMessage}
          onSelectAlbum={props.onSelectAlbum}
          onExplore={props.onExplore}
          onPlayPublisher={props.onPlayPublisher}
        />}
    </section>
  );
}

export function PublisherAlbumInspector({ album, busy, onPlay }: { album: PublisherAlbum; busy: boolean; onPlay: (album: PublisherAlbum) => void }) {
  return (
    <div className="publisher-album-inspector">
      <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
      <div className="publisher-album-inspector__heading"><div><h2>{album.title}</h2><p>{album.artist}</p></div>{album.lovedTracks > 0 ? <span><Star aria-hidden="true" /> {album.rating?.toFixed(1) ?? "—"}</span> : null}</div>
      <dl className="metadata-list">
        <div><dt>Original Year</dt><dd>{album.originalYear ?? "—"}</dd></div>
        <div><dt>Release Year</dt><dd>{album.releaseYear ?? "—"}</dd></div>
        <div className="publisher-metadata"><dt>Publisher</dt><dd>{album.publisher}</dd></div>
        <div><dt>Format</dt><dd>Album · local edition</dd></div>
        <div><dt>Tracks</dt><dd>{formatCount(album.totalTracks)}</dd></div>
        <div><dt>Duration</dt><dd>{formatDuration(album.durationSeconds)}</dd></div>
        <div><dt>Rating</dt><dd>{album.rating === null ? "—" : `${album.rating.toFixed(1)} ★`}</dd></div>
        <div><dt>Genre</dt><dd>{album.genre ?? "Unknown"}</dd></div>
      </dl>
      <button type="button" className="button button--primary publisher-album-inspector__play" disabled={busy} onClick={() => onPlay(album)}>{busy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play album</button>
      <p><Disc3 aria-hidden="true" /> Publisher is read directly from the catalog and remains searchable across Aurora.</p>
    </div>
  );
}
