import { invoke } from "@tauri-apps/api/core";

export type SourceState = "connected" | "unavailable" | "browser-preview";

export interface LibrarySummary {
  songs: number;
  albums: number;
  artists: number;
  genres: number;
  loved: number;
  rated: number;
}

export interface Artist {
  id: string;
  name: string;
  trackCount: number;
  albumCount: number;
  playCount: number | null;
}

export interface Track {
  id: string;
  albumId: string | null;
  title: string;
  artist: string;
  album: string;
  releaseYear: number | null;
  rating: number | null;
  loved: boolean;
  durationSeconds: number | null;
  genre: string | null;
  playCount: number | null;
}

export interface LibrarySnapshot {
  sourceState: SourceState;
  sourceLabel: string;
  sourcePath: string | null;
  summary: LibrarySummary;
  artists: Artist[];
  tracks: Track[];
}

export const browserPreview: LibrarySnapshot = {
  sourceState: "browser-preview",
  sourceLabel: "Browser preview data",
  sourcePath: null,
  summary: {
    songs: 12_846,
    albums: 1_208,
    artists: 2_302,
    genres: 186,
    loved: 914,
    rated: 4_812,
  },
  artists: [
    { id: "preview-m83", name: "M83", trackCount: 94, albumCount: 9, playCount: 4_218 },
    { id: "preview-beethoven", name: "Ludwig van Beethoven", trackCount: 312, albumCount: 28, playCount: 3_804 },
    { id: "preview-daft-punk", name: "Daft Punk", trackCount: 126, albumCount: 12, playCount: 3_116 },
    { id: "preview-gorillaz", name: "Gorillaz", trackCount: 84, albumCount: 8, playCount: 2_730 },
    { id: "preview-coldplay", name: "Coldplay", trackCount: 178, albumCount: 14, playCount: 2_414 },
    { id: "preview-college", name: "College", trackCount: 62, albumCount: 6, playCount: 2_108 },
    { id: "preview-kavinsky", name: "Kavinsky", trackCount: 47, albumCount: 5, playCount: 1_982 },
    { id: "preview-the-xx", name: "The xx", trackCount: 53, albumCount: 4, playCount: 1_755 },
  ],
  tracks: [
    { id: "preview-1", albumId: "preview-hurry-up", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", releaseYear: 2011, rating: 5, loved: true, durationSeconds: 243, genre: "Electronic", playCount: 186 },
    { id: "preview-2", albumId: "preview-drive", title: "A Real Hero", artist: "College", album: "Drive", releaseYear: 2011, rating: 4, loved: false, durationSeconds: 267, genre: "Soundtrack", playCount: 141 },
    { id: "preview-3", albumId: "preview-outrun", title: "Nightcall", artist: "Kavinsky", album: "OutRun", releaseYear: 2013, rating: 4.5, loved: true, durationSeconds: 258, genre: "Synthwave", playCount: 137 },
    { id: "preview-4", albumId: "preview-xx", title: "Intro", artist: "The xx", album: "xx", releaseYear: 2009, rating: 4, loved: false, durationSeconds: 127, genre: "Indie Rock", playCount: 129 },
    { id: "preview-5", albumId: "preview-discovery", title: "Digital Love", artist: "Daft Punk", album: "Discovery", releaseYear: 2001, rating: 5, loved: true, durationSeconds: 301, genre: "House", playCount: 122 },
    { id: "preview-6", albumId: "preview-plastic-beach", title: "On Melancholy Hill", artist: "Gorillaz", album: "Plastic Beach", releaseYear: 2010, rating: 4.5, loved: true, durationSeconds: 233, genre: "Alternative", playCount: 116 },
    { id: "preview-7", albumId: "preview-viva", title: "Strawberry Swing", artist: "Coldplay", album: "Viva la Vida", releaseYear: 2008, rating: 4, loved: false, durationSeconds: 249, genre: "Alternative", playCount: 108 },
  ],
};

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function albumCoverUrl(albumId: string | null, size: 64 | 128 | 256 | 512): string | null {
  if (!albumId || !isTauriRuntime()) return null;
  return `http://aurora-cover.localhost/album/${encodeURIComponent(albumId)}?size=${size}`;
}

export async function loadLibrarySnapshot(): Promise<LibrarySnapshot> {
  if (!isTauriRuntime()) {
    return browserPreview;
  }

  return invoke<LibrarySnapshot>("library_snapshot");
}

export async function loadArtistTracks(artist: string): Promise<Track[]> {
  if (!isTauriRuntime()) {
    return browserPreview.tracks.filter((track) => track.artist === artist);
  }
  return invoke<Track[]>("artist_tracks", { artist });
}

export async function searchLibraryTracks(query: string): Promise<Track[]> {
  if (!isTauriRuntime()) {
    return filterTracks(browserPreview.tracks, query, null);
  }
  return invoke<Track[]>("search_tracks", { query });
}

export function formatCount(value: number): string {
  return new Intl.NumberFormat(undefined).format(value);
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds < 0) return "—";
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return `${minutes}:${remainder.toString().padStart(2, "0")}`;
}

export function filterTracks(tracks: Track[], query: string, artist: string | null): Track[] {
  const normalized = query.trim().toLocaleLowerCase();
  return tracks.filter((track) => {
    if (artist && track.artist !== artist) return false;
    if (!normalized) return true;
    return [track.title, track.artist, track.album, track.genre ?? ""]
      .join("\u0000")
      .toLocaleLowerCase()
      .includes(normalized);
  });
}
