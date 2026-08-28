import { useMemo, useState } from "react";
import { artistPortraitUrl } from "../library";
import "./ArtistPortrait.css";

function initialsForArtist(artist: string): string {
  const words = artist.match(/[\p{L}\p{N}]+/gu) ?? ["?"];
  return words.length === 1
    ? words[0].slice(0, 2).toLocaleUpperCase()
    : words.slice(0, 2).map((word) => word[0]).join("").toLocaleUpperCase();
}

export function ArtistPortrait({
  artist,
  className,
  size = 64,
  eager = false,
}: {
  artist: string;
  className?: string;
  size?: 64 | 128;
  eager?: boolean;
}) {
  const source = artistPortraitUrl(artist, size);
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const initials = useMemo(() => initialsForArtist(artist), [artist]);

  return (
    <span className={`artist-portrait${className ? ` ${className}` : ""}`} aria-hidden="true">
      <span className="artist-portrait__fallback">{initials}</span>
      {source && source !== failedSource ? (
        <img
          src={source}
          alt=""
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          onError={() => setFailedSource(source)}
        />
      ) : null}
    </span>
  );
}
