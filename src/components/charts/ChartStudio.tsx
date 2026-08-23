import {
  ArrowDown,
  ArrowUp,
  CalendarRange,
  ChartColumn,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Disc3,
  Heart,
  Info,
  Library,
  LoaderCircle,
  Minus,
  Play,
  RefreshCw,
  Star,
  Trophy,
  X,
} from "lucide-react";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import {
  chartPresets,
  loadChartEntryTrack,
  loadChartItemDetail,
  loadChartPage,
  loadChartQueue,
  type AlbumScoreEntry,
  type ChartEntry,
  type ChartItemDetail,
  type ChartKind,
  type ChartPage,
  type ChartPageRequest,
  type ChartPeriod,
  type ChartScope,
  type ChartSource,
  type ChartYearBasis,
} from "../../charts";
import type { Track } from "../../library";
import type { LoveState } from "../../tags";
import { Artwork } from "../Artwork";
import { InlineLoveControl, InlineRatingControl } from "../InlineTagControls";
import "./ChartStudio.css";

export type ChartLoadState = "loading" | "ready" | "error";

export interface ChartSelectionContext {
  kind: ChartKind;
  entry: ChartEntry;
  detail: ChartItemDetail | null;
  pageRequest: ChartPageRequest;
  chartTitle: string;
}

interface ChartStudioProps {
  onSelectionChange: (selection: ChartSelectionContext | null) => void;
  onSelectTrack: (track: Track) => void;
  onPlayQueue: (tracks: Track[]) => Promise<boolean>;
}

const sourceOptions: Record<ChartKind, ReadonlyArray<{ source: ChartSource; label: string; annual?: boolean }>> = {
  singles: [
    { source: "officialUk", label: "Official UK" },
    { source: "vgLista", label: "VG Lista" },
    { source: "tiISkuddet", label: "Ti i Skuddet" },
    { source: "norsktoppen", label: "Norsktoppen" },
    { source: "billboard", label: "Billboard", annual: true },
  ],
  albums: [
    { source: "auroraScore", label: "Aurora Score", annual: true },
    { source: "officialUk", label: "Official UK" },
    { source: "vgLista", label: "VG Lista" },
    { source: "billboard", label: "Billboard", annual: true },
  ],
};

const initialRequest: ChartPageRequest = {
  kind: "singles",
  source: "officialUk",
  scope: "week",
  period: chartPresets[0],
  selectedYear: 1985,
  selectedWeek: 23,
  yearBasis: "year",
  limit: 100,
};

function formatCount(value: number): string {
  return new Intl.NumberFormat().format(value);
}

function formatDate(value: string | null): string {
  if (!value) return "Date unavailable";
  const date = new Date(`${value}T12:00:00`);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(date);
}

function entryAsTrack(entry: ChartEntry): Track {
  return {
    id: entry.matchedTrackId ?? `chart-entry:${entry.artistKey}:${entry.titleKey}`,
    trackKey: `chart-entry:${entry.artistKey}:${entry.titleKey}`,
    albumId: entry.artworkAlbumId,
    title: entry.title,
    artist: entry.artist,
    album: entry.title,
    releaseYear: null,
    rating: entry.rating,
    loved: entry.loved,
    loveState: entry.loved ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: null,
    genre: null,
    playCount: null,
  };
}

function scoreAsTrack(album: AlbumScoreEntry): Track {
  return {
    id: `chart-score:${album.id}`,
    trackKey: `chart-score:${album.id}`,
    albumId: album.id,
    title: album.title,
    artist: album.artist,
    album: album.title,
    originalYear: album.originalYear,
    releaseYear: album.releaseYear,
    rating: null,
    loved: false,
    loveState: "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: null,
    genre: null,
    playCount: null,
  };
}

function movementLabel(entry: ChartEntry) {
  if (entry.movement === null || entry.movement === 0) return <span className="chart-movement is-static"><Minus aria-hidden="true" /> —</span>;
  if (entry.movement > 0) return <span className="chart-movement is-up"><ArrowUp aria-hidden="true" /> {entry.movement}</span>;
  return <span className="chart-movement is-down"><ArrowDown aria-hidden="true" /> {Math.abs(entry.movement)}</span>;
}

