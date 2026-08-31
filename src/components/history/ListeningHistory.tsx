import {
  BarChart3,
  CalendarDays,
  Check,
  Clock3,
  CloudOff,
  Headphones,
  History,
  ListMusic,
  Play,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  SkipForward,
  TimerReset,
} from "lucide-react";
import { lazy, Suspense, useMemo, useState } from "react";
import type {
  HistoryItem,
  HistoryOutcomeFilter,
  HistoryPage,
} from "../../history";
import { formatCount, type Track } from "../../library";
import { Artwork } from "../Artwork";
import { ArtistSmartLink } from "../ArtistSmartLink";
import "./ListeningHistory.css";

const ListeningReport = lazy(() => import("./ListeningReport").then((module) => ({ default: module.ListeningReport })));

export type HistoryLoadState = "loading" | "ready" | "error";
export type HistoryDateRange = "all" | "7" | "30" | "90";

interface ListeningHistoryProps {
  page: HistoryPage | null;
  loadState: HistoryLoadState;
  errorMessage: string | null;
  search: string;
  outcome: HistoryOutcomeFilter;
  deviceId: string | null;
  dateRange: HistoryDateRange;
  isLoadingMore: boolean;
  isSavingThreshold: boolean;
  thresholdMessage: string | null;
  onSearchChange: (value: string) => void;
  onOutcomeChange: (value: HistoryOutcomeFilter) => void;
  onDeviceChange: (value: string | null) => void;
  onDateRangeChange: (value: HistoryDateRange) => void;
  onSaveThreshold: (value: number) => void;
  onSelectTrack: (track: Track) => void;
  onPlayTrack: (track: Track) => void;
  onOpenArtistAlbums: (artist: string) => void;
  onLoadMore: () => void;
  onRefresh: () => void;
}

function listenedLabel(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)} sec listened`;
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return remainder > 0 ? `${minutes}m ${remainder}s listened` : `${minutes} min listened`;
}

function totalTimeLabel(seconds: number): string {
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes} min`;
}

