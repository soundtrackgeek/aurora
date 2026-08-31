import {
  CalendarRange,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Disc3,
  Heart,
  Layers3,
  ListMusic,
  LoaderCircle,
  Play,
  RefreshCw,
  Sparkles,
  Star,
} from "lucide-react";
import { memo, useMemo, useState, type KeyboardEvent } from "react";
import { formatCount, formatDuration, type Track, type YearBasis } from "../../library";
import {
  type YearAlbum,
  type YearBucket,
  type YearDetail,
  type YearOverview,
  type YearSelection,
  type YearsMode,
} from "../../years";
import { Artwork } from "../Artwork";
import { ArtistSmartLink } from "../ArtistSmartLink";
import type { CatalogChartRank } from "../../charts";
import { CatalogChartRanks } from "../charts/CatalogChartRanks";
import { CountryFlag } from "../CountryFlag";
import "./YearsExplorer.css";

export type YearsLoadState = "loading" | "ready" | "error";

interface YearsExplorerProps {
  overview: YearOverview | null;
  detail: YearDetail | null;
  loadState: YearsLoadState;
  detailState: YearsLoadState;
  errorMessage: string | null;
  detailError: string | null;
  selectedAlbumId: string | null;
  queueBusy: boolean;
  queueMessage: string | null;
  onSelect: (selection: YearSelection) => void;
  onSelectAlbum: (album: YearAlbum) => void;
  onExplore: (selection: YearSelection) => void;
  onPlayYear: (selection: YearSelection) => void;
  onPlayAlbum: (album: YearAlbum) => void;
  onRetry: () => void;
  onRetryDetail: () => void;
}

const VIEW_WIDTH = 900;
const PLOT_LEFT = 28;
const PLOT_RIGHT = 872;
const TOP_BASELINE = 118;
const BOTTOM_BASELINE = 350;

interface AlbumInspectorAlbum {
  id: string;
  title: string;
  artist: string;
  originalYear?: number | null;
  releaseYear: number | null;
  publisher?: string | null;
  originCountryCode?: string | null;
  originCountryName?: string | null;
  totalTracks: number;
  lovedTracks: number;
  durationSeconds: number | null;
  genre: string | null;
  rating: number | null;
  formats?: string[];
}

function albumAsTrack(album: AlbumInspectorAlbum): Track {
  return {
    id: `year-album:${album.id}`,
    trackKey: `year-album:${album.id}`,
    albumId: album.id,
    title: album.title,
    artist: album.artist,
    album: album.title,
    originalYear: album.originalYear,
    releaseYear: album.releaseYear,
    publisher: album.publisher ?? null,
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

function selectionCopy(selection: YearSelection): string {
  const field = selection.basis === "original" ? "Original Year" : "Release Year";
  return selection.year === null ? `Missing ${field}` : `${field} ${selection.year}`;
}

function shelfTitle(selection: YearSelection): string {
  if (selection.year === null) return selection.basis === "original" ? "MUSIC WITHOUT AN ORIGINAL YEAR" : "EDITIONS WITHOUT A RELEASE YEAR";
  return selection.basis === "original" ? `MUSIC ORIGINALLY FROM ${selection.year}` : `${selection.year} EDITIONS`;
}

function yearX(year: number, firstYear: number, lastYear: number): number {
  if (firstYear === lastYear) return (PLOT_LEFT + PLOT_RIGHT) / 2;
  return PLOT_LEFT + ((year - firstYear) / (lastYear - firstYear)) * (PLOT_RIGHT - PLOT_LEFT);
}

function activateKey(event: KeyboardEvent<SVGGElement>, action: () => void) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    action();
  }
}

function yearTicks(firstYear: number, lastYear: number): number[] {
  const ticks: number[] = [firstYear];
  const firstDecade = Math.ceil(firstYear / 10) * 10;
  for (let year = firstDecade; year <= lastYear; year += 10) ticks.push(year);
  if (ticks[ticks.length - 1] !== lastYear) ticks.push(lastYear);
  return [...new Set(ticks)];
}

interface TimelineMarksProps {
  buckets: readonly YearBucket[];
  basis: YearBasis;
  baseline: number;
  direction: -1 | 1;
  firstYear: number;
  lastYear: number;
  selected: YearSelection;
  onSelect: (selection: YearSelection) => void;
}

