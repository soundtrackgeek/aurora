import { invoke } from "@tauri-apps/api/core";
import { exploreTracks, isTauriRuntime, type Track } from "./library";
import type { ArtistProfile } from "./musicbrainz";

export interface PopularArtistTrack { name: string; playCount: number | null; listeners: number | null; url: string | null }
export interface ArtistDiscovery {
  biography: string;
  listeners: number | null;
  playCount: number | null;
  tags: string[];
  topTracks: PopularArtistTrack[];
  similarArtists: { name: string; matchScore: number | null }[];
  warnings: string[];
}
export interface ArtistArtwork { backgroundUrl: string | null; portraitUrl: string | null; warning: string | null }
export interface ArtistListening {
  plays: number;
  listenedSeconds: number;
  months: Record<string, number>;
  topTracks: { trackKey: string; title: string; album: string; plays: number }[];
}
export interface FanartSettings { configured: boolean; personalKeyConfigured: boolean }
export type FanartCredentials = { mode: "save"; apiKey: string; personalKey: string | null } | { mode: "clear" };

export async function loadArtistDiscovery(artist: string, refresh = false): Promise<ArtistDiscovery> {
  if (!isTauriRuntime()) return { biography: "", listeners: null, playCount: null, tags: [], topTracks: [], similarArtists: [], warnings: ["Last.fm discovery is available in the desktop app. Configure it in Settings → Metadata."] };
  return invoke("artist_discovery", { artist, refresh });
}
export async function loadArtistArtwork(artist: string, refresh = false): Promise<ArtistArtwork> {
  if (!isTauriRuntime()) return { backgroundUrl: null, portraitUrl: null, warning: "fanart.tv artwork is available in the desktop app." };
  return invoke("artist_artwork", { artist, refresh });
}
export async function loadArtistListening(artist: string): Promise<ArtistListening> {
  if (!isTauriRuntime()) return { plays: 0, listenedSeconds: 0, months: {}, topTracks: [] };
  return invoke("artist_listening", { artist });
}
export async function loadFanartSettings(): Promise<FanartSettings> {
  if (!isTauriRuntime()) return { configured: false, personalKeyConfigured: false };
  return invoke("fanart_settings");
}
export async function saveFanartCredentials(request: FanartCredentials): Promise<FanartSettings> {
  if (!isTauriRuntime()) throw new Error("Save credentials in the Aurora desktop app.");
  return invoke("update_fanart_credentials", { request });
}

export async function openArtistLink(url: string): Promise<void> {
  if (isTauriRuntime()) return invoke("open_artist_link", { url });
  window.open(url, "_blank", "noopener,noreferrer");
}

export function artistFacts(profile: ArtistProfile | null): [string, string][] {
  if (!profile) return [];
  const person = profile.artistType?.toLowerCase() === "person";
  return [
    ["Origin", profile.countryName ?? profile.countryCode],
    [person ? "Birthplace" : "Formed in", profile.beginAreaName],
    [person ? "Born" : "Founded", profile.lifeBeginDate],
    [person ? "Died" : "Dissolved", profile.lifeEndDate ?? (profile.lifeEnded ? "Date unknown" : null)],
    ["Type", profile.artistType],
  ].filter((fact): fact is [string, string] => Boolean(fact[1]));
}

/** Provider HTML is converted to text; it is never inserted into the document. */
export function biographyText(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("script, style, a").forEach((element) => element.remove());
  return (doc.body.textContent ?? "").replace(/\s+/g, " ").trim();
}

export function listeningMonths(months: Record<string, number>, now = new Date()) {
  return Array.from({ length: 12 }, (_, i) => {
    const date = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - 11 + i, 1));
    const key = date.toISOString().slice(0, 7);
    return { key, label: date.toLocaleDateString(undefined, { month: "short", year: "numeric", timeZone: "UTC" }), plays: months[key] ?? 0 };
  });
}

export async function findArtistTrack(artist: string, title: string): Promise<Track | null> {
  const page = await exploreTracks({ artist, search: `title:"${title.replace(/"/g, '""')}"`, pageSize: 100, sort: "albumAsc" }, { localOnly: true });
  return page.items.find((track) => track.title.trim().toLocaleLowerCase() === title.trim().toLocaleLowerCase()) ?? null;
}
