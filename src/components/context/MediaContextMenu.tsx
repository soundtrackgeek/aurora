import { useCallback, useContext, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { displayTrackArtist, isTauriRuntime, loadAlbumDetail, loadArtistTracks, type Track } from "../../library";
import { authorPlaylist, recentEditablePlaylists, type SavedPlaylistSummary } from "../../playlists";
import { PlaylistAuthoringContext, type PlaylistSelection } from "../playlists/playlistAuthoringContext";
import { MediaMenuContext, type MediaMenuRequest, type MediaSelection } from "./mediaMenuContext";
import "./MediaContextMenu.css";

async function resolveMediaTracks(selection: MediaSelection): Promise<Track[]> {
  if (selection.tracks?.length) return [...selection.tracks];
  if (selection.loadTrack) return [await selection.loadTrack()];
  if (selection.albumIds?.length) {
    const tracks: Track[] = [];
    for (const id of selection.albumIds) {
      const detail = await loadAlbumDetail(id, { localOnly: true });
      if (detail.tracksTruncated) throw new Error("This album is too large for a queue action. Select its songs instead.");
      tracks.push(...detail.tracks);
      if (tracks.length > 200) throw new Error("Select at most 200 songs for a queue action.");
    }
    return tracks;
  }
  return selection.artistOnly && selection.artistName ? loadArtistTracks(selection.artistName) : [];
}

interface Props {
  children: ReactNode;
  playlists: SavedPlaylistSummary[];
  onPlay: (tracks: Track[]) => Promise<boolean>;
  onEnqueue: (tracks: Track[], next: boolean) => Promise<boolean>;
  onOpenAlbum: (track: Track) => void;
  onOpenArtist: (artist: string) => void;
  onOpenTags: (selection: PlaylistSelection) => void;
  onRate: (track: Track, rating: number | null) => Promise<unknown>;
  onPlaylistSaved: () => void;
}

export function MediaContextMenuProvider({ children, playlists, onPlay, onEnqueue, onOpenAlbum, onOpenArtist, onOpenTags, onRate, onPlaylistSaved }: Props) {
  const [request, setRequest] = useState<MediaMenuRequest | null>(null);
  const [section, setSection] = useState<"main" | "playlist" | "rating">("main");
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [working, setWorking] = useState(false);
  const openPlaylist = useContext(PlaylistAuthoringContext);
  const typeAhead = useRef("");
  const typeAheadAt = useRef(0);
  const openMenu = useCallback((next: MediaMenuRequest) => {
    if (working) return;
    setSection("main"); typeAhead.current = ""; setRequest(next);
  }, [working]);

  function close(restore = true) {
    if (restore && request?.anchor.isConnected) request.anchor.focus({ preventScroll: true });
    setRequest(null);
  }
  useLayoutEffect(() => {
    if (!request || !menu.current) return;
    const element = menu.current;
    const bounds = element.getBoundingClientRect();
    element.style.left = `${Math.max(8, Math.min(request.x, window.innerWidth - bounds.width - 8))}px`;
    element.style.top = `${Math.max(8, Math.min(request.y, window.innerHeight - bounds.height - 8))}px`;
    element.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [request, section]);
  useEffect(() => {
    if (!request) return;
    const dismiss = (event: Event) => {
      if (event.target instanceof Node && menu.current?.contains(event.target)) return;
      setRequest(null);
    };
    const hide = () => setRequest(null);
    window.addEventListener("pointerdown", dismiss, true);
    window.addEventListener("scroll", dismiss, true);
    window.addEventListener("resize", hide);
    window.addEventListener("blur", hide);
    return () => {
      window.removeEventListener("pointerdown", dismiss, true);
      window.removeEventListener("scroll", dismiss, true);
      window.removeEventListener("resize", hide);
      window.removeEventListener("blur", hide);
    };
  }, [request]);

  function run(action: () => Promise<unknown>, success?: string) {
    if (working) return;
    setWorking(true);
    close();
    setNotice({ text: "Working…" });
    void action().then(() => setNotice(success ? { text: success } : null)).catch((error: unknown) => setNotice({ text: String(error), error: true })).finally(() => { setWorking(false); });
  }
  const selection = request?.selection;
  const track = selection?.tracks?.[0];
  const artist = selection?.artistName ?? (track ? displayTrackArtist(track) : null);
  const available = Boolean(selection?.tracks?.length || selection?.albumIds?.length || selection?.loadTrack || selection?.artistOnly);
  const playlistAvailable = Boolean(openPlaylist && available);
  const singleTrack = Boolean(selection?.loadTrack || selection?.tracks?.length === 1);
  const singleAlbum = selection?.albumIds?.length === 1;
  const fileBrowserLabel = /Macintosh|Mac OS/.test(navigator.userAgent) ? "Show in Finder" : /Linux/.test(navigator.userAgent) ? "Show in file browser" : "Show in Windows Explorer";
  async function tracks() {
    const result = await resolveMediaTracks(selection!);
    if (!result.length) throw new Error("No local songs are available for this item.");
    return result;
  }
  async function firstTrack() {
    if (track) return track;
    if (singleAlbum) {
      const first = (await loadAlbumDetail(selection!.albumIds![0], { localOnly: true })).tracks[0];
      if (!first) throw new Error("No local songs are available for this album.");
      return first;
    }
    return (await tracks())[0];
  }
  async function playlistSelection(): Promise<PlaylistSelection> {
    if (selection?.albumIds?.length) return selection;
    return { tracks: await tracks(), label: selection!.label };
  }
  function playlistDialog(playlistMode: "choose" | "create") { run(async () => { openPlaylist?.({ ...await playlistSelection(), playlistMode }); }); }
  const item = (label: string, action: () => void, disabled = false, submenu = false) => <button type="button" role="menuitem" disabled={disabled} aria-haspopup={submenu ? "menu" : undefined} onClick={action}>{label}{submenu && <span aria-hidden="true">›</span>}</button>;

  return <MediaMenuContext value={openMenu}>
    {children}
    {notice && <div className="media-menu-notice" role={notice.error ? "alert" : "status"}>{notice.text}<button type="button" aria-label="Dismiss context menu message" onClick={() => setNotice(null)}>×</button></div>}
    {request && selection && <div ref={menu} className="media-context-menu" role="menu" aria-label={section === "main" ? `Actions for ${selection.label}` : section === "playlist" ? "Add to playlist" : "Rate"} onContextMenu={e => e.preventDefault()} onKeyDown={event => {
      const buttons = [...(menu.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      if (event.key === "Escape" || event.key === "Tab") { event.preventDefault(); event.stopPropagation(); close(); }
      else if (event.key === "ArrowLeft" && section !== "main") { event.preventDefault(); setSection("main"); }
      else if (event.key === "ArrowRight" && section === "main" && document.activeElement?.getAttribute("aria-haspopup")) { event.preventDefault(); (document.activeElement as HTMLButtonElement).click(); }
      else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
        event.preventDefault();
        const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
        buttons[next]?.focus();
      } else if (event.key.length === 1 && event.key !== " " && !event.ctrlKey && !event.metaKey && !event.altKey) {
        event.preventDefault();
        if (Date.now() - typeAheadAt.current > 600) typeAhead.current = "";
        typeAheadAt.current = Date.now(); typeAhead.current += event.key.toLowerCase();
        buttons.find(button => button.textContent?.toLowerCase().startsWith(typeAhead.current))?.focus();
      }
    }}>
      <div className="media-context-menu__label" aria-hidden="true">{selection.label}</div>
      {selection.artistOnly && <div className="media-context-menu__hint">Uses up to 50 local songs, ordered by rating and title.</div>}
      {section === "main" ? <>
        {item("Play", () => run(async () => { if (!await onPlay(await tracks())) throw new Error("Could not start playback. Check the player error."); }), !available)}
        {item("Play next", () => run(async () => { if (!await onEnqueue(await tracks(), true)) throw new Error("Could not change the queue. Check the player error."); }, "Added to play next."), !available)}
        {item("Add to queue", () => run(async () => { if (!await onEnqueue(await tracks(), false)) throw new Error("Could not change the queue. Check the player error."); }, "Added to queue."), !available)}
        {item("Add to playlist", () => setSection("playlist"), !playlistAvailable, true)}
        <div role="separator" />
        {item("Go to album", () => run(async () => { onOpenAlbum(await firstTrack()); }), !(singleTrack || singleAlbum) || (Boolean(track) && !track?.albumId))}
        {item("Go to artist", () => run(async () => { onOpenArtist(artist ?? (singleAlbum ? (await loadAlbumDetail(selection.albumIds![0], { localOnly: true })).album.artist : displayTrackArtist(await firstTrack()))); }), !artist && !available)}
        {item(fileBrowserLabel, () => run(async () => {
          if (!isTauriRuntime()) throw new Error("Open the file browser from Aurora desktop.");
          if (singleAlbum) await invoke("reveal_catalog_item", { albumId: selection.albumIds![0] });
          else { const song = await firstTrack(); await invoke("reveal_catalog_item", { trackId: song.id, trackKey: song.trackKey }); }
        }), !(singleTrack || singleAlbum))}
        {item('Copy “Artist – Title”', () => run(async () => {
          let text = selection.label;
          if (selection.tracks?.length) text = selection.tracks.map(song => `${displayTrackArtist(song)} – ${song.title}`).join("\n");
          else if (artist) text = selection.label === artist ? artist : `${artist} – ${selection.label}`;
          else if (singleAlbum) { const album = await loadAlbumDetail(selection.albumIds![0], { localOnly: true }); text = `${album.album.artist} – ${album.album.title}`; }
          await navigator.clipboard.writeText(text);
        }, "Copied."))}
        <div role="separator" />
        {item("Open Tags", () => run(async () => { onOpenTags(await playlistSelection()); }), !available || Boolean(selection.artistOnly))}
        {item("Rate", () => setSection("rating"), !singleTrack, true)}
      </> : section === "playlist" ? <>
        {item("‹ Back", () => setSection("main"))}
        {recentEditablePlaylists(playlists).map(playlist => <button type="button" role="menuitem" key={playlist.id} onClick={() => run(async () => {
          const source = await playlistSelection();
          await authorPlaylist({ action: "append", id: playlist.id, expectedUpdatedAt: playlist.updatedAt, tracks: source.tracks, albumIds: source.albumIds });
          onPlaylistSaved();
        }, `Added to ${playlist.name}.`)}>{playlist.name}</button>)}
        <div role="separator" />
        {item("Choose playlist…", () => playlistDialog("choose"))}
        {item("Create playlist…", () => playlistDialog("create"))}
      </> : <>
        {item("‹ Back", () => setSection("main"))}
        {item("Clear rating", () => run(async () => { await onRate(await firstTrack(), null); }))}
        {Array.from({ length: 10 }, (_, index) => (index + 1) / 2).map(rating => <button key={rating} type="button" role="menuitem" onClick={() => run(async () => { await onRate(await firstTrack(), rating); })}>{rating} ★</button>)}
      </>}
    </div>}
  </MediaMenuContext>;
}