const TimelineMarks = memo(function TimelineMarks({ buckets, basis, baseline, direction, firstYear, lastYear, selected, onSelect }: TimelineMarksProps) {
  const maximum = Math.max(1, ...buckets.map((bucket) => bucket.albumCount));
  const tone = basis === "original" ? "years-clock__bar--original" : "years-clock__bar--release";
  return <g className={`years-clock__marks years-clock__marks--${basis}`}>
    {buckets.map((bucket) => {
      const height = Math.max(2, Math.sqrt(bucket.albumCount / maximum) * 73);
      const x = yearX(bucket.year, firstYear, lastYear);
      const selectedHere = selected.basis === basis && selected.year === bucket.year;
      return <g
        role="button"
        tabIndex={0}
        aria-label={`${basis === "original" ? "Original" : "Release"} Year ${bucket.year}: ${formatCount(bucket.albumCount)} albums and ${formatCount(bucket.trackCount)} tracks`}
        aria-pressed={selectedHere}
        className={selectedHere ? "is-selected" : undefined}
        onClick={() => onSelect({ basis, year: bucket.year })}
        onKeyDown={(event) => activateKey(event, () => onSelect({ basis, year: bucket.year }))}
        key={bucket.year}
      >
        <title>{bucket.year} · {formatCount(bucket.albumCount)} albums · {formatCount(bucket.trackCount)} tracks</title>
        <rect className={`${tone} years-clock__hit`} x={x - 5} y={Math.min(baseline, baseline + direction * height) - 5} width="10" height={height + 10} />
        <rect className={tone} x={x - 1.55} y={direction < 0 ? baseline - height : baseline} width="3.1" height={height} rx="1.5" />
        {selectedHere ? <><circle className={`years-clock__selected years-clock__selected--${basis}`} cx={x} cy={baseline} r="10" /><circle className="years-clock__selected-core" cx={x} cy={baseline} r="4" /></> : null}
      </g>;
    })}
  </g>;
});

function ClockLabel({ basis, y, active, firstYear, lastYear }: { basis: YearBasis; y: number; active: boolean; firstYear: number; lastYear: number }) {
  return <g className={`years-clock__label years-clock__label--${basis}${active ? " is-active" : ""}`}>
    <text x="0" y={y}>{basis === "original" ? "ORIGINAL YEAR" : "RELEASE YEAR"}</text>
    <text className="years-clock__range" x="0" y={y + 18}>{firstYear}–{lastYear}</text>
  </g>;
}