function CustomPeriodDialog({ initial, onClose, onApply }: { initial: ChartPeriod; onClose: () => void; onApply: (period: ChartPeriod) => void }) {
  const [draft, setDraft] = useState(initial);
  function submit(event: FormEvent) {
    event.preventDefault();
    const fromKey = draft.fromYear * 100 + draft.fromWeek;
    const toKey = draft.toYear * 100 + draft.toWeek;
    if (fromKey > toKey) return;
    onApply({ ...draft, label: draft.label.trim() || `${draft.fromYear} W${draft.fromWeek}–${draft.toYear} W${draft.toWeek}` });
  }
  return <div className="chart-dialog-backdrop" role="presentation" onPointerDown={(event) => { if (event.currentTarget === event.target) onClose(); }}>
    <form className="chart-dialog" role="dialog" aria-modal="true" aria-labelledby="chart-dialog-title" onSubmit={submit}>
      <header><div><p className="eyebrow">Custom period</p><h2 id="chart-dialog-title">Build a chart window</h2></div><button type="button" aria-label="Close custom period" onClick={onClose}><X aria-hidden="true" /></button></header>
      <label>Label<input value={draft.label} maxLength={80} onChange={(event) => setDraft((current) => ({ ...current, label: event.target.value }))} /></label>
      <div className="chart-dialog__range">
        <fieldset><legend>From</legend><label>Year<input type="number" min="1890" max="2200" value={draft.fromYear} onChange={(event) => setDraft((current) => ({ ...current, fromYear: Number(event.target.value) }))} /></label><label>Week<input type="number" min="1" max="53" value={draft.fromWeek} onChange={(event) => setDraft((current) => ({ ...current, fromWeek: Number(event.target.value) }))} /></label></fieldset>
        <fieldset><legend>To</legend><label>Year<input type="number" min="1890" max="2200" value={draft.toYear} onChange={(event) => setDraft((current) => ({ ...current, toYear: Number(event.target.value) }))} /></label><label>Week<input type="number" min="1" max="53" value={draft.toWeek} onChange={(event) => setDraft((current) => ({ ...current, toWeek: Number(event.target.value) }))} /></label></fieldset>
      </div>
      <p><Info aria-hidden="true" /> Period charts compare weeks at #1 first, then #2, #3 and onward. Total position points break the final tie.</p>
      <footer><button type="button" className="button button--quiet" onClick={onClose}>Cancel</button><button type="submit" className="button button--primary"><CalendarRange aria-hidden="true" /> Apply period</button></footer>
    </form>
  </div>;
}

function Feedback({ state, error, onRetry }: { state: ChartLoadState; error: string | null; onRetry: () => void }) {
  return <div className={`chart-feedback${state === "error" ? " is-error" : ""}`} role={state === "error" ? "alert" : "status"}>
    {state === "error" ? <Disc3 aria-hidden="true" /> : <LoaderCircle className="is-spinning" aria-hidden="true" />}
    <strong>{state === "error" ? error : "Opening the chart archive…"}</strong>
    {state === "error" ? <button type="button" onClick={onRetry}><RefreshCw aria-hidden="true" /> Try again</button> : null}
  </div>;
}

