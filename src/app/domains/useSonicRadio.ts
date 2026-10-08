import { useEffect, useRef, useState } from "react";
import { sonicMatches, type SonicRequest, type SonicAlbum } from "../../sonic";
import { type Track } from "../../library";
import { type usePlayback } from "../../playback";

interface Session { version: 1; seedKey: string; seedAlbumId?: string; minimumCoverage?: number; seedTitle: string; minimumRating: number | null; sameGenre: boolean; played: string[]; }
const STORAGE = "aurora:sonic-radio:v1";
function restore(): Session | null {
  try { const value: unknown = JSON.parse(localStorage.getItem(STORAGE) ?? "null");
    if (!value || typeof value !== "object") return null;
    const s = value as Partial<Session>;
    return s.version === 1 && typeof s.seedKey === "string" && s.seedKey.length <= 4096 && (s.seedAlbumId === undefined || typeof s.seedAlbumId === "string" && s.seedAlbumId.length > 0 && s.seedAlbumId.length <= 4096) && (s.minimumCoverage === undefined || typeof s.minimumCoverage === "number" && Number.isInteger(s.minimumCoverage) && s.minimumCoverage >= 50 && s.minimumCoverage <= 100) && typeof s.seedTitle === "string" && typeof s.sameGenre === "boolean" && (s.minimumRating === null || typeof s.minimumRating === "number" && Number.isFinite(s.minimumRating) && s.minimumRating >= 0 && s.minimumRating <= 5) && Array.isArray(s.played) && s.played.length <= 2000 && s.played.every(k => typeof k === "string" && k.length <= 4096) ? s as Session : null;
  } catch { return null; }
}
function save(session: Session | null) { try { if (session) localStorage.setItem(STORAGE, JSON.stringify(session)); else localStorage.removeItem(STORAGE); } catch { /* Playback remains available without browser storage. */ } }

export function useSonicRadio(playback: Pick<ReturnType<typeof usePlayback>, "play" | "append"> & { state: Pick<ReturnType<typeof usePlayback>["state"], "queue" | "currentIndex"> }) {
  const [session, setSession] = useState<Session | null>(restore);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const serial = useRef(0);
  const filling = useRef(false);
  const current = useRef(session);
  const failureKey = useRef<string | null>(null);
  function install(next: Session | null) { current.current = next; setSession(next); save(next); }
  function stop() { serial.current++; filling.current = false; failureKey.current = null; install(null); setBusy(false); setMessage(null); }
  function request(s: Session): SonicRequest { return { seedKey: s.seedAlbumId ? "" : s.seedKey, ...(s.seedAlbumId ? { seedAlbumId: s.seedAlbumId, minimumCoverage: s.minimumCoverage ?? 50 } : {}), excludeKeys: s.played, limit: 100, minimumRating: s.minimumRating, sameGenre: s.sameGenre, radio: true }; }
  async function start(seed: Track, minimumRating: number | null, sameGenre: boolean) {
    return startSession({ version: 1, seedKey: seed.trackKey, seedTitle: seed.title, minimumRating, sameGenre, played: [seed.trackKey] });
  }
  async function startAlbum(seed: SonicAlbum, minimumRating: number | null, sameGenre: boolean, minimumCoverage: number) {
    return startSession({ version: 1, seedKey: seed.albumId, seedAlbumId: seed.albumId, minimumCoverage, seedTitle: seed.title, minimumRating, sameGenre, played: [] });
  }
  async function startSession(next: Session) {
    install(null);
    const id = ++serial.current; filling.current = true; setBusy(true); setMessage(null);
    try {
      const result = await sonicMatches(request(next));
      if (serial.current !== id) return;
      if (!result.seedReady) { setMessage("Analyze this seed in Music Library before starting sonic radio."); return; }
      if (!result.tracks.length) { setMessage(`No eligible matches among ${result.analyzed.toLocaleString()} analyzed tracks. Analyze more music in Music Library or change the filters.`); return; }
      const snapshot = await playback.play(result.tracks, result.tracks[0].id);
      if (serial.current !== id || !snapshot) return;
      next.played.push(...result.tracks.map(t => t.trackKey)); install(next); failureKey.current = null;
      setMessage(`Radio from ${next.seedTitle} · ${result.analyzed.toLocaleString()} analyzed tracks`);
    } catch (error) { if (serial.current === id) setMessage(error instanceof Error ? error.message : String(error)); }
    finally { if (serial.current === id) { filling.current = false; setBusy(false); } }
  }
  useEffect(() => {
    if (!session || playback.state.currentIndex === null || !playback.state.queue.length) return;
    if (!session.played.includes(playback.state.queue[playback.state.currentIndex]?.trackKey)) return;
    const remaining = playback.state.queue.length - playback.state.currentIndex - 1;
    const key = `${session.seedKey}:${session.played.length}:${playback.state.currentIndex}`;
    if (remaining >= 20 || filling.current || failureKey.current === key) return;
    if (session.played.length >= 1900) return; // Start a new station after this bounded session.
    filling.current = true;
    const id = serial.current;
    const next = { ...session, played: [...new Set([...session.played, ...playback.state.queue.map(t => t.trackKey)])].slice(0, 2000) };
    void sonicMatches(request(next)).then(async result => {
      if (id !== serial.current || current.current?.seedKey !== next.seedKey) return;
      if (!result.tracks.length) { failureKey.current = key; setMessage("Sonic radio has reached the end of the eligible analyzed music."); return; }
      const snapshot = await playback.append(result.tracks);
      if (id !== serial.current || !snapshot) return;
      next.played = [...new Set([...next.played, ...result.tracks.map(t => t.trackKey)])].slice(0, 2000);
      current.current = next; setSession(next); save(next);
    }).catch((error: unknown) => { if (id === serial.current) { failureKey.current = key; setMessage(error instanceof Error ? error.message : String(error)); } })
      .finally(() => { if (id === serial.current) filling.current = false; });
  }, [session, playback]);
  return { session, busy, message, start, startAlbum, stop };
}
