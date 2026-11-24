import { ArrowUpRight, AudioLines, Compass, Disc3, Play, Star } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useAlbumCoverUrl } from "../../albumArtwork";
import { Artwork } from "../../components/Artwork";
import { displayTrackArtist, formatCount, formatDuration, type AlbumSummary, type Track } from "../../library";
import type { PlaybackSnapshot } from "../../playback";
import { loadNearFinalAlbums, loadRecentAlbums, loadRediscoverTracks, loadUniverseArtists, type UniversePeriod } from "../../universe";

function useShelf<T>(load: () => Promise<T[]>, revision: string | number) {
  const [retry, setRetry] = useState(0);
  const [result, setResult] = useState<{ load: typeof load; items: T[] | null; failed: boolean }>();
  useEffect(() => {
    let active = true;
    void load().then((items) => {
      if (active) setResult({ load, items, failed: false });
    }, () => {
      if (active) setResult((previous) => ({
        load, items: previous?.load === load ? previous.items : null, failed: true,
      }));
    });
    return () => { active = false; };
  }, [load, revision, retry]);
  return {
    items: result?.load === load ? result.items : null,
    failed: result?.load === load && result.failed,
    retry: () => setRetry((count) => count + 1),
  };
}

const lastHeardDate = new Intl.DateTimeFormat(undefined, { month: "short", year: "numeric" });

interface UniverseProps {
  period: UniversePeriod;
  onPeriodChange: (period: UniversePeriod) => void;
  catalogRevision: number;
  historyRevision: number;
  playback: Pick<PlaybackSnapshot, "currentTrack" | "queue" | "status" | "positionSeconds" | "blindTastingLabel">;
  playbackBusy: boolean;
  onResume: () => void;
  onSelectArtist: (artist: { id: string }) => void;
  onOpenAlbum: (id: string, title: string) => void;
  onPlayTrack: (track: Track, queue: Track[]) => void;
}