function TwoClockChart({ overview, detail, onSelect }: { overview: YearOverview; detail: YearDetail; onSelect: (selection: YearSelection) => void }) {
  const allYears = [...overview.originalYears, ...overview.releaseYears].map((bucket) => bucket.year);
  const firstYear = Math.min(...allYears);
  const lastYear = Math.max(...allYears);
  const ticks = yearTicks(firstYear, lastYear);
  const selectedYear = detail.selection.year;
  const selectedX = selectedYear === null ? null : yearX(selectedYear, firstYear, lastYear);
  const maximumFlow = Math.max(1, ...detail.flows.map((flow) => flow.albumCount));
  return <svg className="years-clock" viewBox={`0 0 ${VIEW_WIDTH} 405`} role="img" aria-labelledby="years-clock-title years-clock-description">
    <title id="years-clock-title">Original and Release Year clocks</title>
    <desc id="years-clock-description">Select any year on either timeline. Curves aggregate albums connecting the active year to the other date field.</desc>
    <defs>
      <linearGradient id="years-flow-original" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#32d9ef" /><stop offset="1" stopColor="#9c54e8" /></linearGradient>
      <linearGradient id="years-flow-release" x1="0" y1="1" x2="0" y2="0"><stop offset="0" stopColor="#d45cff" /><stop offset="1" stopColor="#4c9fe6" /></linearGradient>
    </defs>
    <ClockLabel basis="original" y={24} active={detail.selection.basis === "original"} firstYear={firstYear} lastYear={lastYear} />
    <ClockLabel basis="release" y={278} active={detail.selection.basis === "release"} firstYear={firstYear} lastYear={lastYear} />
    <line className="years-clock__axis" x1={PLOT_LEFT} x2={PLOT_RIGHT} y1={TOP_BASELINE} y2={TOP_BASELINE} />
    <line className="years-clock__axis" x1={PLOT_LEFT} x2={PLOT_RIGHT} y1={BOTTOM_BASELINE} y2={BOTTOM_BASELINE} />
    {ticks.map((year) => <g className="years-clock__tick" key={`top-${year}`}><line x1={yearX(year, firstYear, lastYear)} x2={yearX(year, firstYear, lastYear)} y1={TOP_BASELINE} y2={TOP_BASELINE + 5} /><text x={yearX(year, firstYear, lastYear)} y={TOP_BASELINE + 18}>{year}</text></g>)}
    {ticks.map((year) => <g className="years-clock__tick" key={`bottom-${year}`}><line x1={yearX(year, firstYear, lastYear)} x2={yearX(year, firstYear, lastYear)} y1={BOTTOM_BASELINE} y2={BOTTOM_BASELINE + 5} /><text x={yearX(year, firstYear, lastYear)} y={BOTTOM_BASELINE + 18}>{year}</text></g>)}
    <g className="years-clock__flows" aria-hidden="true">
      {selectedX === null ? null : detail.flows.filter((flow) => flow.year !== null).map((flow) => {
        const counterpartX = yearX(flow.year!, firstYear, lastYear);
        const fromX = detail.selection.basis === "original" ? selectedX : counterpartX;
        const toX = detail.selection.basis === "original" ? counterpartX : selectedX;
        const width = Math.max(.8, Math.sqrt(flow.albumCount / maximumFlow) * 14);
        return <path
          d={`M ${fromX.toFixed(2)} ${TOP_BASELINE} C ${fromX.toFixed(2)} 205, ${toX.toFixed(2)} 260, ${toX.toFixed(2)} ${BOTTOM_BASELINE}`}
          stroke={`url(#years-flow-${detail.selection.basis})`}
          strokeWidth={width}
          key={`${flow.year}:${flow.albumCount}`}
        />;
      })}
    </g>
    <TimelineMarks buckets={overview.originalYears} basis="original" baseline={TOP_BASELINE} direction={-1} firstYear={firstYear} lastYear={lastYear} selected={detail.selection} onSelect={onSelect} />
    <TimelineMarks buckets={overview.releaseYears} basis="release" baseline={BOTTOM_BASELINE} direction={-1} firstYear={firstYear} lastYear={lastYear} selected={detail.selection} onSelect={onSelect} />
    {selectedYear !== null ? <text className={`years-clock__selection-label years-clock__selection-label--${detail.selection.basis}`} x={selectedX ?? 0} y={detail.selection.basis === "original" ? TOP_BASELINE - 60 : BOTTOM_BASELINE + 27}>{selectedYear}</text> : null}
  </svg>;
}

function SingleClockChart({ buckets, selection, onSelect }: { buckets: readonly YearBucket[]; selection: YearSelection; onSelect: (selection: YearSelection) => void }) {
  const firstYear = Math.min(...buckets.map((bucket) => bucket.year));
  const lastYear = Math.max(...buckets.map((bucket) => bucket.year));
  const ticks = yearTicks(firstYear, lastYear);
  return <svg className={`years-clock years-clock--single years-clock--${selection.basis}`} viewBox={`0 0 ${VIEW_WIDTH} 238`} role="img" aria-label={`${selection.basis === "original" ? "Original" : "Release"} Year landscape`}>
    <ClockLabel basis={selection.basis} y={27} active firstYear={firstYear} lastYear={lastYear} />
    <line className="years-clock__axis" x1={PLOT_LEFT} x2={PLOT_RIGHT} y1="180" y2="180" />
    {ticks.map((year) => <g className="years-clock__tick" key={year}><line x1={yearX(year, firstYear, lastYear)} x2={yearX(year, firstYear, lastYear)} y1="180" y2="185" /><text x={yearX(year, firstYear, lastYear)} y="200">{year}</text></g>)}
    <TimelineMarks buckets={buckets} basis={selection.basis} baseline={180} direction={-1} firstYear={firstYear} lastYear={lastYear} selected={selection} onSelect={onSelect} />
  </svg>;
}

