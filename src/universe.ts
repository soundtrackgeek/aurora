import { invoke } from "@tauri-apps/api/core";
import { loadHistoryPage } from "./history";
import { exploreAlbums, isTauriRuntime, type Track } from "./library";

export type UniversePeriod = "recent" | "all";

export interface UniverseArtist {
  id: string;
  name: string;
  disambiguation?: string | null;
  plays: number;
}

export interface RediscoverTrack {
  track: Track;
  lastListenedAtMs: number;
}

export async function loadUniverseArtists(period: UniversePeriod): Promise<UniverseArtist[]> {
  if (isTauriRuntime()) return invoke("universe_top_artists", { recentOnly: period === "recent" });
  const startedAfterMs = period === "recent" ? Date.now() - 90 * 86_400_000 : undefined;
  const page = await loadHistoryPage({ pageSize: 100, outcome: "played", startedAfterMs });
  const artists = new Map<string, UniverseArtist>();
  for (const item of page.items) {
    const id = item.artistId ?? item.artist;
    const key = id.toLocaleLowerCase();
    const existing = artists.get(key);
    if (existing) existing.plays += 1;
    else artists.set(key, { id, name: item.artist, disambiguation: item.artistDisambiguation, plays: 1 });
  }
  return [...artists.values()].sort((a, b) => b.plays - a.plays || a.name.localeCompare(b.name)).slice(0, 8);
}

export async function loadNearFinalAlbums() {
  return (await exploreAlbums({ pageSize: 4, nearFinalRank: true }, { localOnly: true })).items;
}

export async function loadRecentAlbums() {
  return (await exploreAlbums({ pageSize: 4, sort: "newest", inboxAdditionsOnly: true }, { localOnly: true })).items;
}

export async function loadRediscoverTracks(): Promise<RediscoverTrack[]> {
  if (isTauriRuntime()) return invoke("universe_rediscover");
  // The browser preview has recent history only.
  return [];
}
