import { useContext, type KeyboardEvent, type MouseEvent } from "react";
import { MediaMenuContext, mediaMenuHandlers } from "./context/mediaMenuContext";
import "./ArtistSmartLink.css";

interface ArtistSmartLinkProps {
  artist: string;
  onOpen: (artist: string) => void;
  nested?: boolean;
  className?: string;
}

export function ArtistSmartLink({ artist, onOpen, nested = false, className }: ArtistSmartLinkProps) {
  const trimmedArtist = artist.trim();
  const classes = ["artist-smart-link", className].filter(Boolean).join(" ");
  const menu = mediaMenuHandlers(useContext(MediaMenuContext), { artistName: trimmedArtist, artistOnly: true, label: trimmedArtist });

  function activate(event: MouseEvent | KeyboardEvent) {
    event.stopPropagation();
    if (trimmedArtist) onOpen(trimmedArtist);
  }

  if (nested) {
    return (
      <span
        onContextMenu={menu.onContextMenu}
        className={classes}
        role="link"
        tabIndex={0}
        title={`Open artist page for ${trimmedArtist}`}
        aria-label={`Open artist page for ${trimmedArtist}`}
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
      title={`Open artist page for ${trimmedArtist}`}
      aria-label={`Open artist page for ${trimmedArtist}`}
      onClick={activate}
    >
      {artist}
    </button>
  );
}
