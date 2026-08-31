import {
  BadgeCheck,
  CalendarRange,
  Disc3,
  ImagePlus,
  LoaderCircle,
  Play,
  RefreshCw,
  Sparkles,
  Star,
  Undo2,
} from "lucide-react";
import { memo, useMemo, useRef, useState, type ChangeEvent } from "react";
import { Artwork } from "../Artwork";
import { ArtistSmartLink } from "../ArtistSmartLink";
import { formatCount, formatDuration, type Track } from "../../library";
import {
  clearPublisherLogoOverride,
  loadPublisherLogoOverrides,
  preparePublisherLogo,
  publisherLogoKey,
  publisherLogoVariant,
  publisherMonogram,
  savePublisherLogoOverride,
} from "../../publisherLogos";
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
const TIMELINE_PRESENT_YEAR = 2026;

interface PublisherLogoMessage {
  kind: "success" | "error";
  publisher: string;
  text: string;
}

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

function PublisherLogo({ publisher, logoUrl, large = false }: { publisher: string; logoUrl: string | null; large?: boolean }) {
  const [failedLogoUrl, setFailedLogoUrl] = useState<string | null>(null);
  const monogram = publisherMonogram(publisher);
  const showImage = Boolean(logoUrl) && failedLogoUrl !== logoUrl;
  return (
    <span
      className={`publisher-logo${large ? " publisher-logo--large" : ""}${showImage ? " has-image" : " publisher-logo--generated"}`}
      data-variant={publisherLogoVariant(publisher)}
      aria-hidden="true"
    >
      {showImage
        ? <img src={logoUrl ?? undefined} alt="" onError={() => setFailedLogoUrl(logoUrl)} />
        : <span className={`publisher-logo__monogram is-${Math.min(3, monogram.length)}`}>{monogram}</span>}
    </span>
  );
}

function timelineDomain(overview: PublisherOverview) {
  const years = overview.publishers.flatMap((publisher) => [
    publisher.firstYear,
    publisher.lastYear,
    ...publisher.releaseActivity.map((bucket) => bucket.year),
    ...publisher.originalActivity.map((bucket) => bucket.year),
  ]).filter((year): year is number => year !== null);
  const first = Math.max(1900, Math.floor(Math.min(...years, 1950) / 10) * 10);
  const last = Math.max(first + 10, Math.max(...years, TIMELINE_PRESENT_YEAR));
  return { first, last };
}

function areaPaths(
  buckets: readonly PublisherActivityBucket[],
  maximum: number,
  firstYear: number,
  lastYear: number,
) {
  if (!buckets.length) return { line: "", area: "", endpoint: null };
  const yearSpan = Math.max(1, lastYear - firstYear);
  const points = buckets.map((bucket) => {
    const x = (bucket.year - firstYear) / yearSpan * CHART_WIDTH;
    const height = Math.sqrt(bucket.albumCount / Math.max(1, maximum)) * 54;
    return [x, BASELINE - height] as const;
  });
  const line = points.map(([x, y], index) => `${index === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`).join(" ");
  const [firstX] = points[0];
  const [lastX, lastY] = points[points.length - 1];
  return {
    line,
    area: `M${firstX.toFixed(2)} ${BASELINE} ${line.replace(/^M/, "L")} L${lastX.toFixed(2)} ${BASELINE} Z`,
    endpoint: {
      left: lastX / CHART_WIDTH * 100,
      top: lastY / CHART_HEIGHT * 100,
    },
  };
}

