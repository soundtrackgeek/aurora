import { useContext, type KeyboardEvent, type MouseEvent } from "react";
import { MediaMenuContext, mediaMenuHandlers } from "./context/mediaMenuContext";
import { loadAlbumArtistIdentity } from "../library";
import "./ArtistSmartLink.css";

interface ArtistSmartLinkProps {
  artist: string;
  artistId?: string | null;
  disambiguation?: string | null;
  albumId?: string | null;
  onOpen: (artist: string) => void;
  nested?: boolean;
  className?: string;
}

export function ArtistSmartLink({ artist, artistId, disambiguation, albumId, onOpen, nested = false, className }: ArtistSmartLinkProps) {
  const trimmedArtist = artist.trim();
  const classes = ["artist-smart-link", className].filter(Boolean).join(" ");
  const reference = artistId ?? trimmedArtist;
  const label = `Open artist page for ${trimmedArtist}${disambiguation ? ` (${disambiguation})` : ""}`;
  const menu = mediaMenuHandlers(useContext(MediaMenuContext), { artistName: reference, artistOnly: true, label: trimmedArtist });

  function activate(event: MouseEvent | KeyboardEvent) {
    event.stopPropagation();
    if (!trimmedArtist) return;
    if (!artistId && albumId) {
      void loadAlbumArtistIdentity(albumId).then(onOpen).catch(() => onOpen(reference));
    } else onOpen(reference);
  }

  if (nested) {
    return (
      <span
        onContextMenu={menu.onContextMenu}
        className={classes}
        role="link"
        tabIndex={0}
        title={label}
        aria-label={label}
        onClick={activate}
        onDoubleClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          menu.onKeyDown(event);
          if (event.defaultPrevented) return;
          if (event.key !== "Enter" && event.key !== " ") return;
          event.preventDefault();
          activate(event);
        }}
      >
        {artist}
      </span>
    );
  }

  return (
    <button
      {...menu}
      type="button"
      className={classes}
      title={label}
      aria-label={label}
      onClick={activate}
    >
      {artist}
    </button>
  );
}
