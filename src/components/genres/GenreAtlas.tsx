import {
  Album,
  AudioLines,
  ChevronRight,
  Clock3,
  Compass,
  Disc3,
  Heart,
  ListMusic,
  LoaderCircle,
  Music2,
  Play,
  Radio,
  Search,
  Shuffle,
  Sparkles,
  Star,
  Timer,
  UsersRound,
} from "lucide-react";
import { useDeferredValue, useMemo, useState } from "react";
import {
  type GenreAlbum,
  type GenreDetail,
  type GenreQueueMode,
  type GenreRadioSession,
  type GenreSort,
  type GenreSummary,
  sortGenres,
} from "../../genres";
import { albumCoverUrl, formatCount, formatDuration, type Track } from "../../library";
import { Artwork } from "../Artwork";
import { InlineLoveControl, InlineRatingControl } from "../InlineTagControls";
import "./GenreAtlas.css";

export type GenreAtlasLoadState = "loading" | "ready" | "error";

interface GenreAtlasProps {
  genres: readonly GenreSummary[];
  selectedGenre: string | null;
  detail: GenreDetail | null;
  search: string;
  indexState: GenreAtlasLoadState;
  detailState: GenreAtlasLoadState;
  indexError: string | null;
  detailError: string | null;
  queueBusy: GenreQueueMode | null;
  queueMessage: string | null;
  radioSession: GenreRadioSession | null;
  busyTrackKeys?: ReadonlySet<string>;
  onSearchChange: (value: string) => void;
  onSelectGenre: (genre: string) => void;
  onRetryIndex: () => void;
  onRetryDetail: () => void;
  onQueue: (mode: GenreQueueMode) => void;
  onOpenTracks: (genre: string) => void;
  onOpenArtist: (artist: string) => void;
  onSelectTrack: (track: Track) => void;
  onPlayTrack: (track: Track) => void;
  onRatingChange: (track: Track, rating: number | null) => void;
  onLoveChange: (track: Track, loveState: Track["loveState"]) => void;
}

const EMPTY_BUSY_TRACK_KEYS: ReadonlySet<string> = new Set();
const PAGE_SIZE = 120;

const sortOptions: ReadonlyArray<{ value: GenreSort; label: string }> = [
  { value: "size", label: "Largest worlds" },
  { value: "rating", label: "Highest rated" },
  { value: "loved", label: "Most Loved" },
  { value: "recent", label: "Recently heard" },
  { value: "unexplored", label: "Unexplored" },
  { value: "alphabetical", label: "A–Z" },
];

const queueActions: ReadonlyArray<{
  mode: GenreQueueMode;
  label: string;
  detail: string;
  icon: typeof Play;
  primary?: boolean;
}> = [
  { mode: "radio", label: "Play Genre Radio", detail: "Rated favorites with room for discovery", icon: Radio, primary: true },
  { mode: "shuffle", label: "Shuffle", detail: "A fresh cross-section", icon: Shuffle },
  { mode: "loved", label: "Loved", detail: "Only heart-marked tracks", icon: Heart },
  { mode: "highestRated", label: "Highest Rated", detail: "Your strongest ratings first", icon: Star },
  { mode: "rediscover", label: "Rediscover", detail: "Rated tracks without a registered play", icon: Compass },
  { mode: "unrated", label: "Unrated Expedition", detail: "Give neglected music a chance", icon: Sparkles },
];

function albumAsTrack(album: GenreAlbum): Track {
  return {
    id: `genre-album:${album.id}`,
    trackKey: `genre-album:${album.id}`,
    albumId: album.id,
    title: album.title,
    artist: album.artist,
    album: album.title,
    releaseYear: album.releaseYear,
    rating: album.rating,
    loved: album.lovedTracks > 0,
    loveState: album.lovedTracks > 0 ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: album.durationSeconds,
    genre: null,
    playCount: null,
  };
}

function GenreCover({ genre }: { genre: GenreSummary }) {
  const source = albumCoverUrl(genre.representativeAlbumId, 128);
  const [failed, setFailed] = useState<string | null>(null);
  return (
    <span className="genre-cover" aria-hidden="true">
      {source && source !== failed
        ? <img src={source} alt="" onError={() => setFailed(source)} />
        : <><strong>{genre.name.slice(0, 2).toLocaleUpperCase()}</strong><AudioLines /></>}
    </span>
  );
}