function dayKey(timestamp: number): string {
  const date = new Date(timestamp);
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

function dayLabel(timestamp: number): string {
  const date = new Date(timestamp);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (dayKey(timestamp) === dayKey(today.getTime())) return "Today";
  if (dayKey(timestamp) === dayKey(yesterday.getTime())) return "Yesterday";
  return new Intl.DateTimeFormat(undefined, { weekday: "long", month: "short", day: "numeric" }).format(date);
}

function timeLabel(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(new Date(timestamp));
}

function outcomeLabel(item: HistoryItem): string {
  if (item.outcome === "active") return item.registeredPlay ? "Playing · counted" : "Listening";
  if (item.outcome === "completed") return "Completed";
  if (item.outcome === "skipped") return item.registeredPlay ? "Counted · skipped" : "Skipped";
  return item.registeredPlay ? "Counted · interrupted" : "Interrupted";
}

function outcomeIcon(item: HistoryItem) {
  if (item.outcome === "completed") return <Check aria-hidden="true" />;
  if (item.outcome === "skipped") return <SkipForward aria-hidden="true" />;
  if (item.outcome === "active") return <Headphones aria-hidden="true" />;
  return <TimerReset aria-hidden="true" />;
}

function HistoryRow({ item, onSelectTrack, onPlayTrack, onOpenArtistAlbums }: {
  item: HistoryItem;
  onSelectTrack: (track: Track) => void;
  onPlayTrack: (track: Track) => void;
  onOpenArtistAlbums: (artist: string) => void;
}) {
  return (
    <article className={`history-row history-row--${item.outcome}${item.registeredPlay ? " is-play" : ""}`}>
      <button
        type="button"
        className="history-row__track"
        disabled={!item.track}
        onClick={() => item.track && onSelectTrack(item.track)}
        aria-label={item.track ? `Inspect ${item.title}` : `${item.title} is unavailable in the current catalog`}
      >
        {item.track ? <Artwork track={item.track} size="small" /> : <span className="history-row__missing"><ListMusic aria-hidden="true" /></span>}
        <span className="history-row__copy">
          <strong>{item.title}</strong>
          <small><ArtistSmartLink artist={item.artist} onOpen={onOpenArtistAlbums} nested /> <span>·</span> {item.album}</small>
        </span>
      </button>
      <span className="history-row__time"><strong>{timeLabel(item.startedAtMs)}</strong><small>{listenedLabel(item.listenedSeconds)}</small></span>
      <span className={`history-row__outcome history-row__outcome--${item.outcome}`}>
        {outcomeIcon(item)}<span>{outcomeLabel(item)}</span>
      </span>
      <span className="history-row__device">{item.deviceName}</span>
      <button
        type="button"
        className="history-row__play"
        disabled={!item.track}
        onClick={() => item.track && onPlayTrack(item.track)}
        aria-label={`Play ${item.title} again`}
      ><Play aria-hidden="true" /></button>
    </article>
  );
}

function ThresholdSettings({ currentValue, isSaving, message, onSave }: {
  currentValue: number;
  isSaving: boolean;
  message: string | null;
  onSave: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(currentValue));
  const parsed = Number(draft);
  const valid = Number.isInteger(parsed) && parsed >= 1 && parsed <= 3_600;

  return (
    <section className="history-threshold" aria-labelledby="threshold-title">
      <div className="history-threshold__icon"><TimerReset aria-hidden="true" /></div>
      <div><h2 id="threshold-title">When does this device count a play?</h2><p>Only forward playback time counts. A shorter track counts when it finishes.</p></div>
      <label><span>Seconds</span><input type="number" min="1" max="3600" step="1" value={draft} onChange={(event) => setDraft(event.target.value)} /></label>
      <button type="button" disabled={!valid || isSaving || parsed === currentValue} onClick={() => onSave(parsed)}>
        {isSaving ? <RefreshCw className="is-spinning" aria-hidden="true" /> : <Save aria-hidden="true" />}
        {isSaving ? "Saving…" : "Save"}
      </button>
      {message && <span className="history-threshold__message" role="status">{message}</span>}
    </section>
  );
}

export function ListeningHistory({
  page,
  loadState,
  errorMessage,
  search,
  outcome,
  deviceId,
  dateRange,
  isLoadingMore,
  isSavingThreshold,
  thresholdMessage,
  onSearchChange,
  onOutcomeChange,
  onDeviceChange,
  onDateRangeChange,
  onSaveThreshold,
  onSelectTrack,
  onPlayTrack,
  onOpenArtistAlbums,
  onLoadMore,
  onRefresh,
}: ListeningHistoryProps) {
  const [activePage, setActivePage] = useState<"report" | "history">("report");
  const grouped = useMemo(() => {
    const groups: Array<{ key: string; label: string; items: HistoryItem[] }> = [];
    for (const item of page?.items ?? []) {
      const key = dayKey(item.startedAtMs);
      const current = groups[groups.length - 1];
      if (current?.key === key) current.items.push(item);
      else groups.push({ key, label: dayLabel(item.startedAtMs), items: [item] });
    }
    return groups;
  }, [page?.items]);

  return (
    <div className="history-shell">
      <nav className="history-page-tabs" aria-label="Listening memory pages">
        <button type="button" className={activePage === "report" ? "is-active" : ""} aria-current={activePage === "report" ? "page" : undefined} onClick={() => setActivePage("report")}><BarChart3 aria-hidden="true" /> Listening report</button>
        <button type="button" className={activePage === "history" ? "is-active" : ""} aria-current={activePage === "history" ? "page" : undefined} onClick={() => setActivePage("history")}><History aria-hidden="true" /> History</button>
      </nav>
      {activePage === "report" ? (
        <Suspense fallback={<section className="history-state" aria-live="polite"><RefreshCw className="is-spinning" aria-hidden="true" /><p>Opening listening report…</p></section>}>
          <ListeningReport devices={page?.devices ?? []} deviceId={deviceId} onDeviceChange={onDeviceChange} onPlayTrack={onPlayTrack} onOpenArtistAlbums={onOpenArtistAlbums} />
        </Suspense>
      ) : (
    <section className="history-view" aria-labelledby="history-title">
      <header className="history-hero">
        <div>
          <p className="eyebrow">Listening Memory</p>
          <h1 id="history-title">Your music remembers.</h1>
          <p>Every real listen, across Desktop and Laptop Mode, without confusing Last.fm popularity for your plays.</p>
        </div>
        <div className={`history-sync history-sync--${page?.syncState ?? "synced"}`}>
          {page?.syncState === "unavailable" ? <CloudOff aria-hidden="true" /> : <History aria-hidden="true" />}
          <span><strong>{page?.syncState === "unavailable" ? "Local history available" : "Device histories combined"}</strong><small>{page?.syncMessage ?? "Opening listening history…"}</small></span>
        </div>
      </header>

      <div className="history-stats" aria-label="Listening history summary">
        <article><Headphones aria-hidden="true" /><span><small>Registered plays</small><strong>{formatCount(page?.summary.plays ?? 0)}</strong></span></article>
        <article><Clock3 aria-hidden="true" /><span><small>Time listened</small><strong>{totalTimeLabel(page?.summary.listenedSeconds ?? 0)}</strong></span></article>
        <article><ListMusic aria-hidden="true" /><span><small>Unique tracks</small><strong>{formatCount(page?.summary.uniqueTracks ?? 0)}</strong></span></article>
        <article><SkipForward aria-hidden="true" /><span><small>Skipped sessions</small><strong>{formatCount(page?.summary.skips ?? 0)}</strong></span></article>
      </div>

      <ThresholdSettings
        key={page?.playThresholdSeconds ?? 30}
        currentValue={page?.playThresholdSeconds ?? 30}
        isSaving={isSavingThreshold}
        message={thresholdMessage}
        onSave={onSaveThreshold}
      />

      {(page?.topTracks.length ?? 0) > 0 && (
        <section className="history-top" aria-labelledby="history-top-title">
          <div className="history-section-heading"><div><p className="eyebrow">Your rotation</p><h2 id="history-top-title">Most played</h2></div></div>
          <div className="history-top__grid">
            {page?.topTracks.slice(0, 4).map((item, index) => (
              <button type="button" key={item.trackKey} disabled={!item.track} onClick={() => item.track && onPlayTrack(item.track)}>
                <span className="history-top__rank">{String(index + 1).padStart(2, "0")}</span>
                {item.track ? <Artwork track={item.track} size="small" /> : <span className="history-row__missing"><ListMusic aria-hidden="true" /></span>}
                <span><strong>{item.title}</strong><small><ArtistSmartLink artist={item.artist} onOpen={onOpenArtistAlbums} nested /> · {item.plays} {item.plays === 1 ? "play" : "plays"}</small></span>
                <Play aria-hidden="true" />
              </button>
            ))}
          </div>
        </section>
      )}

      <section className="history-timeline" aria-labelledby="history-timeline-title">
        <div className="history-section-heading history-section-heading--timeline">
          <div><p className="eyebrow">Across your devices</p><h2 id="history-timeline-title">Listening timeline</h2></div>
          <button type="button" onClick={onRefresh}><RefreshCw aria-hidden="true" /> Refresh</button>
        </div>
        <div className="history-filters" aria-label="History filters">
          <label className="history-search"><Search aria-hidden="true" /><span className="sr-only">Search history</span><input value={search} onChange={(event) => onSearchChange(event.target.value)} placeholder="Track, artist, album, genre…" /></label>
          <label><span>Outcome</span><select value={outcome} onChange={(event) => onOutcomeChange(event.target.value as HistoryOutcomeFilter)}><option value="all">All sessions</option><option value="played">Registered plays</option><option value="completed">Completed</option><option value="skipped">Skipped</option><option value="interrupted">Interrupted</option></select></label>
          <label><span>Device</span><select value={deviceId ?? "all"} onChange={(event) => onDeviceChange(event.target.value === "all" ? null : event.target.value)}><option value="all">All devices</option>{page?.devices.map((device) => <option key={device.deviceId} value={device.deviceId}>{device.deviceName}{device.isThisDevice ? " · this device" : ""}</option>)}</select></label>
          <label><span>Date</span><select value={dateRange} onChange={(event) => onDateRangeChange(event.target.value as HistoryDateRange)}><option value="all">All time</option><option value="7">Last 7 days</option><option value="30">Last 30 days</option><option value="90">Last 90 days</option></select></label>
          {(search || outcome !== "all" || deviceId || dateRange !== "all") && <button type="button" className="history-reset" onClick={() => { onSearchChange(""); onOutcomeChange("all"); onDeviceChange(null); onDateRangeChange("all"); }}><RotateCcw aria-hidden="true" /> Reset</button>}
        </div>

        {loadState === "loading" && !page ? <div className="history-state" aria-live="polite"><RefreshCw className="is-spinning" aria-hidden="true" /><p>Combining listening history…</p></div>
          : loadState === "error" ? <div className="history-state history-state--error" role="alert"><CloudOff aria-hidden="true" /><h3>Listening history is unavailable.</h3><p>{errorMessage}</p><button type="button" onClick={onRefresh}><RefreshCw aria-hidden="true" /> Try again</button></div>
            : grouped.length === 0 ? <div className="history-state"><CalendarDays aria-hidden="true" /><h3>No sessions match these filters.</h3><p>Play a track for {page?.playThresholdSeconds ?? 30} seconds to register your first play.</p></div>
              : <div className="history-days">{grouped.map((group) => <section key={group.key} className="history-day" aria-labelledby={`history-day-${group.key}`}><div className="history-day__label"><span /><h3 id={`history-day-${group.key}`}>{group.label}</h3><small>{group.items.length} {group.items.length === 1 ? "session" : "sessions"}</small></div><div>{group.items.map((item) => <HistoryRow key={item.sessionId} item={item} onSelectTrack={onSelectTrack} onPlayTrack={onPlayTrack} onOpenArtistAlbums={onOpenArtistAlbums} />)}</div></section>)}</div>}
        {page?.nextCursor && <div className="history-load-more"><button type="button" disabled={isLoadingMore} onClick={onLoadMore}>{isLoadingMore ? <RefreshCw className="is-spinning" aria-hidden="true" /> : <Clock3 aria-hidden="true" />}{isLoadingMore ? "Loading…" : "Load earlier sessions"}</button></div>}
      </section>
    </section>
      )}
    </div>
  );
}