interface EditionGroup {
  key: string;
  order: number;
  label: string;
  albums: YearAlbum[];
}

function groupAlbums(detail: YearDetail): EditionGroup[] {
  const groups = new Map<string, EditionGroup>();
  for (const album of detail.albums) {
    const counterpart = detail.selection.basis === "original" ? album.releaseYear : album.originalYear;
    const sameYear = counterpart !== null && counterpart === detail.selection.year;
    const decade = counterpart === null ? null : Math.floor(counterpart / 10) * 10;
    const key = sameYear ? "same" : counterpart === null ? "missing" : String(decade);
    const label = sameYear
      ? detail.selection.basis === "original" ? "Original editions" : "Same-year originals"
      : counterpart === null
        ? detail.selection.basis === "original" ? "Release year unknown" : "Original year unknown"
        : detail.selection.basis === "original" ? `${decade}s editions` : `${decade}s originals`;
    const order = sameYear ? -10_000 : counterpart === null ? 10_000 : decade!;
    const group = groups.get(key) ?? { key, order, label, albums: [] };
    group.albums.push(album);
    groups.set(key, group);
  }
  return [...groups.values()].sort((left, right) => left.order - right.order);
}

function EditionShelf({ detail, selectedAlbumId, onSelectAlbum }: Pick<YearsExplorerProps, "detail" | "selectedAlbumId" | "onSelectAlbum"> & { detail: YearDetail }) {
  const groups = useMemo(() => groupAlbums(detail), [detail]);
  return <section className="years-editions" aria-labelledby="years-editions-title">
    <header>
      <div className="years-editions__title"><h2 id="years-editions-title">{shelfTitle(detail.selection)}</h2><span>{formatCount(detail.summary.albumCount)} albums</span></div>
      <dl>
        <div><dt>Tracks</dt><dd>{formatCount(detail.summary.trackCount)}</dd></div>
        <div><dt>Loved</dt><dd>{formatCount(detail.summary.lovedTracks)} <Heart aria-hidden="true" /></dd></div>
        <div><dt>Rated</dt><dd>{detail.summary.trackCount ? Math.round((detail.summary.ratedTracks / detail.summary.trackCount) * 100) : 0}%</dd></div>
        <div><dt>Other clock</dt><dd>{formatCount(detail.flows.filter((flow) => flow.year !== null).length)} years</dd></div>
      </dl>
    </header>
    {groups.length ? <div className="years-edition-groups">
      {groups.map((group) => <section className="years-edition-group" aria-label={group.label} key={group.key}>
        <header><strong>{group.label}</strong><span>{formatCount(group.albums.length)} shown</span></header>
        <div>
          {group.albums.slice(0, 4).map((album) => <button
            type="button"
            className={selectedAlbumId === album.id ? "is-selected" : undefined}
            aria-pressed={selectedAlbumId === album.id}
            onClick={() => onSelectAlbum(album)}
            key={album.id}
          >
            <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
            <span><strong>{album.title}</strong><small>{album.artist}</small><em>{album.publisher ?? "Publisher unknown"} · Original {album.originalYear ?? "—"} · Release {album.releaseYear ?? "—"}</em></span>
          </button>)}
        </div>
      </section>)}
    </div> : <div className="years-empty"><Disc3 aria-hidden="true" /><strong>No representative albums were found.</strong><span>The selected clock still remains distinct in Music Library.</span></div>}
  </section>;
}

function Feedback({ state, error, detail, onRetry }: { state: YearsLoadState; error: string | null; detail: boolean; onRetry: () => void }) {
  if (state === "loading") return <div className="years-feedback" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" /><strong>{detail ? "Aligning the two clocks…" : "Reading your musical timeline…"}</strong><span>{detail ? "Aggregating only the selected year." : "One album-level pass; no track inventory enters the interface."}</span></div>;
  return <div className="years-feedback years-feedback--error" role="alert"><CalendarRange aria-hidden="true" /><strong>{detail ? "This year could not be opened." : "The timeline is temporarily unavailable."}</strong><span>{error ?? "Aurora could not read this bounded view."}</span><button type="button" onClick={onRetry}><RefreshCw aria-hidden="true" /> Try again</button></div>;
}

