import { useEffect, useState } from "react";
import { loadTrackChartPeaks, type TrackChartPeak } from "../../charts";
import "./TrackChartInfo.css";

export function TrackChartInfo({ artist, title }: { artist: string; title: string }) {
  const identity = JSON.stringify([artist, title]);
  const [country, setCountry] = useState<"ALL" | "US" | "UK" | "NO">("ALL");
  const [result, setResult] = useState<{ identity: string; peaks?: TrackChartPeak[]; error?: string } | null>(null);
  useEffect(() => {
    let active = true;
    void loadTrackChartPeaks(artist, title).then(
      (peaks) => { if (active) setResult({ identity, peaks }); },
      () => { if (active) setResult({ identity, error: "Chart info could not be loaded." }); },
    );
    return () => { active = false; };
  }, [artist, title, identity]);
  const current = result?.identity === identity ? result : null;
  const peaks = current?.peaks?.filter((peak) => country === "ALL" || peak.country === country);
  return <section className="track-chart-info" aria-label="Chart info">
    <h3>Chart info</h3>
    <p className="track-chart-info__caption">All-time peaks</p>
    <div className="track-chart-info__filters" role="group" aria-label="Chart country">
      {(["ALL", "US", "UK", "NO"] as const).map((value) => <button key={value} type="button" aria-pressed={country === value} onClick={() => setCountry(value)}>{value}</button>)}
    </div>
    {!current ? <p role="status">Loading chart info…</p> : current.error ? <p role="status">{current.error}</p> : peaks?.length ?
      <dl>{peaks.map(({ label, peak, country: region }) => <div key={`${region}:${label}`}><dt>{label}</dt><dd>#{peak}</dd></div>)}</dl> : <p>No chart appearances found{country === "ALL" ? "." : ` for ${country}.`}</p>}
  </section>;
}
