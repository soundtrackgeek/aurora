import { ArrowLeft, ArrowUpRight, AudioLines, BookOpen, Disc3, Globe2, Headphones, ListMusic, LoaderCircle, Play, RefreshCw, Shuffle, UsersRound } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useAlbumCoverUrl } from "../../albumArtwork";
import { artistFacts, biographyText, findArtistTrack, listeningMonths, loadArtistArtwork, loadArtistDiscovery, loadArtistListening, openArtistLink, type ArtistArtwork, type ArtistDiscovery, type ArtistListening } from "../../artistPage";
import { exploreAlbums, exploreTracks, formatCount, formatDuration, loadArtistDetail, type AlbumSummary, type ArtistDetail, type Track, type TrackPage } from "../../library";
import { loadArtistIntelligence, type ArtistIntelligence } from "../../musicbrainz";
import { transitionContent } from "../../contentTransition";
import { ArtistPortrait } from "../ArtistPortrait";
import { Artwork } from "../Artwork";
import { ContentTransition } from "../ContentTransition";
import { CountryFlag } from "../CountryFlag";
import "./ArtistPage.css";

interface ArtistPageProps {
  artist: string;
  catalogRevision?: number;
  onBack?: () => void;
  onOpenArtist: (artist: string) => void;
  onOpenAlbum: (album: AlbumSummary) => void;
  onPlay: (tracks: Track[]) => Promise<boolean>;
  onSettings: () => void;
}
type Tab = "Overview" | "Albums" | "Tracks" | "Similar artists" | "Tags";

function Panel({ title, icon, children, action, className = "" }: { title: string; icon: ReactNode; children: ReactNode; action?: ReactNode; className?: string }) {
  return <section className={`artist-panel ${className}`} aria-label={title}><header><h2>{icon}{title}</h2>{action}</header>{children}</section>;
}

function AlbumTile({ album, onOpen }: { album: AlbumSummary; onOpen: () => void }) {
  const cover = useAlbumCoverUrl(album.id, 256);
  const [failed, setFailed] = useState(false);
  return <button className="artist-album" onClick={onOpen} aria-label={`Open ${album.title}`}><span className="artist-album__cover">{cover && !failed ? <img src={cover} alt="" loading="lazy" onError={() => setFailed(true)} /> : <Disc3 aria-hidden="true" />}<span className="artist-album__open"><ArrowUpRight /></span></span><strong>{album.title}</strong><small>{album.originalYear ?? album.releaseYear ?? "Year unknown"}</small></button>;
}