export function YearsExplorer(props: YearsExplorerProps) {
  const [mode, setMode] = useState<YearsMode>("twoClocks");
  const overview = props.overview;
  const detail = props.detail;

  function changeMode(nextMode: YearsMode) {
    setMode(nextMode);
    if (!overview || !detail || nextMode === "twoClocks") return;
    const basis: YearBasis = nextMode;
    if (detail.selection.basis === basis) return;
    const buckets = basis === "original" ? overview.originalYears : overview.releaseYears;
    const sameYear = detail.selection.year !== null && buckets.some((bucket) => bucket.year === detail.selection.year);
    props.onSelect({ basis, year: sameYear ? detail.selection.year : buckets[buckets.length - 1]?.year ?? null });
  }

  if (props.loadState !== "ready" || !overview) {
    return <section className="years-explorer"><Feedback state={props.loadState} error={props.errorMessage} detail={false} onRetry={props.onRetry} /></section>;
  }
  if (!detail || props.detailState !== "ready") {
    return <section className="years-explorer"><YearsToolbar mode={mode} stats={overview.stats} onModeChange={changeMode} /><Feedback state={props.detailState} error={props.detailError} detail onRetry={props.onRetryDetail} /></section>;
  }
  const selection = detail.selection;
  return <section className="years-explorer" aria-labelledby="years-page-title">
    <h1 className="sr-only" id="years-page-title">Years</h1>
    <YearsToolbar mode={mode} stats={overview.stats} onModeChange={changeMode} />
    <p className="years-clock-hint"><Clock3 aria-hidden="true" /> Select a year on either clock to make it the lens.</p>
    <div className="years-clock-stage" data-mode={mode}>
      {mode === "twoClocks"
        ? <TwoClockChart overview={overview} detail={detail} onSelect={props.onSelect} />
        : <SingleClockChart buckets={mode === "original" ? overview.originalYears : overview.releaseYears} selection={selection} onSelect={props.onSelect} />}
      <div className="years-clock-legend" aria-hidden="true"><span className="is-strong">Many {selection.basis === "original" ? "originals" : "releases"}</span><span>Fewer {selection.basis === "original" ? "originals" : "releases"}</span></div>
      <div className="sr-only" aria-live="polite">{selectionCopy(selection)}: {formatCount(detail.summary.albumCount)} albums and {formatCount(detail.summary.trackCount)} tracks</div>
      <button type="button" className="years-clock-step years-clock-step--previous" aria-label={`Previous ${selection.basis} year`} onClick={() => {
        const buckets = selection.basis === "original" ? overview.originalYears : overview.releaseYears;
        const index = buckets.findIndex((bucket) => bucket.year === selection.year);
        const previous = buckets[Math.max(0, index - 1)];
        if (previous) props.onSelect({ basis: selection.basis, year: previous.year });
      }}><ChevronLeft aria-hidden="true" /></button>
      <button type="button" className="years-clock-step years-clock-step--next" aria-label={`Next ${selection.basis} year`} onClick={() => {
        const buckets = selection.basis === "original" ? overview.originalYears : overview.releaseYears;
        const index = buckets.findIndex((bucket) => bucket.year === selection.year);
        const next = buckets[Math.min(buckets.length - 1, Math.max(0, index + 1))];
        if (next) props.onSelect({ basis: selection.basis, year: next.year });
      }}><ChevronRight aria-hidden="true" /></button>
    </div>
    <div className="years-missing">
      <button type="button" aria-pressed={selection.basis === "original" && selection.year === null} onClick={() => props.onSelect({ basis: "original", year: null })}>Missing Original Year <span>{formatCount(overview.stats.missingOriginalAlbums)}</span></button>
      <button type="button" aria-pressed={selection.basis === "release" && selection.year === null} onClick={() => props.onSelect({ basis: "release", year: null })}>Missing Release Year <span>{formatCount(overview.stats.missingReleaseAlbums)}</span></button>
    </div>
    <EditionShelf detail={detail} selectedAlbumId={props.selectedAlbumId} onSelectAlbum={props.onSelectAlbum} />
    <div className="years-actions">
      <button type="button" className="button button--primary" onClick={() => props.onExplore(selection)}><Sparkles aria-hidden="true" /> {selection.year === null ? "Explore missing years" : `Explore ${selection.basis} ${selection.year}`}</button>
      <button type="button" className="button button--quiet" disabled={props.queueBusy || detail.summary.trackCount === 0} onClick={() => props.onPlayYear(selection)}>{props.queueBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} {selection.basis === "original" ? "Play this original year" : "Play this release year"}</button>
      <button type="button" className="years-actions__more" aria-label="Open selected album" disabled={!props.selectedAlbumId} onClick={() => {
        const album = detail.albums.find((candidate) => candidate.id === props.selectedAlbumId);
        if (album) props.onPlayAlbum(album);
      }}><ListMusic aria-hidden="true" /></button>
      {props.queueMessage ? <span role="status">{props.queueMessage}</span> : null}
    </div>
  </section>;
}