const PublisherSignal = memo(function PublisherSignal({
  publisher,
  selected,
  mode,
  shareMaximum,
  logoUrl,
  firstYear,
  lastYear,
  onSelect,
}: {
  publisher: PublisherSummary;
  selected: boolean;
  mode: PublisherTimelineMode;
  shareMaximum: number;
  logoUrl: string | null;
  firstYear: number;
  lastYear: number;
  onSelect: () => void;
}) {
  const buckets = activityFor(publisher, mode);
  const ownMaximum = Math.max(1, ...buckets.map((bucket) => bucket.albumCount));
  const maximum = mode === "share" ? Math.max(1, shareMaximum) : ownMaximum;
  const paths = areaPaths(buckets, maximum, firstYear, lastYear);
  const gradientId = `publisher-signal-${publisher.name.toLocaleLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
  return (
    <button
      type="button"
      className={`publisher-signal${selected ? " is-selected" : ""}`}
      aria-pressed={selected}
      onClick={onSelect}
    >
      <span className="publisher-signal__identity">
        <PublisherLogo publisher={publisher.name} logoUrl={logoUrl} />
        <span>
          <strong>{publisher.name}</strong>
          <small>Albums <b>{formatCount(publisher.albumCount)}</b></small>
          <small>Tracks <b>{formatCount(publisher.trackCount)}</b></small>
        </span>
      </span>
      <span className="publisher-signal__plot">
        <svg className="publisher-signal__chart" viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} preserveAspectRatio="none" role="img" aria-label={`${publisher.name}, ${formatCount(publisher.albumCount)} albums between ${publisher.firstYear ?? "an unknown year"} and ${publisher.lastYear ?? "an unknown year"}`}>
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stopColor={selected ? "#dc5eff" : "#23c8f2"} stopOpacity=".92" />
              <stop offset="1" stopColor={selected ? "#7c31ad" : "#08729e"} stopOpacity=".08" />
            </linearGradient>
          </defs>
          <path className="publisher-signal__grid" d={`M0 ${BASELINE} H${CHART_WIDTH}`} />
          <path className="publisher-signal__area" d={paths.area} fill={`url(#${gradientId})`} />
          <path className="publisher-signal__line" d={paths.line} />
        </svg>
        {paths.endpoint ? <span className="publisher-signal__endpoint" style={{ left: `${paths.endpoint.left}%`, top: `${paths.endpoint.top}%` }} /> : null}
      </span>
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

function TimelineTicks({ first, last }: { first: number; last: number }) {
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
  logoUrl,
  hasLogoOverride,
  logoBusy,
  logoMessage,
  onChooseLogo,
  onClearLogo,
}: Pick<PublisherSignalTimelineProps, "selectedAlbumId" | "queueBusy" | "queueMessage" | "onSelectAlbum" | "onExplore" | "onPlayPublisher"> & {
  detail: PublisherDetail;
  logoUrl: string | null;
  hasLogoOverride: boolean;
  logoBusy: boolean;
  logoMessage: PublisherLogoMessage | null;
  onChooseLogo: () => void;
  onClearLogo: () => void;
}) {
  const publisher = detail.publisher;
  const highlights = useMemo(() => groupHighlights(detail.albums), [detail.albums]);
  return (
    <section className="publisher-selection" aria-labelledby="publisher-selection-title">
      <header>
        <div className="publisher-selection__identity">
          <PublisherLogo publisher={publisher.name} logoUrl={logoUrl} large />
          <span>
            <span className="publisher-selection__name"><h2 id="publisher-selection-title">{publisher.name}</h2><em><BadgeCheck aria-hidden="true" /> Selected</em></span>
            <p>Publisher value preserved exactly as stored in your Music Library catalog. Logo choices stay on this device.</p>
          </span>
        </div>
        <dl>
          <div><dt>Albums</dt><dd>{formatCount(publisher.albumCount)}</dd></div>
          <div><dt>Tracks</dt><dd>{formatCount(publisher.trackCount)}</dd></div>
          <div><dt>First release</dt><dd>{publisher.firstYear ?? "—"}</dd></div>
        </dl>
      </header>
      <div className="publisher-selection__actions">
        {logoMessage ? <span className={logoMessage.kind === "error" ? "is-error" : undefined} role={logoMessage.kind === "error" ? "alert" : "status"}>{logoMessage.text}</span> : queueMessage ? <span role="status">{queueMessage}</span> : null}
        <button type="button" className="button button--quiet" onClick={() => onExplore(publisher.name)}><Sparkles aria-hidden="true" /> Explore publisher</button>
        <button type="button" className="button button--quiet" disabled={logoBusy} onClick={onChooseLogo}>{logoBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <ImagePlus aria-hidden="true" />} Choose logo</button>
        {hasLogoOverride ? <button type="button" className="button button--quiet" disabled={logoBusy} onClick={onClearLogo}><Undo2 aria-hidden="true" /> Use monogram</button> : null}
        <button type="button" className="button button--primary" disabled={queueBusy || publisher.trackCount === 0} onClick={() => onPlayPublisher(publisher.name)}>{queueBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play selection</button>
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
  const [logoOverrides, setLogoOverrides] = useState(loadPublisherLogoOverrides);
  const [logoBusy, setLogoBusy] = useState(false);
  const [logoMessage, setLogoMessage] = useState<PublisherLogoMessage | null>(null);
  const logoOverridesRef = useRef(logoOverrides);
  const logoInputRef = useRef<HTMLInputElement>(null);
  const selectedName = props.detail?.publisher.name ?? null;

  async function choosePublisherLogo(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file || !selectedName || logoBusy) return;
    const publisher = selectedName;
    setLogoBusy(true);
    setLogoMessage(null);
    try {
      const dataUrl = await preparePublisherLogo(file);
      const next = savePublisherLogoOverride(logoOverridesRef.current, publisher, dataUrl);
      logoOverridesRef.current = next;
      setLogoOverrides(next);
      setLogoMessage({ kind: "success", publisher, text: `Using a local logo for ${publisher}.` });
    } catch (error) {
      setLogoMessage({ kind: "error", publisher, text: error instanceof Error ? error.message : String(error) });
    } finally {
      setLogoBusy(false);
    }
  }

  function clearSelectedPublisherLogo() {
    if (!selectedName || logoBusy) return;
    try {
      const next = clearPublisherLogoOverride(logoOverridesRef.current, selectedName);
      logoOverridesRef.current = next;
      setLogoOverrides(next);
      setLogoMessage({ kind: "success", publisher: selectedName, text: `Restored the Aurora monogram for ${selectedName}.` });
    } catch (error) {
      setLogoMessage({ kind: "error", publisher: selectedName, text: error instanceof Error ? error.message : String(error) });
    }
  }

  if (props.loadState !== "ready" || !props.overview) {
    return <section className="publisher-timeline"><Feedback detail={false} state={props.loadState} error={props.errorMessage} onRetry={props.onRetry} /></section>;
  }
  const shareMaximum = Math.max(1, ...props.overview.publishers.flatMap((publisher) => publisher.releaseActivity.map((bucket) => bucket.albumCount)));
  const domain = timelineDomain(props.overview);
  return (
    <section className="publisher-timeline" aria-label="Publishers">
      <TimelineHeader mode={mode} onChange={setMode} />
      <div className="publisher-signals">
        <TimelineTicks first={domain.first} last={domain.last} />
        {props.overview.publishers.map((publisher) => (
          <PublisherSignal
            publisher={publisher}
            selected={selectedName === publisher.name}
            mode={mode}
            shareMaximum={shareMaximum}
            logoUrl={logoOverrides[publisherLogoKey(publisher.name)]?.dataUrl ?? publisher.logoUrl}
            firstYear={domain.first}
            lastYear={domain.last}
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
          logoUrl={logoOverrides[publisherLogoKey(props.detail.publisher.name)]?.dataUrl ?? props.detail.publisher.logoUrl}
          hasLogoOverride={Boolean(logoOverrides[publisherLogoKey(props.detail.publisher.name)])}
          logoBusy={logoBusy}
          logoMessage={logoMessage?.publisher === props.detail.publisher.name ? logoMessage : null}
          onChooseLogo={() => logoInputRef.current?.click()}
          onClearLogo={clearSelectedPublisherLogo}
          onSelectAlbum={props.onSelectAlbum}
          onExplore={props.onExplore}
          onPlayPublisher={props.onPlayPublisher}
        />}
      <input
        ref={logoInputRef}
        className="publisher-logo-input"
        type="file"
        accept="image/png,image/jpeg,image/webp"
        aria-label={selectedName ? `Choose a local logo for ${selectedName}` : "Choose a local publisher logo"}
        onChange={(event) => void choosePublisherLogo(event)}
      />
    </section>
  );
}

export function PublisherAlbumInspector({ album, busy, onPlay, onOpenArtistAlbums }: { album: PublisherAlbum; busy: boolean; onPlay: (album: PublisherAlbum) => void; onOpenArtistAlbums: (artist: string) => void }) {
  return (
    <div className="publisher-album-inspector">
      <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
      <div className="publisher-album-inspector__heading"><div><h2>{album.title}</h2><p><ArtistSmartLink artist={album.artist} onOpen={onOpenArtistAlbums} /></p></div>{album.lovedTracks > 0 ? <span><Star aria-hidden="true" /> {album.rating?.toFixed(1) ?? "—"}</span> : null}</div>
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
