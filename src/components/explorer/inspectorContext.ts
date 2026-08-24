import { displayTrackArtist, type Track } from "../../library";

interface InspectorAlbum {
  id: string;
  artist: string;
}

export interface ExplorerAlbumInspectorContext<TAlbum extends InspectorAlbum> {
  album: TAlbum;
  track: Track | null;
  artistName: string;
}

export function resolveExplorerAlbumInspectorContext<TAlbum extends InspectorAlbum>(
  albums: readonly TAlbum[],
  selectedAlbumId: string | null,
  albumTracks: readonly Track[],
  selectedTrack: Track | null,
): ExplorerAlbumInspectorContext<TAlbum> | null {
  const album = albums.find((candidate) => candidate.id === selectedAlbumId) ?? null;
  if (!album) return null;
  const track = albumTracks.find((candidate) => candidate.id === selectedTrack?.id)
    ?? albumTracks[0]
    ?? null;
  return { album, track, artistName: track ? displayTrackArtist(track) : album.artist };
}