export function ChartStudio({ onSelectionChange, onSelectTrack, onPlayQueue }: ChartStudioProps) {
  const [request, setRequest] = useState(initialRequest);
  const [page, setPage] = useState<ChartPage | null>(null);
  const [loadState, setLoadState] = useState<ChartLoadState>("loading");
  const [error, setError] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<ChartEntry | null>(null);
  const [detail, setDetail] = useState<ChartItemDetail | null>(null);
  const [queueBusy, setQueueBusy] = useState(false);
  const [queueMessage, setQueueMessage] = useState<string | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const requestIdRef = useRef(0);
  const selectionIdRef = useRef(0);
  const callbacksRef = useRef({ onSelectionChange, onSelectTrack, onPlayQueue });
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    callbacksRef.current = { onSelectionChange, onSelectTrack, onPlayQueue };
  }, [onPlayQueue, onSelectTrack, onSelectionChange]);

  const selectEntry = useCallback((entry: ChartEntry, nextPage: ChartPage) => {
    const selectionId = ++selectionIdRef.current;
    setSelectedEntry(entry);
    setDetail(null);
    const context: ChartSelectionContext = { kind: nextPage.request.kind, entry, detail: null, pageRequest: nextPage.request, chartTitle: nextPage.chartTitle };
    callbacksRef.current.onSelectionChange(context);
    const detailRequest = loadChartItemDetail({ page: nextPage.request, artistKey: entry.artistKey, titleKey: entry.titleKey });
    const trackRequest = nextPage.request.kind === "singles" && entry.matchedTrackId
      ? loadChartEntryTrack(entry.matchedTrackId)
      : Promise.resolve(null);
    void Promise.allSettled([detailRequest, trackRequest]).then(([nextDetail, nextTrack]) => {
      if (selectionId !== selectionIdRef.current) return;
      const value = nextDetail.status === "fulfilled" ? nextDetail.value : null;
      setDetail(value);
      callbacksRef.current.onSelectionChange({ ...context, detail: value });
      if (nextTrack.status === "fulfilled" && nextTrack.value) callbacksRef.current.onSelectTrack(nextTrack.value);
    });
  }, []);

  useEffect(() => {
    const loadId = ++requestIdRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setLoadState("loading");
      setError(null);
      setQueueMessage(null);
      void loadChartPage(request)
        .then((nextPage) => {
          if (cancelled || loadId !== requestIdRef.current) return;
          setPage(nextPage);
          setLoadState("ready");
          const nextSelection = nextPage.entries.find((entry) => entry.loved) ?? nextPage.entries[0] ?? null;
          if (nextSelection) selectEntry(nextSelection, nextPage);
          else {
            setSelectedEntry(null);
            setDetail(null);
            callbacksRef.current.onSelectionChange(null);
          }
        })
        .catch((cause: unknown) => {
          if (cancelled || loadId !== requestIdRef.current) return;
          setError(cause instanceof Error ? cause.message : String(cause));
          setLoadState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [reloadToken, request, selectEntry]);

  const visibleWeeks = (() => {
    if (!page?.weeks.length) return [];
    const activeIndex = page.weeks.findIndex((week) => week.year === request.selectedYear && week.week === request.selectedWeek);
    const start = Math.max(0, Math.min(page.weeks.length - 13, activeIndex - 6));
    return page.weeks.slice(start, start + 13);
  })();

  function applyPeriod(period: ChartPeriod) {
    setCustomOpen(false);
    setRequest((current) => ({ ...current, period, selectedYear: period.fromYear, selectedWeek: period.fromWeek, scope: "week" }));
  }

  function changeKind(kind: ChartKind) {
    setRequest((current) => ({ ...current, kind, source: kind === "albums" ? "auroraScore" : "officialUk", scope: kind === "albums" ? "period" : "week" }));
  }

  function changeSource(source: ChartSource) {
    const annual = source === "billboard" || source === "auroraScore";
    setRequest((current) => ({ ...current, source, scope: annual ? "period" : current.scope }));
  }

  function changeScope(scope: ChartScope) {
    if (page?.annualOnly) return;
    setRequest((current) => ({ ...current, scope }));
  }

  function changeYearBasis(yearBasis: ChartYearBasis) {
    setRequest((current) => ({ ...current, yearBasis }));
  }

  async function playChart() {
    if (queueBusy) return;
    setQueueBusy(true);
    setQueueMessage(null);
    try {
      const tracks = await loadChartQueue(page?.request ?? request);
      if (!tracks.length) {
        setQueueMessage("This chart has no entries matched to playable library files yet.");
        return;
      }
      if (await callbacksRef.current.onPlayQueue(tracks)) setQueueMessage(`Loaded ${formatCount(tracks.length)} matched tracks from this chart.`);
    } catch (cause) {
      setQueueMessage(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setQueueBusy(false);
    }
  }

  return <section className="chart-studio">
    <header className="chart-studio__header">
      <div><h1>Charts <span>The music record</span></h1></div>
      <div className="chart-kind" role="tablist" aria-label="Chart type">
        <button type="button" role="tab" aria-selected={request.kind === "singles"} onClick={() => changeKind("singles")}>Singles</button>
        <button type="button" role="tab" aria-selected={request.kind === "albums"} onClick={() => changeKind("albums")}>Albums</button>
      </div>
    </header>

    <div className="chart-presets" aria-label="Chart period presets">
      {chartPresets.map((preset, index) => <button type="button" className={request.period.label === preset.label ? "is-active" : undefined} onClick={() => applyPeriod(preset)} key={preset.label}>
        {index === 0 ? <Trophy aria-hidden="true" /> : <CalendarRange aria-hidden="true" />}<span>{preset.label}</span>
      </button>)}
      <button type="button" className={!chartPresets.some((preset) => preset.label === request.period.label) ? "is-active" : undefined} onClick={() => setCustomOpen(true)}><CalendarRange aria-hidden="true" /><span>{chartPresets.some((preset) => preset.label === request.period.label) ? "Custom" : request.period.label}</span></button>
    </div>

    <section className="chart-calendar" aria-label={`${request.period.label} chart calendar`}>
      <button type="button" aria-label="Previous period" onClick={() => setRequest((current) => ({ ...current, period: { ...current.period, fromYear: current.period.fromYear - 1, toYear: current.period.toYear - 1, label: current.period.label.replace(/\d{4}/g, (year) => String(Number(year) - 1)) }, selectedYear: current.selectedYear - 1 }))}><ChevronLeft aria-hidden="true" /></button>
      <button
        type="button"
        className="chart-calendar__year"
        aria-label={`${request.selectedYear} full year`}
        title={`Build the ${request.selectedYear} end-of-year chart`}
        onClick={() => setRequest((current) => ({
          ...current,
          period: { fromYear: current.selectedYear, fromWeek: 1, toYear: current.selectedYear, toWeek: 53, label: `${current.selectedYear} year chart` },
          selectedWeek: 1,
          scope: "period",
        }))}
      ><strong>{request.period.fromYear === request.period.toYear ? request.period.fromYear : `${request.period.fromYear}–${request.period.toYear}`}</strong><small>full year</small></button>
      <div className="chart-calendar__weeks">
        {visibleWeeks.map((week) => <button type="button" aria-pressed={request.selectedYear === week.year && request.selectedWeek === week.week} onClick={() => setRequest((current) => ({ ...current, selectedYear: week.year, selectedWeek: week.week, scope: "week" }))} key={`${week.year}:${week.week}`}><span>{week.week}</span><small>Wk</small></button>)}
        {!visibleWeeks.length ? <span className="chart-calendar__annual">Annual view</span> : null}
      </div>
      <button type="button" aria-label="Next period" onClick={() => setRequest((current) => ({ ...current, period: { ...current.period, fromYear: current.period.fromYear + 1, toYear: current.period.toYear + 1, label: current.period.label.replace(/\d{4}/g, (year) => String(Number(year) + 1)) }, selectedYear: current.selectedYear + 1 }))}><ChevronRight aria-hidden="true" /></button>
    </section>

    <div className="chart-source-row">
      <div className="chart-sources" role="tablist" aria-label={`${request.kind} chart source`}>
        {sourceOptions[request.kind].map(({ source, label, annual }) => <button type="button" role="tab" aria-selected={request.source === source} className={annual ? "is-annual" : undefined} onClick={() => changeSource(source)} key={source}><ChartColumn aria-hidden="true" /> {label}{annual ? <small>annual</small> : null}</button>)}
      </div>
      <div className="chart-scope" role="tablist" aria-label="Chart calculation">
        <button type="button" role="tab" aria-selected={page?.request.scope === "week"} disabled={page?.annualOnly} onClick={() => changeScope("week")}>Selected week</button>
        <button type="button" role="tab" aria-selected={page?.request.scope === "period"} onClick={() => changeScope("period")}>Period chart</button>
      </div>
    </div>

    {loadState !== "ready" || !page ? <Feedback state={loadState} error={error} onRetry={() => setReloadToken((value) => value + 1)} /> : <>
      <section className="chart-ranking" aria-labelledby="chart-ranking-heading">
        <header>
          <div><span className="chart-ranking__source"><ChartColumn aria-hidden="true" /></span><div><h2 id="chart-ranking-heading">{page.chartTitle}</h2><p>{page.request.source === "auroraScore" ? `${page.request.period.label} · ranked by Album Score using ${page.request.yearBasis === "year" ? "Year" : "Release Year"}` : page.request.scope === "week" ? `Week ${page.request.selectedWeek} · ${formatDate(page.chartDate)}` : `${page.request.period.label} · ranked by position finishes`}</p></div></div>
          <button type="button" className="button button--primary" disabled={queueBusy || !page.entries.length} onClick={() => void playChart()}>{queueBusy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play this chart</button>
        </header>
        <div className="chart-table" role="table" aria-label={page.chartTitle}>
          <div className="chart-table__head" role="row"><span>#</span><span>Title</span><span>Move</span><span>{page.request.scope === "week" ? "LW" : "#1"}</span><span>Peak</span><span>{page.request.scope === "week" ? "Wks" : "Points"}</span><span>Library</span></div>
          {page.entries.slice(0, 20).map((entry) => {
            const selected = selectedEntry?.artistKey === entry.artistKey && selectedEntry.titleKey === entry.titleKey;
            return <div className={`chart-row${entry.position <= 3 ? " is-podium" : ""}${selected ? " is-selected" : ""}`} role="row" tabIndex={0} aria-selected={selected} onClick={() => selectEntry(entry, page)} onDoubleClick={() => entry.matchedTrackId && void loadChartEntryTrack(entry.matchedTrackId).then((track) => callbacksRef.current.onPlayQueue([track]))} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); selectEntry(entry, page); } }} key={`${entry.artistKey}:${entry.titleKey}`}>
              <strong className="chart-row__rank">{entry.position}</strong>
              <div className="chart-row__identity"><Artwork track={entryAsTrack(entry)} decorative={false} /><span><strong>{entry.title}</strong><small>{entry.artist}</small></span></div>
              {movementLabel(entry)}
              <span>{page.request.scope === "week" ? entry.previousPosition ?? "—" : entry.weeksAtNumberOne}</span>
              <span>{entry.peakPosition ?? "—"}</span>
              <span>{formatCount(page.request.scope === "week" ? entry.appearances : entry.totalPoints)}</span>
              <span className="chart-row__actions">{entry.matchedTrackId || entry.matchedAlbumId ? <CheckCircle2 aria-label="In your library" /> : <span aria-label="Not matched">—</span>}{entry.loved ? <Heart className="is-loved" aria-label="Loved" /> : null}{entry.matchedTrackId ? <button type="button" aria-label={`Play ${entry.title}`} onClick={(event) => { event.stopPropagation(); void loadChartEntryTrack(entry.matchedTrackId!).then((track) => callbacksRef.current.onPlayQueue([track])); }}><Play aria-hidden="true" /></button> : null}</span>
            </div>;
          })}
        </div>
        {queueMessage ? <p className="chart-queue-message" role="status">{queueMessage}</p> : null}
      </section>

      {selectedEntry ? <section className="chart-comparison" aria-label={`Across the sources for ${selectedEntry.title}`}>
        <div><h3>Across the sources</h3><p>{selectedEntry.title} by {selectedEntry.artist}</p></div>
        <div className="chart-comparison__sources">
          {(detail?.sourceRanks ?? []).map((rank) => <div key={rank.source}><span>{rank.label}{rank.annualOnly ? <small> annual</small> : null}</span><strong>{rank.bestRank === null ? "—" : `#${rank.bestRank}`}</strong><i style={{ width: `${rank.bestRank === null ? 0 : Math.max(8, 100 - rank.bestRank)}%` }} /></div>)}
          {!detail ? <span className="chart-comparison__loading"><LoaderCircle className="is-spinning" aria-hidden="true" /> Comparing source archives…</span> : null}
        </div>
      </section> : null}

      <section className="chart-score-shelf" aria-labelledby="chart-score-heading">
        <header>
          <div><h3 id="chart-score-heading">Aurora Album Score <span>· {page.request.period.fromYear === page.request.period.toYear ? page.request.period.fromYear : page.request.period.label}</span></h3><p>Using {page.request.yearBasis === "year" ? "Year" : "Release Year"} for the selected period</p></div>
          <div className="chart-score-shelf__actions">
            <div className="chart-year-basis" role="group" aria-label="Aurora Score year basis">
              <button type="button" aria-pressed={page.request.yearBasis === "year"} onClick={() => changeYearBasis("year")}>Year</button>
              <button type="button" aria-pressed={page.request.yearBasis === "releaseYear"} onClick={() => changeYearBasis("releaseYear")}>Release Year</button>
            </div>
            <button type="button" className="chart-score-shelf__open" onClick={() => { changeKind("albums"); changeSource("auroraScore"); }}>View full chart <ChevronRight aria-hidden="true" /></button>
          </div>
        </header>
        <div>{page.albumScoreEntries.map((album, index) => <button type="button" onClick={() => { const entry = scoreEntriesToChart(album, index); selectEntry(entry, { ...page, request: { ...page.request, kind: "albums", source: "auroraScore", scope: "period" }, chartTitle: `Aurora Album Score · ${page.request.period.label}` }); }} key={album.id}><strong>{index + 1}</strong><Artwork track={scoreAsTrack(album)} decorative={false} /><span><b>{album.title}</b><small>{album.artist}</small></span><em>{album.score.toFixed(1)}</em></button>)}</div>
      </section>
    </>}
    {customOpen ? <CustomPeriodDialog initial={request.period} onClose={() => setCustomOpen(false)} onApply={applyPeriod} /> : null}
  </section>;
}

