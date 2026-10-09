import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { browserPreview } from "../../library";
import { rememberPlaylistWork, type SavedPlaylistSummary } from "../../playlists";
import { PlaylistRow } from "../playlists/PlaylistAuthoring";
import { PlaylistAuthoringContext } from "../playlists/playlistAuthoringContext";
import { MediaContextMenuProvider } from "./MediaContextMenu";
import type { MediaSelection } from "./mediaMenuContext";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { cleanup(); vi.clearAllMocks(); localStorage.clear(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); });
const song = browserPreview.tracks[0];
const playlist = (id: number, updatedAt: string, editable = true): SavedPlaylistSummary => ({ id, name: `Playlist ${id}`, description: "", trackCount: 1, updatedAt, editable });

function setup(selection: MediaSelection = { tracks: [song], label: song.title }, playlists: SavedPlaylistSummary[] = []) {
  const callbacks = { onPlay: vi.fn(async () => true), onEnqueue: vi.fn(async () => true), onOpenAlbum: vi.fn(), onOpenArtist: vi.fn(), onOpenTags: vi.fn(), onRate: vi.fn(async () => undefined), onPlaylistSaved: vi.fn() };
  const openPlaylist = vi.fn();
  const row = vi.fn();
  render(<PlaylistAuthoringContext value={openPlaylist}><MediaContextMenuProvider playlists={playlists} {...callbacks}><PlaylistRow selection={selection}><button onClick={row} onKeyDown={row}>Song row</button></PlaylistRow></MediaContextMenuProvider></PlaylistAuthoringContext>);
  return { ...callbacks, openPlaylist, row, anchor: screen.getByRole("button", { name: "Song row" }) };
}

it("opens with the keyboard without selecting the row, navigates items, and restores focus", () => {
  const { anchor, row } = setup();
  anchor.focus();
  fireEvent.keyDown(anchor, { key: "F10", shiftKey: true });
  expect(screen.getByRole("menuitem", { name: "Play" })).toHaveFocus();
  fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowDown" });
  expect(screen.getByRole("menuitem", { name: "Play next" })).toHaveFocus();
  fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(anchor).toHaveFocus();
  expect(row).not.toHaveBeenCalled();
});

it("passes selected repeats to Play next without replacing the queue", async () => {
  const { anchor, onEnqueue, onPlay } = setup({ tracks: [song, song], label: "Selection" });
  fireEvent.contextMenu(anchor, { clientX: 10, clientY: 20 });
  fireEvent.click(screen.getByRole("menuitem", { name: "Play next" }));
  await waitFor(() => expect(onEnqueue).toHaveBeenCalledWith([song, song], true));
  expect(onPlay).not.toHaveBeenCalled();
  expect(await screen.findByRole("status")).toHaveTextContent("Added to play next.");
});

it("offers three recently worked-on regular playlists and guards their displayed revision", async () => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  vi.mocked(invoke).mockResolvedValue({ id: 1 });
  rememberPlaylistWork(1);
  const { anchor, onPlaylistSaved } = setup(undefined, [playlist(1, "2025-01-01"), playlist(2, "2026-02-01"), playlist(3, "2026-03-01"), playlist(4, "2026-04-01"), playlist(5, "2026-05-01", false)]);
  fireEvent.contextMenu(anchor);
  fireEvent.click(screen.getByRole("menuitem", { name: "Add to playlist" }));
  expect(within(screen.getByRole("menu", { name: "Add to playlist" })).getAllByRole("menuitem").map(item => item.textContent)).toEqual(["‹ Back", "Playlist 1", "Playlist 4", "Playlist 3", "Choose playlist…", "Create playlist…"]);
  fireEvent.click(screen.getByRole("menuitem", { name: "Playlist 1" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("author_music_library_playlist", { input: { action: "append", id: 1, expectedUpdatedAt: "2025-01-01", tracks: [{ id: song.id, trackKey: song.trackKey }], albumIds: undefined } }));
  expect(onPlaylistSaved).toHaveBeenCalledOnce();
});

it("offers creation with the captured selection", async () => {
  const { anchor, openPlaylist } = setup();
  fireEvent.contextMenu(anchor);
  fireEvent.click(screen.getByRole("menuitem", { name: "Add to playlist" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Create playlist…" }));
  await waitFor(() => expect(openPlaylist).toHaveBeenCalledWith({ tracks: [song], label: song.title, playlistMode: "create" }));
});

it("keeps unmatched chart rows useful without queueing other songs by that artist", async () => {
  const { anchor, onOpenArtist, onEnqueue } = setup({ label: "Missing song", artistName: "M83" });
  fireEvent.contextMenu(anchor);
  expect(screen.getByRole("menuitem", { name: "Play next" })).toBeDisabled();
  expect(screen.getByRole("menuitem", { name: /^Show in / })).toBeDisabled();
  expect(screen.getByRole("menuitem", { name: "Go to artist" })).toBeEnabled();
  fireEvent.click(screen.getByRole("menuitem", { name: "Go to artist" }));
  await waitFor(() => expect(onOpenArtist).toHaveBeenCalledWith("M83"));
  expect(onEnqueue).not.toHaveBeenCalled();
});

it("resolves matched chart songs for Tags without fetching the artist's songs", async () => {
  const loadTrack = vi.fn(async () => song);
  const { anchor, onOpenTags } = setup({ label: song.title, artistName: "M83", loadTrack });
  fireEvent.contextMenu(anchor);
  expect(screen.getByRole("menuitem", { name: "Open Tags" })).toBeEnabled();
  fireEvent.click(screen.getByRole("menuitem", { name: "Open Tags" }));
  await waitFor(() => expect(onOpenTags).toHaveBeenCalledWith({ tracks: [song], label: song.title }));
  expect(loadTrack).toHaveBeenCalledOnce();
});

it("shows a stale playlist error without announcing success or retrying the write", async () => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  vi.mocked(invoke).mockRejectedValue(new Error("This playlist changed. Refresh and try again."));
  const { anchor, onPlaylistSaved } = setup(undefined, [playlist(1, "revision")]);
  fireEvent.contextMenu(anchor);
  fireEvent.click(screen.getByRole("menuitem", { name: "Add to playlist" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Playlist 1" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("This playlist changed");
  expect(onPlaylistSaved).not.toHaveBeenCalled();
  expect(invoke).toHaveBeenCalledOnce();
});
