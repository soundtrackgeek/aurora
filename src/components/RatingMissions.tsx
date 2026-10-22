import { useEffect, useRef, useState } from "react";
import { operateRatingMissions, type MissionRequest, type MissionShelf, type RatingMission, type SonicTrack } from "../ratingMissions";
import "./RatingMissions.css";

const labels: Record<string, string> = { fit: "A likely fit", finish: "Close to completion", stretch: "A deliberate stretch" };
function remaining(mission: RatingMission): SonicTrack[] { return mission.tracks.filter(t => (t.rating ?? 0) === 0); }
function duration(seconds: number): string { return `${Math.ceil(seconds / 60)} min`; }
function request(action: string): MissionRequest { return { action, revision: null, missionId: null, minutes: null, mode: null, trackKeys: null }; }

export function RatingMissions({ onOpen, onPlay, onSaved }: {
  onOpen: (track: SonicTrack) => void;
  onPlay?: (tracks: SonicTrack[]) => Promise<void>;
  onSaved?: () => void;
}) {
  const [shelf, setShelf] = useState<MissionShelf | null>(null);
  const [minutes, setMinutes] = useState(40);
  const [mode, setMode] = useState("albums");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const serial = useRef(0);
  const locked = useRef(false);
  useEffect(() => {
    const control = serial;
    const id = ++control.current;
    operateRatingMissions(request("load")).then(result => {
      if (id !== control.current) return;
      setShelf(result); setMinutes(result.minutes); setMode(result.mode);
    }).catch(e => { if (id === control.current) setError(String(e)); }).finally(() => { if (id === control.current) setBusy(false); });
    return () => { control.current++; };
  }, []);

  async function act(action: string, mission?: RatingMission, play = false) {
    if (locked.current) return;
    locked.current = true;
    const id = ++serial.current;
    setBusy(true); setError(null); setMessage(null);
    try {
      const result = await operateRatingMissions({ ...request(action), revision: shelf?.revision ?? null, missionId: mission?.id ?? null, minutes, mode, trackKeys: mission ? remaining(mission).map(t => t.trackKey) : null });
      if (id !== serial.current) return;
      setShelf(result); setMessage(result.message);
      if (action === "save") onSaved?.();
      if (play && onPlay && result.active?.status === "active") {
        const tracks = remaining(result.active);
        if (tracks.length) await onPlay(tracks);
      }
    } catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { locked.current = false; if (id === serial.current) setBusy(false); }
  }
  const active = shelf?.active;
  const hasActive = active && ["active", "unavailable"].includes(active.status);
  const changed = shelf?.generatedAt && (shelf.minutes !== minutes || shelf.mode !== mode);
  function card(mission: RatingMission, isActive = false) {
    const tracks = remaining(mission);
    const usable = ["active", "choice"].includes(mission.status) && tracks.length > 0;
    return <article className={`rating-missions__card rating-missions__card--${mission.kind}`} key={mission.id} aria-label={`${isActive ? "Current mission" : labels[mission.kind]}: ${mission.title}`}>
      <span className="rating-missions__kicker">{isActive ? "Your current mission" : labels[mission.kind]}</span>
      <h3>{mission.title}</h3>
      {mission.albumTotal > 0 && <p>{mission.tracks[0]?.albumArtist} · {mission.albumRated} of {mission.albumTotal} tracks rated</p>}
      <p className="rating-missions__time">{tracks.length} left to rate · {duration(mission.seconds)} remaining</p>
      {isActive && <><progress max={mission.tracks.length} value={mission.completedTracks} aria-label="Mission rating progress" /><p>{mission.completedTracks} of {mission.tracks.length} mission tracks rated · {mission.status === "completed" ? "Mission completed" : mission.status === "unavailable" ? "A recording is unavailable or changed. Leave this mission and find another." : tracks.length === 0 ? "All rated here. Reload after catalog sync to record completion." : "Progress comes from your saved ratings."}</p></>}
      <p>{mission.reason}</p><small>{mission.fit} · Audition guidance</small>
      {mission.anchors.length > 0 && <p className="rating-missions__anchors">Favorite sound evidence: {mission.anchors.map((track, i) => <span key={track.trackKey}>{i > 0 ? "; " : ""}<button type="button" onClick={() => onOpen(track)}>{track.title} — {track.artist}</button></span>)}</p>}
      <details><summary>Review mission tracks ({mission.tracks.length})</summary><ol>{mission.tracks.map(track => <li key={track.trackKey}><button type="button" onClick={() => onOpen(track)}>{track.title} — {track.artist}</button><span>{(track.rating ?? 0) > 0 ? `${track.rating! / 20}★` : "Unrated"} · {duration(track.seconds)}</span></li>)}</ol></details>
      <div className="rating-missions__actions">{!isActive && <button type="button" disabled={busy || !!hasActive || !usable} onClick={() => void act("start", mission, !!onPlay)}>{onPlay ? "Start & play" : "Start mission"}</button>}{isActive && onPlay && <button type="button" disabled={busy || !usable} onClick={() => void act("load", undefined, true)}>Play remaining</button>}<button type="button" disabled={busy || !usable} onClick={() => void act("save", mission)}>Save mission as playlist</button>{isActive && <button type="button" disabled={busy} onClick={() => void act("abandon", mission)}>{mission.status === "completed" ? "Clear completed mission" : "Leave mission"}</button>}</div>
    </article>;
  }
  return <section className="rating-missions" aria-labelledby="rating-missions-title" aria-busy={busy}>
    <header><div><span className="rating-missions__kicker">A little listening mission</span><h2 id="rating-missions-title">What should I rate next?</h2><p>Find an inviting way into the music you still want to rate.</p></div><button type="button" disabled={busy} onClick={() => void act("load")}>Reload progress</button></header>
    <div className="rating-missions__controls"><label>I have<select aria-label="Time for a rating mission" value={minutes} disabled={busy} onChange={e => setMinutes(Number(e.target.value))}>{[10, 20, 30, 40, 60, 90, 120, 180, 240].map(value => <option key={value} value={value}>{value} minutes</option>)}</select></label><label>Help me choose<select aria-label="Rating mission format" value={mode} disabled={busy} onChange={e => setMode(e.target.value)}><option value="albums">Albums to finish</option><option value="mix">A mix of unrated tracks</option></select></label><button type="button" disabled={busy} onClick={() => void act("refresh")}>{busy ? "Working…" : shelf?.generatedAt ? "Find new missions" : "Find my next mission"}</button></div>
    {changed && <p role="status">Find new missions to apply your time and format. The choices below keep their original plan.</p>}
    {shelf && <><p className="rating-missions__note">{shelf.note}</p>{shelf.modelAuc !== null && <details><summary>About the taste evidence</summary><p>{shelf.modelVersion} · {shelf.trainingTracks.toLocaleString()} training tracks · {shelf.validationTracks.toLocaleString()} tracks by held-out album artists · AUC {shelf.modelAuc.toFixed(3)}. All music families pooled; fit is a ranking signal, not a predicted rating.</p></details>}{active && <div className="rating-missions__current">{card(active, true)}</div>}<div className="rating-missions__grid">{shelf.choices.map(m => card(m))}</div>{shelf.generatedAt && shelf.choices.length === 0 && <p>No new choices fit this request. Try more time, a mixed mission, or analyze more music.</p>}{shelf.startedMissions > 0 && <p className="rating-missions__history">{shelf.completedMissions} of {shelf.startedMissions} started missions completed · {shelf.completedArtists} album artists · {shelf.completedNeighborhoods} favorite sound anchors reached. No rating target or deadline.</p>}</>}
    {message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
  </section>;
}