function YearsToolbar({ mode, stats, onModeChange }: { mode: YearsMode; stats: YearOverview["stats"]; onModeChange: (mode: YearsMode) => void }) {
  const modes: ReadonlyArray<{ id: YearsMode; label: string; icon: typeof Clock3 }> = [
    { id: "release", label: "Release landscape", icon: CalendarRange },
    { id: "original", label: "Original landscape", icon: Clock3 },
    { id: "twoClocks", label: "Two clocks", icon: Layers3 },
  ];
  return <header className="years-toolbar">
    <div className="years-mode" role="tablist" aria-label="Years view">
      {modes.map(({ id, label, icon: Icon }) => <button type="button" role="tab" aria-selected={mode === id} onClick={() => onModeChange(id)} key={id}><Icon aria-hidden="true" /> {label}</button>)}
    </div>
    <div className="years-scope"><strong>{formatCount(stats.differentTracks)} tracks</strong><span>·</span><strong>{formatCount(stats.differentAlbums)} albums with different dates</strong><Star aria-hidden="true" /></div>
  </header>;
}

export function YearAlbumInspector<T extends AlbumInspectorAlbum>({ album, busy, onPlay, onOpenArtistAlbums, chartRanks }: { album: T; busy: boolean; onPlay: (album: T) => void; onOpenArtistAlbums: (artist: string) => void; chartRanks?: readonly CatalogChartRank[] }) {
  return <div className="year-album-inspector">
    <Artwork track={albumAsTrack(album)} size="large" decorative={false} />
    <div className="year-album-inspector__heading"><div><h2>{album.title}</h2><p><CountryFlag code={album.originCountryCode} name={album.originCountryName} /><ArtistSmartLink artist={album.artist} onOpen={onOpenArtistAlbums} /></p></div>{album.lovedTracks > 0 ? <span><Heart aria-hidden="true" /> {formatCount(album.lovedTracks)}</span> : null}</div>
    <dl className="metadata-list">
      <div><dt>Original Year</dt><dd className="year-original">{album.originalYear ?? "—"}</dd></div>
      <div><dt>Release Year</dt><dd className="year-release">{album.releaseYear ?? "—"}</dd></div>
      <div className="publisher-metadata"><dt>Publisher</dt><dd>{album.publisher ?? "Unknown"}</dd></div>
      <div><dt>Format</dt><dd>{album.formats?.length ? album.formats.join(" · ") : "Unknown"}</dd></div>
      <div><dt>Tracks</dt><dd>{formatCount(album.totalTracks)}</dd></div>
      <div><dt>Duration</dt><dd>{formatDuration(album.durationSeconds)}</dd></div>
      {chartRanks?.length ? <div><dt>Charts</dt><dd><CatalogChartRanks kind="album" ranks={chartRanks} /></dd></div> : null}
      <div><dt>Rating</dt><dd>{album.rating === null ? "—" : <><Star aria-hidden="true" /> {album.rating.toFixed(1)}</>}</dd></div>
      <div><dt>Genre</dt><dd>{album.genre ?? "Unknown"}</dd></div>
    </dl>
    <button type="button" className="button button--primary year-album-inspector__play" disabled={busy} onClick={() => onPlay(album)}>{busy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play album</button>
    <p><Disc3 aria-hidden="true" /> Both clocks come from the read-only Music Library catalog. Aurora never substitutes one for the other.</p>
  </div>;
}
