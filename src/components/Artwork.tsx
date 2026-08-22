import { AudioLines } from "lucide-react";
import { type CSSProperties, useMemo, useState } from "react";
import { albumCoverUrl, type Track } from "../library";

export function Artwork({
  track,
  size = "small",
  decorative = true,
}: {
  track: Track;
  size?: "small" | "player" | "large";
  decorative?: boolean;
}) {
  const source = albumCoverUrl(track.albumId, size === "large" ? 512 : size === "player" ? 128 : 64);
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const { initials, seed } = useMemo(() => {
    const nextSeed = [...track.artist].reduce(
      (sum, character) => sum + (character.codePointAt(0) ?? 0),
      0,
    );
    const words = track.artist.match(/[\p{L}\p{N}]+/gu) ?? ["?"];
    const nextInitials = words.length === 1
      ? words[0].slice(0, 2).toLocaleUpperCase()
      : words.slice(0, 2).map((word) => word[0]).join("").toLocaleUpperCase();
    return { initials: nextInitials, seed: nextSeed };
  }, [track.artist]);

  return (
    <div
      className={`artwork artwork--${size}`}
      style={{ "--art-seed": seed } as CSSProperties}
      aria-hidden={decorative || undefined}
      aria-label={decorative ? undefined : `${track.album} cover`}
    >
      {source && source !== failedSource ? (
        <img
          className="artwork__image"
          src={source}
          alt={decorative ? "" : `${track.album} cover`}
          onError={() => setFailedSource(source)}
        />
      ) : (
        <>
          <span>{initials}</span>
          <AudioLines />
        </>
      )}
    </div>
  );
}