export function Universe({
  period, onPeriodChange, catalogRevision, historyRevision, playback, playbackBusy,
  onResume, onSelectArtist, onOpenAlbum, onPlayTrack,
}: UniverseProps) {
  const loadArtists = useCallback(() => loadUniverseArtists(period), [period]);
  const artists = useShelf(loadArtists, `${catalogRevision}:${historyRevision}`);
  const nearFinal = useShelf(loadNearFinalAlbums, catalogRevision);
  const recent = useShelf(loadRecentAlbums, catalogRevision);
  const rediscover = useShelf(loadRediscoverTracks, `${catalogRevision}:${historyRevision}`);
  const currentTrack = playback.currentTrack;
  const playing = playback.status === "playing";
  const hideTrack = Boolean(playback.blindTastingLabel);

  return (
    <>
      <section className="universe" aria-labelledby="universe-title">
        <div className="universe__heading">
          <h1 id="universe-title">Your listening universe</h1>
          <p>Top artists by registered plays.</p>
          {artists.failed ? <button className="universe-retry" type="button" onClick={artists.retry}>Retry listening history</button>
            : artists.items?.length === 0 ? <p className="universe__status">No registered plays in this period.</p>
              : artists.items === null ? <p className="universe__status" role="status">Loading listening history…</p> : null}
        </div>
        <div className="universe-period" role="group" aria-label="Listening period">
          <button type="button" aria-pressed={period === "recent"} onClick={() => onPeriodChange("recent")}>90 days</button>
          <button type="button" aria-pressed={period === "all"} onClick={() => onPeriodChange("all")}>All time</button>
        </div>
        <div className="solar-system" role="group" aria-label="Top artists">
          <span className="sun" aria-hidden="true" />
          {[1, 2, 3, 4].map((orbit) => <span className={`orbit orbit--${orbit}`} key={orbit} aria-hidden="true" />)}
          {artists.items?.map((artist, index) => {
            const name = artist.disambiguation ? `${artist.name} (${artist.disambiguation})` : artist.name;
            const plays = `${formatCount(artist.plays)} ${artist.plays === 1 ? "play" : "plays"}`;
            return (
              <button type="button" className={`planet planet--${index + 1}`} key={artist.id}
                onClick={() => onSelectArtist(artist)} aria-label={`Explore ${name}, ${plays}`}>
                <span className="planet__dot" aria-hidden="true" />
                <span className="planet__label" aria-hidden="true"><strong>{name}</strong><small>{plays}</small></span>
              </button>
            );
          })}
        </div>
      </section>
      <div className="universe-shelves">
        <Shelf title="Continue Listening" icon={<Play />} empty={currentTrack ? null : "Your saved queue will appear here."}>
          {currentTrack && (
            <div className="universe-resume">
              {hideTrack ? <div className="universe-cover"><AudioLines aria-hidden="true" /></div> : <Artwork track={currentTrack} />}
              <div className="universe-row__copy">
                <strong>{hideTrack ? (playback.blindTastingLabel ?? "Blind tasting") : currentTrack.title}</strong>
                {!hideTrack && <small>{displayTrackArtist(currentTrack)}</small>}
                <small>{playing ? "Playing now" : `At ${formatDuration(playback.positionSeconds)}`} · {formatCount(playback.queue.length)} in queue</small>
              </div>
              {!playing && <button type="button" className="universe-resume__action" disabled={playbackBusy} onClick={onResume}>
                <Play aria-hidden="true" /> Resume
              </button>}
            </div>
          )}
        </Shelf>
        <Shelf title="Almost Final Rank" icon={<Star />} empty={shelfStatus(nearFinal, "No ranked-year albums with one or two ratings left.")}
          retry={nearFinal.failed ? nearFinal.retry : undefined}>
          {nearFinal.items?.map((album) => (
            <AlbumRow key={album.id} album={album}
              detail={`${album.totalTracks - album.ratedTracks} ${album.totalTracks - album.ratedTracks === 1 ? "rating" : "ratings"} left`} onOpen={onOpenAlbum} />
          ))}
        </Shelf>
        <Shelf title="Recently Added" icon={<Disc3 />} empty={shelfStatus(recent, "Albums imported through Inbox will appear here.")}
          retry={recent.failed ? recent.retry : undefined}>
          {recent.items?.map((album) => <AlbumRow key={album.id} album={album} onOpen={onOpenAlbum} />)}
        </Shelf>
        <Shelf title="Rediscover" icon={<Compass />} empty={shelfStatus(rediscover, "No loved or five-star tracks last heard at least a year ago.")}
          retry={rediscover.failed ? rediscover.retry : undefined}>
          {rediscover.items?.map(({ track, lastListenedAtMs }) => (
            <button type="button" className="universe-row" key={track.trackKey} aria-label={`Play ${track.title}`}
              disabled={playbackBusy} onClick={() => onPlayTrack(track, rediscover.items?.map((item) => item.track) ?? [track])}>
              <Artwork track={track} />
              <span className="universe-row__copy"><strong>{track.title}</strong><small>{displayTrackArtist(track)}</small></span>
              <span className="universe-row__detail">Last heard {lastHeardDate.format(lastListenedAtMs)}</span>
              <Play className="universe-row__action" aria-hidden="true" />
            </button>
          ))}
        </Shelf>
      </div>
    </>
  );
}

function shelfStatus(shelf: { items: unknown[] | null; failed: boolean }, empty: string) {
  if (shelf.items?.length) return null;
  if (shelf.failed) return "Could not load this shelf.";
  return shelf.items === null ? "Loading…" : empty;
}

function Shelf({ title, icon, empty, retry, children }: {
  title: string; icon: ReactNode; empty: string | null; retry?: () => void; children: ReactNode;
}) {
  return (
    <section className="universe-shelf" aria-label={title}>
      <h2><span aria-hidden="true">{icon}</span>{title}</h2>
      {empty && <p className="universe-shelf__empty">{empty}</p>}
      {children}
      {retry && <button className="universe-retry" type="button" onClick={retry}>Retry</button>}
    </section>
  );
}

function AlbumRow({ album, detail, onOpen }: { album: AlbumSummary; detail?: string; onOpen: (id: string, title: string) => void }) {
  const cover = useAlbumCoverUrl(album.id, 64);
  const [failedCover, setFailedCover] = useState<string | null>(null);
  return (
    <button className="universe-row" type="button" aria-label={`Open ${album.title}${detail ? `, ${detail}` : ""}`}
      onClick={() => onOpen(album.id, album.title)}>
      <span className="universe-cover" aria-hidden="true">
        {cover && cover !== failedCover
          ? <img src={cover} alt="" loading="lazy" onError={() => setFailedCover(cover)} />
          : <Disc3 />}
      </span>
      <span className="universe-row__copy"><strong>{album.title}</strong><small>{album.artist}</small></span>
      {detail && <span className="universe-row__detail">{detail}</span>}
      <ArrowUpRight className="universe-row__action" aria-hidden="true" />
    </button>
  );
}