function Loading({ children = "Loading…" }: { children?: ReactNode }) { return <p className="artist-empty" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" />{children}</p>; }

export function ArtistPage({ artist, catalogRevision = 0, onBack, onOpenArtist, onOpenAlbum, onPlay, onSettings }: ArtistPageProps) {
  const [tab, setTab] = useState<Tab>("Overview");
  const [detail, setDetail] = useState<ArtistDetail | null>(null);
  const [intelligence, setIntelligence] = useState<ArtistIntelligence | null>(null);
  const [discovery, setDiscovery] = useState<ArtistDiscovery | null>(null);
  const [artwork, setArtwork] = useState<ArtistArtwork | null>(null);
  const [listening, setListening] = useState<ArtistListening | null>(null);
  const [tracks, setTracks] = useState<TrackPage | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [refresh, setRefresh] = useState(0);
  const [busy, setBusy] = useState<string | null>(null);
  const [playMessage, setPlayMessage] = useState<string | null>(null);
  const [failedImages, setFailedImages] = useState<string[]>([]);
  const alive = useRef(true);
  const loadedResources = useRef(new Map<string, string>());
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  useEffect(() => {
    let active = true;
    function settle<T>(key: string, load: () => Promise<T>, commit: (value: T) => void) {
      const requestKey = JSON.stringify([artist, refresh, catalogRevision]);
      if (loadedResources.current.get(key) === requestKey) return;
      void load().then((value) => { if (active) transitionContent(() => { loadedResources.current.set(key, requestKey); commit(value); setErrors((current) => { const next = { ...current }; delete next[key]; return next; }); }, "artist-detail"); })
        .catch((error: unknown) => { if (active) setErrors((current) => ({ ...current, [key]: error instanceof Error ? error.message : String(error) })); });
    }
    settle("Library", () => loadArtistDetail(artist), setDetail);
    settle("MusicBrainz", () => loadArtistIntelligence(artist), setIntelligence);
    settle("Last.fm", () => loadArtistDiscovery(artist, refresh > 0), setDiscovery);
    settle("fanart.tv", () => loadArtistArtwork(artist, refresh > 0), setArtwork);
    settle("Listening history", () => loadArtistListening(artist), setListening);
    settle("Tracks", () => exploreTracks({ artist, pageSize: 100, sort: "albumAsc" }, { localOnly: true }), setTracks);
    return () => { active = false; };
  }, [artist, refresh, catalogRevision]);

  function chooseTab(next: Tab) { transitionContent(() => setTab(next), "artist-detail"); }
  async function play(queue: Track[], shuffle = false) {
    if (busy || !queue.length) return;
    setBusy("play"); setPlayMessage(null);
    const ordered = [...queue];
    if (shuffle) for (let i = ordered.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [ordered[i], ordered[j]] = [ordered[j], ordered[i]]; }
    try { if (!await onPlay(ordered) && alive.current) setPlayMessage("Playback could not start. Check the player for details."); }
    catch (error) { if (alive.current) setPlayMessage(error instanceof Error ? error.message : String(error)); }
    finally { if (alive.current) setBusy(null); }
  }
  async function playNamed(title: string) {
    if (busy) return;
    setBusy(title); setPlayMessage(null);
    try {
      const track = await findArtistTrack(artist, title);
      if (!alive.current) return;
      if (track) { if (!await onPlay([track])) setPlayMessage("Playback could not start. Check the player for details."); }
      else setPlayMessage(`“${title}” is not available in your local library.`);
    } catch (error) { if (alive.current) setPlayMessage(error instanceof Error ? error.message : String(error)); }
    finally { if (alive.current) setBusy(null); }
  }
  async function moreTracks() {
    if (!tracks?.nextCursor || busy) return;
    setBusy("tracks");
    try { const page = await exploreTracks({ artist, pageSize: 100, sort: "albumAsc", cursor: tracks.nextCursor }, { localOnly: true }); if (alive.current) transitionContent(() => setTracks({ ...page, items: [...tracks.items, ...page.items] }), "artist-detail"); }
    catch (error) { if (alive.current) setPlayMessage(String(error)); }
    finally { if (alive.current) setBusy(null); }
  }
  async function allAlbums() {
    if (!detail?.albumsTruncated || busy) return;
    setBusy("albums");
    try {
      const albums: AlbumSummary[] = [];
      let cursor;
      do {
        const page = await exploreAlbums({ artist, pageSize: 100, sort: "yearDesc", cursor }, { localOnly: true });
        if (!alive.current) return;
        albums.push(...page.items); cursor = page.nextCursor ?? undefined;
      } while (cursor);
      transitionContent(() => setDetail({ ...detail, albums, albumsTruncated: false }), "artist-detail");
    } catch (error) { if (alive.current) setPlayMessage(String(error)); }
    finally { if (alive.current) setBusy(null); }
  }

  const albums = detail?.albums ?? [];
  const profile = intelligence?.profile ?? null;
  const facts = artistFacts(profile);
  const tags = [...new Set([...(discovery?.tags ?? []), ...albums.flatMap((album) => album.genre ? [album.genre] : [])])];
  const background = artwork?.backgroundUrl && !failedImages.includes(artwork.backgroundUrl) ? artwork.backgroundUrl : null;
  const portrait = artwork?.portraitUrl && !failedImages.includes(artwork.portraitUrl) ? artwork.portraitUrl : null;
  const biography = biographyText(discovery?.biography ?? "");
  const months = listeningMonths(listening?.months ?? {});
  const maxPlays = Math.max(1, ...months.map((month) => month.plays));
  const lastfmUrl = `https://www.last.fm/music/${encodeURIComponent(artist)}`;
  const duration = !detail?.albumsTruncated ? albums.reduce((sum, album) => sum + (album.durationSeconds ?? 0), 0) : null;
  const years = [...new Map(albums.filter((a) => a.originalYear ?? a.releaseYear).map((a) => [a.originalYear ?? a.releaseYear, 0])).keys()].sort((a, b) => (a ?? 0) - (b ?? 0));
  const yearMin = years[0] ?? 0, yearMax = years[years.length - 1] ?? yearMin;
  const summary = detail ? `${formatCount(detail.artist.albumCount)} albums · ${formatCount(detail.artist.trackCount)} tracks${duration ? ` · ${(duration / 3600).toFixed(1)} hours of music` : ""}` : errors.Library ? "Explore this artist" : "Opening your library…";
  const externalLink = (url: string, label: string) => <a href={url} target="_blank" rel="noreferrer">{label}<ArrowUpRight aria-hidden="true" /></a>;

  const similar = <Panel title="Similar artists" icon={<UsersRound />} action={tab === "Overview" ? <button onClick={() => chooseTab("Similar artists")}>See all</button> : <small>Last.fm</small>}>
    {!discovery && !errors["Last.fm"] ? <Loading>Finding similar artists…</Loading> : discovery?.similarArtists.length ? <div className="artist-similar">{discovery.similarArtists.slice(0, tab === "Overview" ? 6 : 12).map((item) => <button key={item.name} onClick={() => onOpenArtist(item.name)}><ArtistPortrait artist={item.name} size={128} /><strong>{item.name}</strong></button>)}</div> : <p className="artist-empty">No similar artists are available from Last.fm.</p>}
  </Panel>;

  const albumSection = <Panel title="Albums in your library" icon={<Disc3 />} action={tab === "Overview" ? <button onClick={() => chooseTab("Albums")}>See all{detail ? ` ${detail.artist.albumCount}` : ""}</button> : undefined}>
    {!detail && !errors.Library ? <Loading>Loading albums…</Loading> : albums.length ? <div className="artist-albums">{albums.slice(0, tab === "Overview" ? 6 : undefined).map((album) => <AlbumTile key={album.id} album={album} onOpen={() => onOpenAlbum(album)} />)}</div> : <p className="artist-empty">No albums by this artist are available in your library.</p>}
    {tab === "Albums" && detail?.albumsTruncated && <button className="button" disabled={Boolean(busy)} onClick={() => void allAlbums()}>Load remaining albums</button>}
  </Panel>;

  return <article className="artist-page" aria-label={`${artist} artist page`} onClick={(event) => {
    const link = (event.target as HTMLElement).closest<HTMLAnchorElement>("a[href]");
    if (!link) return;
    event.preventDefault();
    void openArtistLink(link.href).catch((error: unknown) => setPlayMessage(error instanceof Error ? error.message : String(error)));
  }}>
    <div className="artist-page__toolbar">{onBack && <button onClick={onBack}><ArrowLeft aria-hidden="true" /> Back to library</button>}<button onClick={() => { setRefresh((n) => n + 1); }} title="Refresh artist details"><RefreshCw aria-hidden="true" /> Refresh</button></div>
    <header className="artist-hero">
      {background && <img className="artist-hero__background" src={background} alt="" onError={() => setFailedImages((current) => [...current, background])} />}
      <div className="artist-hero__identity">
        <span className="artist-hero__portrait">{portrait ? <img src={portrait} alt="" onError={() => setFailedImages((current) => [...current, portrait])} /> : <ArtistPortrait artist={artist} size={128} eager />}</span>
        <div className="artist-hero__copy"><p className="eyebrow">Artist</p><h1>{artist}</h1><p>{summary}</p><div className="artist-hero__tags">{profile?.countryCode && <span className="artist-hero__country"><CountryFlag code={profile.countryCode} name={profile.countryName} />{profile.countryName ?? profile.countryCode}</span>}{tags.slice(0, 4).map((tag) => <span key={tag}>{tag}</span>)}</div></div>
        <div className="artist-hero__actions"><button className="button button--primary" disabled={!tracks?.items.length || Boolean(busy)} onClick={() => void play(tracks?.items ?? [])} title={`Play ${tracks?.items.length ?? 0} loaded tracks`}><Play aria-hidden="true" /> Play</button><button className="button" disabled={!tracks?.items.length || Boolean(busy)} onClick={() => void play(tracks?.items ?? [], true)} title="Shuffle loaded tracks"><Shuffle aria-hidden="true" /> Shuffle</button></div>
      </div>
      {background && <a className="artist-hero__credit" href="https://fanart.tv/" target="_blank" rel="noreferrer">Photography via fanart.tv</a>}
    </header>
    <nav className="artist-tabs" aria-label="Artist sections">{(["Overview", "Albums", "Tracks", "Similar artists", "Tags"] as Tab[]).map((name) => <button key={name} aria-current={name === tab ? "page" : undefined} onClick={() => chooseTab(name)}>{name}</button>)}</nav>
    {playMessage && <p className="artist-notice" role="status">{playMessage}</p>}
    <ContentTransition type="artist-detail" enter>
      <div key={tab} className={tab === "Overview" ? "artist-overview" : "artist-section"}>
        {tab === "Overview" && <>
          <div className="artist-overview__main">
            <div className="artist-overview__intro">
              <Panel title="About the artist" icon={<BookOpen />} className="artist-about">
                {!discovery && !errors["Last.fm"] ? <Loading>Loading biography…</Loading> : biography ? <p className="artist-biography">{biography}</p> : <p className="artist-empty">No biography is available yet.</p>}
                {facts.length > 0 && <dl className="artist-facts">{facts.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>}
                <div className="artist-source-links">{externalLink(lastfmUrl, "Biography on Last.fm")}{intelligence?.identity && externalLink(`https://musicbrainz.org/artist/${intelligence.identity.mbid}`, "MusicBrainz")}</div>
                {intelligence?.matchState === "unconfirmed" && <small className="artist-source-note">MusicBrainz identity is not yet verified.</small>}
              </Panel>
              <div className="artist-overview__memory">
                <Panel title="Your listening history" icon={<AudioLines />} action={<small>Last 12 months · UTC</small>}>
                  {!listening && !errors["Listening history"] ? <Loading>Loading your listening history…</Loading> : <>
                    <div className="artist-history-chart" role="img" aria-label={months.map((m) => `${m.label}: ${m.plays} plays`).join(", ")}>{months.map((month) => <div key={month.key} title={`${month.label}: ${formatCount(month.plays)} plays`}><span style={{ height: `${Math.max(month.plays > 0 ? 3 : 0, month.plays / maxPlays * 100)}%` }} /><small>{month.label.split(" ")[0]}</small></div>)}</div>
                    <div className="artist-listening-stats"><div><strong>{formatCount(listening?.plays ?? 0)}</strong><span>registered plays</span></div><div><strong>{((listening?.listenedSeconds ?? 0) / 3600).toFixed(1)}</strong><span>hours listened</span></div><div><strong>{discovery?.listeners == null ? "—" : formatCount(discovery.listeners)}</strong><span>Last.fm listeners</span></div></div>
                    {listening?.plays === 0 && <small className="artist-source-note">No registered plays during this period.</small>}
                  </>}
                </Panel>
                <Panel title="Release timeline" icon={<Disc3 />} action={<small>Your library</small>}>
                  {years.length ? <div className="artist-timeline"><div>{years.map((year) => <span key={year} title={`${year}: ${albums.filter((a) => (a.originalYear ?? a.releaseYear) === year).length} albums`} style={{ left: `${5 + ((year ?? 0) - yearMin) / Math.max(1, yearMax - yearMin) * 90}%` }} />)}</div><p><span>{yearMin}</span><span>{yearMax !== yearMin ? yearMax : ""}</span></p></div> : <p className="artist-empty">Release years will appear when available.</p>}
                </Panel>
              </div>
            </div>
            {albumSection}
            {similar}
          </div>
          <div className="artist-overview__side">
            <Panel title="Popular tracks" icon={<ListMusic />} action={<small>Last.fm top 10</small>}>
              <p className="artist-section-caption">Worldwide listening · all time</p>
              {!discovery && !errors["Last.fm"] ? <Loading>Loading popular tracks…</Loading> : discovery?.topTracks.length ? <ol className="artist-popular">{discovery.topTracks.map((track, index) => <li key={`${index}:${track.name}`}><span className="artist-rank">{String(index + 1).padStart(2, "0")}</span><button disabled={Boolean(busy)} onClick={() => void playNamed(track.name)} title={`Find and play ${track.name} in your library`}><strong>{track.name}</strong><small>{track.listeners == null ? "Listeners unavailable" : `${formatCount(track.listeners)} listeners`}</small></button><span className="artist-global-count">{track.playCount == null ? "—" : formatCount(track.playCount)}<small>plays</small></span>{track.url && <a href={track.url} target="_blank" rel="noreferrer" aria-label={`${track.name} on Last.fm`}><ArrowUpRight /></a>}</li>)}</ol> : <p className="artist-empty">Last.fm has no popular tracks to show.</p>}
              {externalLink(`${lastfmUrl}/+tracks`, "View on Last.fm")}
            </Panel>
            <Panel title="Most played by you" icon={<Headphones />} action={<small>Last 12 months</small>}>
              {listening?.topTracks.length ? <ol className="artist-personal">{listening.topTracks.map((track, index) => <li key={track.trackKey}><span className="artist-rank">{String(index + 1).padStart(2, "0")}</span><button disabled={Boolean(busy)} onClick={() => void playNamed(track.title)}><strong>{track.title}</strong><small>{track.album}</small></button><span>{formatCount(track.plays)}</span></li>)}</ol> : <p className="artist-empty">Your most played tracks will appear here.</p>}
            </Panel>
            <Panel title="Links & identity" icon={<Globe2 />}><div className="artist-links">{externalLink(lastfmUrl, "Last.fm")}{intelligence?.identity && externalLink(`https://musicbrainz.org/artist/${intelligence.identity.mbid}`, "MusicBrainz")}{externalLink(`https://en.wikipedia.org/w/index.php?search=${encodeURIComponent(artist)}`, "Find on Wikipedia")}</div></Panel>
          </div>
        </>}
        {tab === "Albums" && albumSection}
        {tab === "Similar artists" && similar}
        {tab === "Tags" && <Panel title="Artist tags" icon={<ListMusic />}><p className="artist-section-caption">Last.fm tags and genres from your library</p><div className="artist-tag-cloud">{tags.map((tag) => <span key={tag}>{tag}</span>)}</div>{!tags.length && <p className="artist-empty">No tags are available yet.</p>}</Panel>}
        {tab === "Tracks" && <Panel title="Tracks in your library" icon={<ListMusic />} action={<small>{tracks ? `${formatCount(tracks.items.length)} of ${formatCount(tracks.totalCount)}` : ""}</small>}>
          {!tracks && !errors.Tracks ? <Loading>Loading tracks…</Loading> : tracks?.items.length ? <div className="artist-track-list">{tracks.items.map((track, index) => <button key={track.trackKey} disabled={Boolean(busy)} onClick={() => void play([track, ...tracks.items.filter((t) => t.trackKey !== track.trackKey)])}><span className="artist-rank">{String(index + 1).padStart(2, "0")}</span><Artwork track={track} /><span><strong>{track.title}</strong><small>{track.album}</small></span><span>{formatDuration(track.durationSeconds)}</span><Play aria-hidden="true" /></button>)}</div> : <p className="artist-empty">No local tracks are available for this artist.</p>}
          {tracks?.nextCursor && <button className="button" disabled={Boolean(busy)} onClick={() => void moreTracks()}>{busy === "tracks" ? "Loading…" : "Load more tracks"}</button>}
        </Panel>}
      </div>
    </ContentTransition>
    <footer className="artist-page__footer"><span>MusicBrainz · Last.fm · fanart.tv</span><button onClick={onSettings}>Metadata settings <ArrowUpRight /></button></footer>
    {(Object.keys(errors).length > 0 || discovery?.warnings.length || artwork?.warning) ? <details className="artist-provider-status"><summary>Data availability</summary>{Object.entries(errors).map(([source, message]) => <p key={source}>{source}: {message}</p>)}{discovery?.warnings.map((warning) => <p key={warning}>{warning}</p>)}{artwork?.warning && <p>{artwork.warning}</p>}<button onClick={() => setRefresh((n) => n + 1)}>Retry unavailable data</button></details> : null}
  </article>;
}