function compactHours(seconds: number): string {
  if (seconds < 3_600) return `${Math.round(Math.max(0, seconds) / 60)}m`;
  const hours = Math.round(Math.max(0, seconds) / 3600);
  if (hours < 1_000) return `${formatCount(hours)}h`;
  return `${new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 }).format(hours)}h`;
}

function countLabel(count: number, singular: string): string {
  return `${formatCount(count)} ${count === 1 ? singular : `${singular}s`}`;
}

function relativeListeningTime(value: number | null): string {
  if (value === null) return "Not heard in Aurora yet";
  const deltaDays = Math.round((value - Date.now()) / 86_400_000);
  if (Math.abs(deltaDays) < 1) return "Heard today";
  if (Math.abs(deltaDays) < 60) return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(deltaDays, "day");
  const deltaMonths = Math.round(deltaDays / 30);
  if (Math.abs(deltaMonths) < 24) return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(deltaMonths, "month");
  return new Date(value).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function LoadingState({ copy }: { copy: string }) {
  return <div className="genre-atlas-feedback" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" /><strong>{copy}</strong><span>Reading one bounded view from the local catalog.</span></div>;
}

function ErrorState({ message, onRetry }: { message: string | null; onRetry: () => void }) {
  return <div className="genre-atlas-feedback genre-atlas-feedback--error" role="alert"><Disc3 aria-hidden="true" /><strong>Genre space is temporarily unavailable.</strong><span>{message ?? "Aurora could not read this view."}</span><button type="button" onClick={onRetry}>Try again</button></div>;
}

function GenreIndex({
  genres,
  selectedGenre,
  search,
  state,
  error,
  onSearchChange,
  onSelectGenre,
  onRetry,
}: {
  genres: readonly GenreSummary[];
  selectedGenre: string | null;
  search: string;
  state: GenreAtlasLoadState;
  error: string | null;
  onSearchChange: (value: string) => void;
  onSelectGenre: (genre: string) => void;
  onRetry: () => void;
}) {
  const [sort, setSort] = useState<GenreSort>("size");
  const [visibility, setVisibility] = useState({ key: "", limit: PAGE_SIZE });
  const deferredSearch = useDeferredValue(search.trim().toLocaleLowerCase());
  const ordered = useMemo(() => {
    const matches = deferredSearch
      ? genres.filter((genre) => genre.name.toLocaleLowerCase().includes(deferredSearch))
      : genres;
    return sortGenres(matches, sort);
  }, [deferredSearch, genres, sort]);
  const visibilityKey = `${deferredSearch}:${sort}`;
  const visibleLimit = visibility.key === visibilityKey ? visibility.limit : PAGE_SIZE;
  const visible = ordered.slice(0, visibleLimit);
  return (
    <aside className="genre-index" aria-label="Genre index">
      <header>
        <div><span>Genre index</span><strong>{formatCount(ordered.length)} worlds</strong></div>
        <label className="genre-index__sort"><span className="sr-only">Sort genres</span><select value={sort} onChange={(event) => setSort(event.currentTarget.value as GenreSort)}>{sortOptions.map((option) => <option value={option.value} key={option.value}>{option.label}</option>)}</select></label>
      </header>
      <label className="genre-index__search">
        <Search aria-hidden="true" />
        <span className="sr-only">Search genres</span>
        <input type="search" value={search} onChange={(event) => onSearchChange(event.currentTarget.value)} placeholder="Search 687 genres…" />
      </label>
      {state === "loading" ? <LoadingState copy="Charting your genres…" />
        : state === "error" ? <ErrorState message={error} onRetry={onRetry} />
          : visible.length === 0 ? <div className="genre-atlas-feedback"><Search aria-hidden="true" /><strong>No genre matches “{search}”.</strong><button type="button" onClick={() => onSearchChange("")}>Clear search</button></div>
            : <>
              <div className="genre-index__list">
                {visible.map((genre) => (
                  <button
                    type="button"
                    className={selectedGenre === genre.name ? "is-selected" : undefined}
                    aria-pressed={selectedGenre === genre.name}
                    onClick={() => onSelectGenre(genre.name)}
                    key={genre.name}
                  >
                    <GenreCover genre={genre} />
                    <span className="genre-index__copy"><strong>{genre.name}</strong><small>{countLabel(genre.trackCount, "track")} · {countLabel(genre.albumCount, "album")}</small></span>
                    <span className="genre-index__signal">{genre.averageRating === null ? "Unrated" : `${genre.averageRating.toFixed(1)} ★`}<small>{genre.plays > 0 ? `${formatCount(genre.plays)} plays` : "unheard"}</small></span>
                    <ChevronRight aria-hidden="true" />
                  </button>
                ))}
              </div>
              {visible.length < ordered.length ? <button type="button" className="genre-index__more" onClick={() => setVisibility({ key: visibilityKey, limit: visibleLimit + PAGE_SIZE })}>Show {formatCount(Math.min(PAGE_SIZE, ordered.length - visible.length))} more</button> : null}
            </>}
    </aside>
  );
}

function GenreTimeline({ detail }: { detail: GenreDetail }) {
  const largest = Math.max(1, ...detail.decades.map((decade) => decade.trackCount));
  return (
    <section className="genre-panel genre-timeline" aria-labelledby="genre-timeline-title">
      <header><div><span>Release gravity</span><h3 id="genre-timeline-title">Through the decades</h3></div><Clock3 aria-hidden="true" /></header>
      {detail.decades.length ? <div className="genre-timeline__bars" role="img" aria-label={`${detail.summary.name} releases from ${detail.summary.firstYear ?? "unknown"} to ${detail.summary.lastYear ?? "unknown"}`}>
        {detail.decades.map((decade) => <div key={decade.decade}><span aria-hidden="true"><i style={{ height: `${Math.max(7, (decade.trackCount / largest) * 100)}%` }} /></span><strong>{decade.decade}s</strong><small>{formatCount(decade.trackCount)}</small></div>)}
      </div> : <p className="genre-panel__empty">No reliable release years are available for this genre.</p>}
    </section>
  );
}

function GenreHero({
  detail,
  queueBusy,
  queueMessage,
  radioSession,
  onQueue,
  onOpenTracks,
}: Pick<GenreAtlasProps, "detail" | "queueBusy" | "queueMessage" | "radioSession" | "onQueue" | "onOpenTracks"> & { detail: GenreDetail }) {
  const { summary } = detail;
  const ratingCoverage = summary.trackCount ? Math.round((summary.ratedTracks / summary.trackCount) * 100) : 0;
  return (
    <>
      <section className="genre-hero" aria-labelledby="genre-detail-title">
        <div className="genre-hero__covers" aria-hidden="true">
          {detail.albums.slice(0, 4).map((album) => <Artwork track={albumAsTrack(album)} size="large" key={album.id} />)}
        </div>
        <div className="genre-hero__shade" />
        <div className="genre-hero__content">
          <span className="genre-hero__kicker"><Sparkles aria-hidden="true" /> Canonical genre</span>
          <h2 id="genre-detail-title">{summary.name}</h2>
          <p>{summary.firstYear && summary.lastYear ? `${summary.firstYear}–${summary.lastYear}` : "Release years still forming"} · {countLabel(summary.artistCount, "artist")} · {compactHours(summary.durationSeconds)}</p>
          <div className="genre-hero__actions">
            <button type="button" className="genre-play" disabled={queueBusy !== null} onClick={() => onQueue("radio")}>
              {queueBusy === "radio" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Play aria-hidden="true" />} Play Genre Radio
            </button>
            <button type="button" onClick={() => onOpenTracks(summary.name)}><ListMusic aria-hidden="true" /> All tracks</button>
          </div>
        </div>
        <dl className="genre-hero__stats">
          <div><dt>Tracks</dt><dd>{formatCount(summary.trackCount)}</dd></div>
          <div><dt>Albums</dt><dd>{formatCount(summary.albumCount)}</dd></div>
          <div><dt>Your rating</dt><dd>{summary.averageRating === null ? "—" : `${summary.averageRating.toFixed(1)} ★`}</dd><small>{ratingCoverage}% rated</small></div>
          <div><dt>Loved</dt><dd>{formatCount(summary.lovedTracks)}</dd></div>
        </dl>
      </section>
      <section className="genre-queue-actions" aria-label={`Play ${summary.name}`}>
        {queueActions.slice(1).map(({ mode, label, detail: actionDetail, icon: Icon }) => <button type="button" disabled={queueBusy !== null} onClick={() => onQueue(mode)} key={mode}><span>{queueBusy === mode ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Icon aria-hidden="true" />}</span><strong>{label}</strong><small>{actionDetail}</small></button>)}
      </section>
      {(queueMessage || radioSession?.genre === summary.name) ? <div className="genre-queue-status" role="status"><Radio aria-hidden="true" /><span><strong>{radioSession?.genre === summary.name ? `${summary.name} ${radioSession.mode === "radio" ? "Radio" : queueActions.find((action) => action.mode === radioSession.mode)?.label ?? "queue"} is active` : "Genre queue"}</strong><small>{queueMessage ?? "Aurora will refill before this bounded queue runs out."}</small></span></div> : null}
    </>
  );
}

function GenreRelationship({ detail }: { detail: GenreDetail }) {
  const summary = detail.summary;
  return (
    <section className="genre-panel genre-relationship" aria-labelledby="genre-relationship-title">
      <header><div><span>Your relationship</span><h3 id="genre-relationship-title">Listening memory</h3></div><Timer aria-hidden="true" /></header>
      <dl>
        <div><dt>Registered plays</dt><dd>{formatCount(summary.plays)}</dd></div>
        <div><dt>Listening time</dt><dd>{compactHours(summary.listenedSeconds)}</dd></div>
        <div><dt>Last heard</dt><dd>{relativeListeningTime(summary.lastListenedAtMs)}</dd></div>
      </dl>
      {summary.sessions === 0 ? <p>Aurora is ready to learn this relationship. Listening memory grows only from registered plays.</p> : <p>{formatCount(summary.sessions)} listening sessions have shaped this genre in Aurora.</p>}
    </section>
  );
}

function GenreConnections({ detail, onSelect }: { detail: GenreDetail; onSelect: (genre: string) => void }) {
  return (
    <section className="genre-panel genre-connections" aria-labelledby="genre-connections-title">
      <header><div><span>Shared artist paths</span><h3 id="genre-connections-title">Connected genres</h3></div><UsersRound aria-hidden="true" /></header>
      {detail.relatedGenres.length ? <div>{detail.relatedGenres.map((related) => <button type="button" onClick={() => onSelect(related.name)} key={related.name}><span><strong>{related.name}</strong><small>{countLabel(related.sharedArtists, "shared artist")} · {countLabel(related.sharedTracks, "track")}</small></span><ChevronRight aria-hidden="true" /></button>)}</div> : <p className="genre-panel__empty">No strong shared-artist paths were found.</p>}
      <small className="genre-connections__note">Connections are navigational signals, not an authoritative genre family tree.</small>
    </section>
  );
}

function GenreAlbums({ detail }: { detail: GenreDetail }) {
  return (
    <section className="genre-panel genre-albums" aria-labelledby="genre-albums-title">
      <header><div><span>Representative releases</span><h3 id="genre-albums-title">Albums in orbit</h3></div><Album aria-hidden="true" /></header>
      <div className="genre-albums__grid">{detail.albums.slice(0, 8).map((album) => <article key={album.id}><Artwork track={albumAsTrack(album)} size="large" decorative={false} /><strong>{album.title}</strong><span>{album.artist}</span><small>{album.releaseYear ?? "Year unknown"} · {countLabel(album.totalTracks, "track")}{album.rating === null ? "" : ` · ${album.rating.toFixed(1)} ★`}</small></article>)}</div>
    </section>
  );
}

function GenreArtists({ detail, onOpenArtist }: { detail: GenreDetail; onOpenArtist: (artist: string) => void }) {
  const largest = Math.max(1, ...detail.artists.map((artist) => artist.trackCount));
  return (
    <section className="genre-panel genre-artists" aria-labelledby="genre-artists-title">
      <header><div><span>Largest bodies</span><h3 id="genre-artists-title">Artists shaping the genre</h3></div><UsersRound aria-hidden="true" /></header>
      <ol>{detail.artists.map((artist, index) => <li key={artist.name}><button type="button" onClick={() => onOpenArtist(artist.name)}><span className="genre-artists__rank">{String(index + 1).padStart(2, "0")}</span><span className="genre-artists__copy"><strong>{artist.name}</strong><small>{countLabel(artist.albumCount, "album")} · {countLabel(artist.trackCount, "track")}</small><i aria-hidden="true"><b style={{ width: `${Math.max(5, (artist.trackCount / largest) * 100)}%` }} /></i></span>{artist.lovedTracks > 0 ? <span className="genre-artists__love"><Heart aria-hidden="true" /> {formatCount(artist.lovedTracks)}</span> : null}<ChevronRight aria-hidden="true" /></button></li>)}</ol>
    </section>
  );
}

function GenreHighlights({
  detail,
  busyTrackKeys,
  onSelectTrack,
  onPlayTrack,
  onRatingChange,
  onLoveChange,
}: Pick<GenreAtlasProps, "busyTrackKeys" | "onSelectTrack" | "onPlayTrack" | "onRatingChange" | "onLoveChange"> & { detail: GenreDetail }) {
  const busyKeys = busyTrackKeys ?? EMPTY_BUSY_TRACK_KEYS;
  return (
    <section className="genre-panel genre-highlights" aria-labelledby="genre-highlights-title">
      <header><div><span>Your strongest signals</span><h3 id="genre-highlights-title">Highlights</h3></div><Music2 aria-hidden="true" /></header>
      {detail.highlights.length ? <ol>{detail.highlights.map((track, index) => <li key={track.trackKey} onClick={() => onSelectTrack(track)} onDoubleClick={() => onPlayTrack(track)}><button type="button" className="genre-highlight__play" aria-label={`Play ${track.title}`} onClick={(event) => { event.stopPropagation(); onPlayTrack(track); }}><Play aria-hidden="true" /></button><span className="genre-highlight__rank">{String(index + 1).padStart(2, "0")}</span><Artwork track={track} /><span className="genre-highlight__copy"><strong>{track.title}</strong><small>{track.artist} · {track.album}</small></span><span className="genre-highlight__time">{formatDuration(track.durationSeconds)}</span><InlineRatingControl title={track.title} rating={track.rating} busy={busyKeys.has(track.trackKey)} allowClear onRatingChange={(rating) => onRatingChange(track, rating)} /><InlineLoveControl title={track.title} loveState={track.loveState} busy={busyKeys.has(track.trackKey)} onLoveChange={(loveState) => onLoveChange(track, loveState)} /></li>)}</ol> : <p className="genre-panel__empty">No rated or representative tracks are available.</p>}
    </section>
  );
}

function GenreDetailView(props: GenreAtlasProps) {
  if (props.detailState === "loading") return <div className="genre-detail"><LoadingState copy={`Opening ${props.selectedGenre ?? "this genre"}…`} /></div>;
  if (props.detailState === "error") return <div className="genre-detail"><ErrorState message={props.detailError} onRetry={props.onRetryDetail} /></div>;
  if (!props.detail) return <div className="genre-detail"><div className="genre-atlas-feedback"><Disc3 aria-hidden="true" /><strong>Choose a genre to enter its world.</strong><span>Aurora will load only its bounded detail view.</span></div></div>;
  return (
    <article className="genre-detail">
      <GenreHero detail={props.detail} queueBusy={props.queueBusy} queueMessage={props.queueMessage} radioSession={props.radioSession} onQueue={props.onQueue} onOpenTracks={props.onOpenTracks} />
      <div className="genre-detail__pair"><GenreTimeline detail={props.detail} /><GenreRelationship detail={props.detail} /></div>
      <GenreConnections detail={props.detail} onSelect={props.onSelectGenre} />
      <GenreAlbums detail={props.detail} />
      <div className="genre-detail__pair genre-detail__pair--lower"><GenreArtists detail={props.detail} onOpenArtist={props.onOpenArtist} /><GenreHighlights detail={props.detail} busyTrackKeys={props.busyTrackKeys} onSelectTrack={props.onSelectTrack} onPlayTrack={props.onPlayTrack} onRatingChange={props.onRatingChange} onLoveChange={props.onLoveChange} /></div>
    </article>
  );
}

export function GenreAtlas(props: GenreAtlasProps) {
  return (
    <section className="genre-atlas" aria-labelledby="genre-atlas-title">
      <header className="genre-atlas__heading">
        <div><span className="genre-atlas__kicker"><Disc3 aria-hidden="true" /> Genre Atlas</span><h1 id="genre-atlas-title">Every sound has a place in your universe.</h1><p>Move from the largest worlds to the quietest corners without loading the whole catalog.</p></div>
        <div className="genre-atlas__scope"><strong>{formatCount(props.genres.length)}</strong><span>canonical genres</span><small>Read-only Music Library authority</small></div>
      </header>
      <div className="genre-atlas__workspace">
        <GenreIndex genres={props.genres} selectedGenre={props.selectedGenre} search={props.search} state={props.indexState} error={props.indexError} onSearchChange={props.onSearchChange} onSelectGenre={props.onSelectGenre} onRetry={props.onRetryIndex} />
        <GenreDetailView {...props} />
      </div>
    </section>
  );
}
