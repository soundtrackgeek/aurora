export function albumArtistSearchQuery(artist: string): string {
  const escaped = artist.trim().replace(/"/g, '""');
  return `aartist:"${escaped}"`;
}
