import { useEffect, useRef, useState } from "react";
import { operateSonicAtlas, searchAtlasTracks, type AtlasRequest, type SonicAtlas as AtlasData, type SonicTrack, type AtlasCounts } from "../sonicAtlas";
import "./SonicAtlas.css";

interface Props {
  initialSeed?: { trackKey: string; title: string };
  onClose: () => void;
  onOpenAlbum: (track: SonicTrack) => void;
  onPlay?: (tracks: SonicTrack[]) => Promise<void>;
  onPause?: () => void;
  playing?: boolean;
  onSaved?: () => void;
}
function countLine(c: AtlasCounts) {
  return `${c.owned} owned · ${c.rated} rated · ${c.loved} loved · ${c.absentFromHistory} absent from available history`;
}
export function SonicAtlas({ initialSeed, onClose, onOpenAlbum, onPlay, onPause, playing, onSaved }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const serial = useRef(0);
  const locked = useRef(false);
  const [atlas, setAtlas] = useState<AtlasData | null>(null);
  const [regionId, setRegionId] = useState<string | null>(null);
  const [trail, setTrail] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [choices, setChoices] = useState<SonicTrack[]>([]);
  const [searched, setSearched] = useState(false);
  const [busy, setBusy] = useState(Boolean(initialSeed));
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const region = atlas?.regions.find(r => r.id === regionId) ?? atlas?.regions[0];
  const representative = region?.points.find(p => p.track.trackKey === region.representativeKey);
  const initialKey = initialSeed?.trackKey;

  useEffect(() => {
    const requests = serial;
    dialog.current?.showModal();
    if (initialKey) {
      locked.current = true;
      const id = ++requests.current;
      operateSonicAtlas({ action: "load", seedKey: initialKey, revision: null, regionId: null, trackKeys: null, name: null })
        .then(result => { if (id === requests.current) setAtlas(result); })
        .catch(e => { if (id === requests.current) setError(String(e)); })
        .finally(() => { if (id === requests.current) { locked.current = false; setBusy(false); } });
    }
    return () => { requests.current++; };
  }, [initialKey]);

  async function act(action: string, seedKey = atlas?.seed.trackKey, tracks?: SonicTrack[], back = false) {
    if (!seedKey || locked.current) return;
    locked.current = true;
    const id = ++serial.current;
    setBusy(true); setError(null);
    const request: AtlasRequest = { action, seedKey, revision: atlas?.revision ?? null, regionId: region?.id ?? null, trackKeys: tracks?.map(t => t.trackKey) ?? null, name: name.trim() || `Sonic atlas — ${region?.label ?? "region"}` };
    try {
      const result = await operateSonicAtlas(request);
      if (id !== serial.current) return;
      if (seedKey !== atlas?.seed.trackKey) {
        setTrail(old => back ? old.slice(0, -1) : atlas ? [...old, atlas.seed.trackKey] : old);
        setRegionId(null); setName(""); setChoices([]); setSearched(false);
      }
      setAtlas(result);
      if (action === "save") onSaved?.();
      if (action === "audition" && tracks && onPlay) {
        const points = result.regions.flatMap(r => r.points);
        const current = tracks.map(t => points.find(p => p.track.trackKey === t.trackKey && p.available)?.track);
        if (current.some(t => !t)) throw new Error("A reviewed track is unavailable. Reload the region.");
        await onPlay(current as SonicTrack[]);
      }
    } catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { locked.current = false; if (id === serial.current) setBusy(false); }
  }
  async function search() {
    if (locked.current) return;
    const id = ++serial.current;
    setBusy(true); setError(null); locked.current = true;
    try {
      const tracks = await searchAtlasTracks(query);
      if (id === serial.current) { setChoices(tracks.map(t => ({ ...t, distance: null }))); setSearched(true); }
    } catch (e) { if (id === serial.current) setError(String(e)); }
    finally { locked.current = false; if (id === serial.current) setBusy(false); }
  }
  function selectRegion(id: string) { setRegionId(id); setName(""); }
  const selectedTracks = region?.points.filter(p => p.available).map(p => p.track) ?? [];
  return <dialog ref={dialog} className="sonic-atlas" aria-label="Playable sonic atlas" onCancel={onClose}>
    <header><div><span className="sonic-atlas__eyebrow">Your collection, by sound</span><h2>Playable sonic atlas</h2></div><div className="sonic-atlas__actions">{onPause && <button type="button" disabled={!playing} onClick={onPause}>Pause playback</button>}<button type="button" onClick={onClose} aria-label="Close sonic atlas">Close</button></div></header>
    <p>Start with a track. Explore its sound neighbors through real recordings.</p>
    <details className="sonic-atlas__finder" open={!initialSeed && !atlas}><summary>{atlas ? "Find another starting track" : "Choose a starting track"}</summary>
    <form className="sonic-atlas__search" onSubmit={e => { e.preventDefault(); void search(); }}><label>Find an analyzed track<input value={query} maxLength={200} disabled={busy} onChange={e => { setQuery(e.target.value); setChoices([]); setSearched(false); }} placeholder="Artist or title" /></label><button type="submit" disabled={busy || query.trim().length < 2}>Search tracks</button></form>
    {choices.length > 0 && <ul className="sonic-atlas__choices">{choices.map(t => <li key={t.trackKey}><button type="button" disabled={busy} onClick={() => void act("load", t.trackKey)}>{t.title} <span>{t.artist} · {t.album}</span></button></li>)}</ul>}
    {searched && !choices.length && <p role="status">No current analyzed tracks match. Try another title or analyze more music.</p>}
    </details>
    {!atlas && initialSeed && !error && <p role="status">Opening the neighborhood around {initialSeed.title}…</p>}
    {atlas && <>
      <div className="sonic-atlas__location"><div><span>Centered on</span><h3>{atlas.seed.title}</h3><p>{atlas.seed.artist} · {atlas.seed.album}</p></div><div className="sonic-atlas__actions"><button type="button" disabled={busy || !trail.length} onClick={() => void act("load", trail[trail.length - 1], undefined, true)}>Back</button><button type="button" disabled={busy} onClick={() => void act("load")}>Reload counts</button><button type="button" disabled={busy} onClick={() => void act("rebuild")}>Rebuild neighborhood</button></div></div>
      <p className="sonic-atlas__summary">{countLine(atlas.counts)}</p>
      {atlas.profile.startsWith("Browser") && <p className="sonic-atlas__notice">{atlas.profile}. Playback and saved playlists are previews.</p>}
      <div className="sonic-atlas__layout">
        <figure className="sonic-atlas__figure"><div className="sonic-atlas__plot"><span className="sonic-atlas__level">Higher level</span>
          <svg viewBox="0 0 600 360" role="img" aria-label="Sound map: brightness increases left to right; average frame level increases upward. Choose a numbered region in the adjacent list.">
            {[.25,.5,.75].map(t => <g key={t} className="sonic-atlas__grid"><line x1={35 + t * 530} x2={35 + t * 530} y1="25" y2="335" /><line x1="35" x2="565" y1={25+t*310} y2={25+t*310} /></g>)}
            {atlas.regions.map((r,i) => <g key={r.id} className={`sonic-atlas__marks sonic-atlas__color-${i % 6}${r.id === region?.id ? " is-selected" : ""}`} aria-hidden="true">
              {r.points.map(p => p.x !== null && p.y !== null ? <circle key={p.track.trackKey} cx={35+p.x*530} cy={335-p.y*310} r={p.track.trackKey === atlas.seed.trackKey ? 6 : 3.5} opacity={p.available ? 1 : .2} onClick={() => selectRegion(r.id)}><title>{p.track.title} · {p.track.artist}</title></circle> : null)}
              {r.points.filter(p => p.track.trackKey === r.representativeKey).map(p => p.x !== null && p.y !== null ? <g key={p.track.trackKey} onClick={() => selectRegion(r.id)}><circle className="sonic-atlas__marker" cx={35+p.x*530} cy={335-p.y*310} r="13"/><text x={35+p.x*530} y={335-p.y*310+4} textAnchor="middle">{i+1}</text></g> : null)}
            </g>)}
          </svg><span className="sonic-atlas__level">Lower level</span><div className="sonic-atlas__axis"><span>Darker</span><span>Brightness →</span><span>Brighter</span></div>
        </div><figcaption>Positions show spectral brightness and average frame level within this neighborhood. Regions and neighbors use the full sound metric; closeness on this map is only part of the picture. Level is not LUFS, mood, or a volume setting.</figcaption></figure>
        <nav className="sonic-atlas__regions" aria-label="Sound regions">{atlas.regions.map((r,i) => <button type="button" key={r.id} className={`sonic-atlas__region sonic-atlas__color-${i % 6}`} aria-pressed={r.id === region?.id} onClick={() => selectRegion(r.id)}><b>{i+1}</b><span><strong>{r.label}</strong><small>{r.counts.owned} tracks · {r.counts.rated} rated · {r.counts.loved} loved</small><small>{r.counts.absentFromHistory} absent from available history</small></span></button>)}</nav>
      </div>
      {region && <section className="sonic-atlas__detail" aria-label="Selected sound region"><div className="sonic-atlas__entry"><div><span className="sonic-atlas__eyebrow">Your entry point</span><h3>{representative?.track.title}</h3><p>{representative?.track.artist} · {representative?.track.album}</p></div><div className="sonic-atlas__actions">{representative?.available && <>{onPlay && <button type="button" disabled={busy} onClick={() => void act("audition", undefined, [representative.track])}>Play entry point</button>}<button type="button" onClick={() => { onOpenAlbum(representative.track); onClose(); }}>Open album</button><button type="button" disabled={busy || representative.track.trackKey === atlas.seed.trackKey} onClick={() => void act("load", representative.track.trackKey)}>Center here</button></>}{!representative?.available && <p>This entry point is unavailable. Choose another track below.</p>}</div></div>
        <p>{countLine(region.counts)}</p>
        <details open><summary>Review region tracks ({selectedTracks.length})</summary><ol className="sonic-atlas__tracks">{region.points.map(p => <li key={p.track.trackKey}><div><strong>{p.track.title}</strong><span>{p.track.artist} · {p.track.album}</span><small>{!p.available ? "Unavailable or changed" : `${(p.track.rating ?? 0) > 0 ? `${p.track.rating! / 20}★` : "Unrated"}${p.track.loved ? " · Loved" : ""} · ${p.inHistory ? "In available history" : "Absent from available history"}`}</small></div><div className="sonic-atlas__actions">{onPlay && <button type="button" disabled={busy || !p.available} aria-label={`Play ${p.track.title}`} onClick={() => void act("audition", undefined, [p.track])}>Play</button>}<button type="button" disabled={!p.available} aria-label={`Open album for ${p.track.title}`} onClick={() => { onOpenAlbum(p.track); onClose(); }}>Album</button><button type="button" disabled={busy || !p.available || p.track.trackKey === atlas.seed.trackKey} aria-label={`Center on ${p.track.title}`} onClick={() => void act("load", p.track.trackKey)}>Center</button></div></li>)}</ol></details>
        <div className="sonic-atlas__save"><label>Playlist name<input value={name} maxLength={120} disabled={busy} placeholder={`Sonic atlas — ${region.label}`} onChange={e => setName(e.target.value)} /></label><div className="sonic-atlas__actions">{onPlay && <button type="button" disabled={busy || !selectedTracks.length} onClick={() => void act("audition", undefined, selectedTracks)}>Play region</button>}<button type="button" disabled={busy || !selectedTracks.length} onClick={() => void act("save", undefined, selectedTracks)}>Save region playlist</button></div></div>
      </section>}
      {atlas.counts.unavailable > 0 && <p role="status">{atlas.counts.unavailable} saved points are unavailable or changed. They keep their positions but cannot be played or saved. Rebuild to find new neighbors.</p>}
      <p className="sonic-atlas__notice">Up to 180 distinct recordings from 4,096 sound candidates; counts describe this neighborhood, not your whole collection. Available history contains {atlas.historyPlays.toLocaleString()} plays{atlas.historySince ? ` since ${new Date(atlas.historySince * 1000).toLocaleDateString()}` : ""}; absence does not mean never heard. The map stays fixed until you rebuild.</p>
      <details className="sonic-atlas__notice"><summary>Map version and evidence</summary><p>{atlas.version} · {atlas.profile} · revision {atlas.revision} · built {new Date(atlas.generatedAt).toLocaleString()}. Uses the existing analysis profile's weights. No new audio decoding or learned mapping metric.</p></details>
      {atlas.message && <p role="status">{atlas.message}</p>}
    </>}
    {!atlas && initialKey && error && <button type="button" disabled={busy} onClick={() => void act("load", initialKey)}>Retry neighborhood</button>}
    {busy && <p role="status">Working…</p>}{error && <p role="alert">{error}</p>}
  </dialog>;
}