function scoreEntriesToChart(album: AlbumScoreEntry, index: number): ChartEntry {
  return {
    position: index + 1,
    sourcePosition: index + 1,
    previousPosition: null,
    movement: null,
    peakPosition: index + 1,
    appearances: 1,
    weeksAtNumberOne: index === 0 ? 1 : 0,
    totalPoints: Math.round(album.score),
    artist: album.artist,
    title: album.title,
    artistKey: album.artist.toLocaleLowerCase(),
    titleKey: album.title.toLocaleLowerCase(),
    matchedTrackId: null,
    matchedAlbumId: album.id,
    artworkAlbumId: album.id,
    rating: null,
    loved: false,
    albumScore: album.score,
  };
}

export function ChartInspector({
  selection,
  track,
  busy,
  onPlay,
  onOpenLibrary,
  onRatingChange,
  onLoveChange,
}: {
  selection: ChartSelectionContext;
  track: Track | null;
  busy: boolean;
  onPlay: () => void;
  onOpenLibrary: () => void;
  onRatingChange: (track: Track, rating: number | null) => void;
  onLoveChange: (track: Track, state: LoveState) => void;
}) {
  const { entry, detail, pageRequest } = selection;
  return <div className="chart-inspector">
    <Artwork track={track ?? entryAsTrack(entry)} size="large" decorative={false} />
    <div className="chart-inspector__heading"><span>#{entry.position}</span><div><h2>{entry.title}</h2><p>{entry.artist}</p></div>{track?.loved || entry.loved ? <Heart aria-label="Loved" /> : null}</div>
    <dl className="metadata-list">
      <div><dt>Chart</dt><dd>{selection.chartTitle}</dd></div>
      <div><dt>{pageRequest.scope === "week" ? "Week" : "Period"}</dt><dd>{pageRequest.scope === "week" ? `${pageRequest.selectedWeek} · ${pageRequest.selectedYear}` : pageRequest.period.label}</dd></div>
      <div><dt>Peak position</dt><dd>{entry.peakPosition === null ? "—" : `#${entry.peakPosition}`}</dd></div>
      <div><dt>{pageRequest.scope === "week" ? "Weeks on chart" : "Appearances"}</dt><dd>{entry.appearances}</dd></div>
      {pageRequest.scope === "period" ? <><div><dt>Weeks at #1</dt><dd>{entry.weeksAtNumberOne}</dd></div><div><dt>Total points</dt><dd>{formatCount(entry.totalPoints)}</dd></div></> : null}
      {entry.albumScore !== null ? <div><dt>Album Score</dt><dd>{entry.albumScore.toFixed(1)}</dd></div> : null}
    </dl>
    <section className="chart-inspector__history"><header><h3>Source history</h3><span>{pageRequest.period.label}</span></header>{detail?.sourceRanks.map((rank) => <div key={rank.source}><span><i />{rank.label}{rank.annualOnly ? <small> annual</small> : null}</span><strong>{rank.bestRank === null ? "—" : `#${rank.bestRank}`}</strong></div>) ?? <p><LoaderCircle className="is-spinning" aria-hidden="true" /> Loading matches…</p>}</section>
    <div className="chart-inspector__library">{entry.matchedTrackId || entry.matchedAlbumId ? <><CheckCircle2 aria-hidden="true" /><span><strong>In your library</strong><small>Matched to local catalog</small></span></> : <><Library aria-hidden="true" /><span><strong>Not matched</strong><small>Chart history is still available</small></span></>}</div>
    {track ? <div className="chart-inspector__tags"><label>Your rating</label><InlineRatingControl title={track.title} rating={track.rating} busy={busy} allowClear onRatingChange={(rating) => onRatingChange(track, rating)} /><label>Love</label><InlineLoveControl title={track.title} loveState={track.loveState} busy={busy} onLoveChange={(state) => onLoveChange(track, state)} /></div> : null}
    <button type="button" className="button button--primary chart-inspector__play" disabled={busy || (!entry.matchedTrackId && !entry.matchedAlbumId)} onClick={onPlay}>{busy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play</button>
    <button type="button" className="button button--quiet chart-inspector__open" disabled={!entry.matchedTrackId && !entry.matchedAlbumId} onClick={onOpenLibrary}><Library aria-hidden="true" /> Open in Library</button>
    <p><Star aria-hidden="true" /> Weekly sources remain exact. Billboard and Aurora Score are presented as annual or period charts.</p>
  </div>;
}
