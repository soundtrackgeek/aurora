import type { KeyboardEvent, MouseEvent } from "react";
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

  function activate(event: MouseEvent | KeyboardEvent) {
    event.stopPropagation();
    if (trimmedArtist) onOpen(trimmedArtist);
  }

  if (nested) {
    return (
      <span
        className={classes}
        role="link"
        tabIndex={0}
        title={`Show albums by ${trimmedArtist}`}
        aria-label={`Show albums by ${trimmedArtist}`}
        onClick={activate}
        onDoubleClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
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
      type="button"
      className={classes}
      title={`Show albums by ${trimmedArtist}`}
      aria-label={`Show albums by ${trimmedArtist}`}
      onClick={activate}
    >
      {artist}
    </button>
  );
}
