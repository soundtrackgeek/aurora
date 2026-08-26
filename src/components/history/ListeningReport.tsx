import {
  Album,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Disc3,
  Headphones,
  Monitor,
  Music2,
  Play,
  RefreshCw,
  Sparkles,
  UserRound,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  loadHistoryReport,
  type HistoryDevice,
  type HistoryReport,
  type HistoryReportBucket,
} from "../../history";
import { formatCount, type Track } from "../../library";
import { Artwork } from "../Artwork";
import "./ListeningReport.css";

type ReportPeriod = "7" | "30" | "90" | "all";

interface ReportRange {
  startedAfterMs?: number;
  startedBeforeMs: number;
  previousStartedAfterMs?: number;
  previousStartedBeforeMs?: number;
}

interface ListeningReportProps {
  devices: HistoryDevice[];
  deviceId: string | null;
  onDeviceChange: (value: string | null) => void;
  onPlayTrack: (track: Track) => void;
}

const DAY_MS = 86_400_000;
const periods: Array<{ value: ReportPeriod; label: string }> = [
  { value: "7", label: "7 days" },
  { value: "30", label: "30 days" },
  { value: "90", label: "90 days" },
  { value: "all", label: "All time" },
];

function startOfLocalDay(timestamp: number): number {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

function reportRange(period: ReportPeriod, offset: number): ReportRange {
  if (period === "all") {
    return {
      startedAfterMs: undefined,
      startedBeforeMs: Date.now(),
      previousStartedAfterMs: undefined,
      previousStartedBeforeMs: undefined,
    };
  }
  const days = Number(period);
  const endExclusive = startOfLocalDay(Date.now()) + DAY_MS - offset * days * DAY_MS;
  const startedAfterMs = endExclusive - days * DAY_MS;
  return {
    startedAfterMs,
    startedBeforeMs: endExclusive - 1,
    previousStartedAfterMs: startedAfterMs - days * DAY_MS,
    previousStartedBeforeMs: startedAfterMs - 1,
  };
}

function dateRangeLabel(period: ReportPeriod, range: ReturnType<typeof reportRange>): string {
  if (period === "all" || range.startedAfterMs === undefined) return "All listening history";
  const start = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(range.startedAfterMs);
  const end = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(range.startedBeforeMs);
  return `${start} – ${end}`;
}

function durationLabel(seconds: number): string {
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.round((seconds % 3_600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes} min`;
  return `${Math.round(seconds)} sec`;
}

function percent(value: number, total: number): number {
  return total > 0 ? Math.round((value / total) * 100) : 0;
}

function comparison(current: number, previous: number | undefined): { label: string; direction: "up" | "down" | "flat" } {
  if (previous === undefined) return { label: "Your complete listening record", direction: "flat" };
  if (previous === 0) return current === 0
    ? { label: "No change from the previous period", direction: "flat" }
    : { label: "New activity after a quiet previous period", direction: "up" };
  const change = Math.round(((current - previous) / previous) * 100);
  return {
    label: `${change >= 0 ? "+" : ""}${change}% vs. previous period`,
    direction: change > 0 ? "up" : change < 0 ? "down" : "flat",
  };
}

function aggregateBuckets(
  values: HistoryReportBucket[],
  period: ReportPeriod,
  range: ReturnType<typeof reportRange>,
): Array<{ label: string; plays: number }> {
  if (values.length === 0) return [];
  const start = period === "all" || range.startedAfterMs === undefined
    ? startOfLocalDay(values[0].startMs)
    : range.startedAfterMs;
  const end = (range.startedBeforeMs ?? Date.now()) + 1;
  const totalDays = Math.max(1, Math.ceil((end - start) / DAY_MS));
  const bucketCount = Math.min(period === "7" ? 7 : 10, totalDays);
  const bucketDays = Math.ceil(totalDays / bucketCount);
  const result = Array.from({ length: bucketCount }, (_, index) => {
    const bucketStart = start + index * bucketDays * DAY_MS;
    return {
      label: new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(bucketStart),
      plays: 0,
    };
  });
  for (const value of values) {
    const index = Math.min(bucketCount - 1, Math.max(0, Math.floor((value.startMs - start) / (bucketDays * DAY_MS))));
    result[index].plays += value.plays;
  }
  return result;
}

function ActivityChart({ report, period, range }: {
  report: HistoryReport;
  period: ReportPeriod;
  range: ReturnType<typeof reportRange>;
}) {
  const current = aggregateBuckets(report.daily, period, range);
  const previous = aggregateBuckets(report.previousDaily, period, {
    ...range,
    startedAfterMs: range.previousStartedAfterMs,
    startedBeforeMs: range.previousStartedBeforeMs ?? range.startedBeforeMs,
  });
  const count = Math.max(current.length, previous.length, 1);
  const max = Math.max(1, ...current.map((item) => item.plays), ...previous.map((item) => item.plays));
  const width = 720;
  const plotHeight = 180;
  const baseline = 205;
  const groupWidth = width / count;
  return (
    <svg className="report-activity" viewBox={`0 0 ${width} 238`} role="img" aria-label="Registered plays compared with the previous period">
      <title>Registered plays by date</title>
      {[0, 0.5, 1].map((ratio) => <line key={ratio} x1="18" x2={width - 8} y1={baseline - plotHeight * ratio} y2={baseline - plotHeight * ratio} />)}
      {Array.from({ length: count }, (_, index) => {
        const currentItem = current[index];
        const previousItem = previous[index];
        const currentHeight = ((currentItem?.plays ?? 0) / max) * plotHeight;
        const previousHeight = ((previousItem?.plays ?? 0) / max) * plotHeight;
        const x = index * groupWidth + groupWidth * 0.22;
        return (
          <g key={index}>
            <rect className="report-activity__previous" x={x + groupWidth * 0.28} y={baseline - previousHeight} width={Math.max(5, groupWidth * 0.2)} height={previousHeight} rx="2" />
            <rect className="report-activity__current" x={x} y={baseline - currentHeight} width={Math.max(7, groupWidth * 0.22)} height={currentHeight} rx="2"><title>{currentItem?.label ?? "Period"}: {currentItem?.plays ?? 0} plays</title></rect>
            <text x={x + groupWidth * 0.22} y="228" textAnchor="middle">{currentItem?.label ?? ""}</text>
          </g>
        );
      })}
    </svg>
  );
}

function ListeningClock({ hourly }: { hourly: number[] }) {
  const max = Math.max(1, ...hourly);
  const busiest = hourly.reduce((best, value, index) => value > hourly[best] ? index : best, 0);
  return (
    <div className="report-clock">
      <svg viewBox="0 0 300 300" role="img" aria-label={`Listening activity by hour. Busiest hour ${busiest}:00 with ${hourly[busiest]} plays.`}>
        <title>Listening activity over 24 hours</title>
        {hourly.map((value, hour) => {
          const angle = hour * 15;
          const height = 10 + (value / max) * 42;
          return <rect key={hour} className={hour === busiest ? "is-busiest" : ""} x="143" y="32" width="14" height={height} rx="4" transform={`rotate(${angle} 150 150)`}><title>{hour}:00 · {value} plays</title></rect>;
        })}
        <circle cx="150" cy="150" r="68" />
        <text x="150" y="135" textAnchor="middle">Busiest hour</text>
        <text className="report-clock__time" x="150" y="165" textAnchor="middle">{String(busiest).padStart(2, "0")}:00</text>
        <text x="150" y="186" textAnchor="middle">{hourly[busiest]} plays</text>
        <text x="150" y="18" textAnchor="middle">00</text><text x="286" y="154" textAnchor="middle">06</text>
        <text x="150" y="296" textAnchor="middle">12</text><text x="14" y="154" textAnchor="middle">18</text>
      </svg>
    </div>
  );
}

function RadarChart({ report }: { report: HistoryReport }) {
  const { summary, discovery, topTracks } = report;
  const values = [
    Math.min(100, Math.round((summary.activeDays / Math.max(1, Math.min(summary.sessions, 30))) * 100)),
    percent(discovery.newTracks, discovery.totalTracks),
    Math.min(100, Math.round(((topTracks[0]?.plays ?? 0) / Math.max(1, summary.plays)) * 260)),
    Math.min(100, Math.round((summary.completed / Math.max(1, summary.sessions)) * 100)),
    Math.min(100, Math.round((summary.uniqueArtists / Math.max(1, summary.plays)) * 220)),
  ];
  const labels = ["Consistency", "Discovery", "Replay", "Focus", "Variety"];
  const center = 150;
  const radius = 96;
  const point = (index: number, value: number) => {
    const angle = -Math.PI / 2 + index * (Math.PI * 2 / labels.length);
    const scaled = radius * value / 100;
    return [center + Math.cos(angle) * scaled, center + Math.sin(angle) * scaled];
  };
  const polygon = (value: number) => labels.map((_, index) => point(index, value).join(",")).join(" ");
  return (
    <svg className="report-radar" viewBox="0 0 300 300" role="img" aria-label={`Listening fingerprint: ${labels.map((label, index) => `${label} ${values[index]}`).join(", ")}`}>
      <title>Listening fingerprint</title>
      {[25, 50, 75, 100].map((value) => <polygon key={value} className="report-radar__grid" points={polygon(value)} />)}
      {labels.map((label, index) => {
        const [x, y] = point(index, 118);
        return <g key={label}><line x1={center} y1={center} x2={point(index, 100)[0]} y2={point(index, 100)[1]} /><text x={x} y={y} textAnchor="middle">{label}<tspan x={x} dy="15">{values[index]}</tspan></text></g>;
      })}
      <polygon className="report-radar__value" points={values.map((value, index) => point(index, value).join(",")).join(" ")} />
      {values.map((value, index) => <circle key={labels[index]} cx={point(index, value)[0]} cy={point(index, value)[1]} r="4" />)}
    </svg>
  );
}

function TopMusic({ report, onPlayTrack }: { report: HistoryReport; onPlayTrack: (track: Track) => void }) {
  return (
    <section className="report-section report-top" aria-labelledby="report-top-title">
      <div className="report-heading"><div><h2 id="report-top-title">Top music</h2><p>The artists, albums, and tracks that shaped this period.</p></div></div>
      <div className="report-top__columns">
        <article><h3><UserRound aria-hidden="true" /> Artists</h3>{report.topArtists.map((item, index) => <div className="report-rank" key={item.artist}><strong>{index + 1}</strong><span className="report-artist-mark">{item.artist.slice(0, 1).toLocaleUpperCase()}</span><span><b>{item.artist}</b><small>{durationLabel(item.listenedSeconds)}</small></span><em>{item.plays} plays</em></div>)}</article>
        <article><h3><Disc3 aria-hidden="true" /> Albums</h3>{report.topAlbums.map((item, index) => <div className="report-rank" key={`${item.artist}-${item.album}`}><strong>{index + 1}</strong>{item.track ? <Artwork track={item.track} size="small" /> : <span className="report-artwork-fallback"><Album aria-hidden="true" /></span>}<span><b>{item.album}</b><small>{item.artist}</small></span><em>{item.plays} plays</em></div>)}</article>
        <article><h3><Music2 aria-hidden="true" /> Tracks</h3>{report.topTracks.map((item, index) => <div className="report-rank" key={item.trackKey}><strong>{index + 1}</strong><span><b>{item.title}</b><small>{item.artist}</small></span><em>{item.plays} plays</em><button type="button" disabled={!item.track} onClick={() => item.track && onPlayTrack(item.track)} aria-label={`Play ${item.title}`}><Play aria-hidden="true" /></button></div>)}</article>
      </div>
    </section>
  );
}

export function ListeningReport({ devices, deviceId, onDeviceChange, onPlayTrack }: ListeningReportProps) {
  const [period, setPeriod] = useState<ReportPeriod>("7");
  const [offset, setOffset] = useState(0);
  const [reloadToken, setReloadToken] = useState(0);
  const range = useMemo(() => reportRange(period, offset), [offset, period]);
  const requestKey = `${range.startedAfterMs ?? "all"}:${range.startedBeforeMs ?? "now"}:${deviceId ?? "all"}:${reloadToken}`;
  const [result, setResult] = useState<{ key: string; report: HistoryReport | null; error: string | null }>({ key: "", report: null, error: null });

  useEffect(() => {
    let cancelled = false;
    void loadHistoryReport({
      ...range,
      deviceId: deviceId ?? undefined,
      timezoneOffsetMinutes: new Date().getTimezoneOffset(),
    }).then((next) => {
      if (cancelled) return;
      setResult({ key: requestKey, report: next, error: null });
    }).catch((reason: unknown) => {
      if (cancelled) return;
      setResult({ key: requestKey, report: null, error: reason instanceof Error ? reason.message : String(reason) });
    });
    return () => { cancelled = true; };
  }, [deviceId, range, requestKey]);

  const report = result.report;
  const isLoading = result.key !== requestKey;
  const error = result.key === requestKey ? result.error : null;
  const delta = comparison(report?.summary.plays ?? 0, report?.previousSummary?.plays);
  const maxDecade = Math.max(1, ...(report?.decades.map((item) => item.plays) ?? []));
  const completionRate = percent(report?.summary.completed ?? 0, report?.summary.sessions ?? 0);

  if (error) return <section className="report-state" role="alert"><Music2 aria-hidden="true" /><h2>Listening report unavailable</h2><p>{error}</p><button type="button" onClick={() => setReloadToken((value) => value + 1)}><RefreshCw aria-hidden="true" /> Try again</button></section>;

  return (
    <section className="listening-report" aria-labelledby="listening-report-title" aria-busy={isLoading}>
      <div className="report-toolbar">
        <div className="report-periods" aria-label="Report period">{periods.map((item) => <button type="button" key={item.value} className={period === item.value ? "is-active" : ""} aria-pressed={period === item.value} onClick={() => { setPeriod(item.value); setOffset(0); }}>{item.label}</button>)}</div>
        <div className="report-date"><button type="button" disabled={period === "all"} onClick={() => setOffset((value) => value + 1)} aria-label="Previous period"><ChevronLeft aria-hidden="true" /></button><span><CalendarDays aria-hidden="true" />{dateRangeLabel(period, range)}</span><button type="button" disabled={period === "all" || offset === 0} onClick={() => setOffset((value) => Math.max(0, value - 1))} aria-label="Next period"><ChevronRight aria-hidden="true" /></button></div>
        <label className="report-device"><Monitor aria-hidden="true" /><span className="sr-only">Report device</span><select value={deviceId ?? "all"} onChange={(event) => onDeviceChange(event.target.value === "all" ? null : event.target.value)}><option value="all">All devices</option>{devices.map((device) => <option key={device.deviceId} value={device.deviceId}>{device.deviceName}{device.isThisDevice ? " · this device" : ""}</option>)}</select></label>
      </div>

      {isLoading && !report ? <div className="report-state" aria-live="polite"><RefreshCw className="is-spinning" aria-hidden="true" /><p>Reading your complete listening history…</p></div> : report && <>
        <section className="report-hero">
          <div className="report-hero__copy"><h1 id="listening-report-title">Your listening,<br />in focus.</h1><p><strong>{formatCount(report.summary.plays)}</strong> registered plays</p><span className={`report-delta report-delta--${delta.direction}`}>{delta.direction === "up" ? "↑" : delta.direction === "down" ? "↓" : "—"} {delta.label}</span></div>
          <div className="report-hero__chart"><div className="report-legend"><span><i />This period</span>{report.previousSummary && <span><i />Previous period</span>}</div><ActivityChart report={report} period={period} range={range} /></div>
        </section>

        {report.summary.sessions === 0 ? <section className="report-empty"><Headphones aria-hidden="true" /><h2>No listening in this period</h2><p>Move to another period or start listening to build your next report.</p></section> : <>
          <TopMusic report={report} onPlayTrack={onPlayTrack} />
          <section className="report-analysis">
            <article><div className="report-heading"><div><h2>Listening rhythm</h2><p>When your registered plays happen across 24 hours.</p></div></div><ListeningClock hourly={report.hourly} /></article>
            <article><div className="report-heading"><div><h2>Listening fingerprint</h2><p>Five signals derived from this period—no global score.</p></div></div><RadarChart report={report} /></article>
          </section>
          <section className="report-analysis report-analysis--lower">
            <article><div className="report-heading"><div><h2>Music by decade</h2><p>Registered plays by the release year in your catalog.</p></div></div><div className="report-decades">{report.decades.map((item) => <div key={item.decade}><span>{item.decade}</span><i><b style={{ width: `${(item.plays / maxDecade) * 100}%` }} /></i><strong>{item.plays}</strong></div>)}</div></article>
            <article><div className="report-heading"><div><h2>Discovery</h2><p>Music first heard in Aurora during this period.</p></div></div><div className="report-discovery"><div><UserRound aria-hidden="true" /><span><small>New artists</small><strong>{percent(report.discovery.newArtists, report.discovery.totalArtists)}%</strong><em>{report.discovery.newArtists} of {report.discovery.totalArtists}</em></span></div><div><Disc3 aria-hidden="true" /><span><small>New albums</small><strong>{percent(report.discovery.newAlbums, report.discovery.totalAlbums)}%</strong><em>{report.discovery.newAlbums} of {report.discovery.totalAlbums}</em></span></div><div><Sparkles aria-hidden="true" /><span><small>New tracks</small><strong>{percent(report.discovery.newTracks, report.discovery.totalTracks)}%</strong><em>{report.discovery.newTracks} of {report.discovery.totalTracks}</em></span></div></div></article>
          </section>
          <section className="report-section report-facts"><div className="report-heading"><div><h2>Quick facts</h2><p>The shape of this listening period at a glance.</p></div></div><div className="report-facts__grid"><article><Clock3 aria-hidden="true" /><span>Listening time</span><strong>{durationLabel(report.summary.listenedSeconds)}</strong><small>Total session time</small></article><article><Music2 aria-hidden="true" /><span>Average per active day</span><strong>{Math.round(report.summary.plays / Math.max(1, report.summary.activeDays))}</strong><small>Registered plays</small></article><article><CalendarDays aria-hidden="true" /><span>Most active day</span><strong>{report.summary.mostActiveDayStartMs ? new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric" }).format(report.summary.mostActiveDayStartMs) : "—"}</strong><small>{report.summary.mostActiveDayPlays} plays</small></article><article><Headphones aria-hidden="true" /><span>Longest session</span><strong>{durationLabel(report.summary.longestSessionSeconds)}</strong><small>{report.summary.longestSessionStartedAtMs ? new Intl.DateTimeFormat(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit" }).format(report.summary.longestSessionStartedAtMs) : "No sessions"}</small></article><article><Disc3 aria-hidden="true" /><span>Completion rate</span><strong>{completionRate}%</strong><small>{report.summary.completed} completed sessions</small></article></div></section>
        </>}
      </>}
    </section>
  );
}
