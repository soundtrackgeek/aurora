import { ArtistSmartLink } from "../ArtistSmartLink";
import "./HistoryArtistLink.css";

export function HistoryArtistLink({ artist, artistId, artistDisambiguation, onOpen, nested = false }: {
  artist: string; artistId?: string | null; artistDisambiguation?: string | null;
  onOpen: (artist: string) => void; nested?: boolean;
}) {
  return <span className="history-artist-label">
    {!artistId && artistDisambiguation === "Unresolved artist" ? <span>{artist}</span>
      : <ArtistSmartLink artist={artist} artistId={artistId} disambiguation={artistDisambiguation} onOpen={onOpen} nested={nested} />}
    {artistDisambiguation && <span className="history-artist-description"> · {artistDisambiguation}</span>}
  </span>;
}
