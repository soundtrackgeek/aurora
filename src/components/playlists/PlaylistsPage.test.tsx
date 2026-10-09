import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { Track } from "../../library";
import { authorPlaylist, exportPlaylist, previewPlaylistImport, savePlaylistImport, loadMusicLibraryPlaylist } from "../../playlists";
import { PlaylistsPage } from "./PlaylistsPage";

vi.mock("../../playlists", () => ({
  loadMusicLibraryPlaylist: vi.fn(),
  authorPlaylist: vi.fn(), exportPlaylist: vi.fn(), previewPlaylistImport: vi.fn(), savePlaylistImport: vi.fn(),
}));
afterEach(cleanup);

const tracks = [
  { id: "2", trackKey: "b", title: "Second", artist: "Artist B", album: "Album B", durationSeconds: 120 },
  { id: "1", trackKey: "a", title: "First", artist: "Artist A", album: "Album A", durationSeconds: 180 },
] as Track[];

it("shows Music Library order and starts playback at the chosen song", async () => {
  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValue({
    id: 3, name: "Saved", description: "From Music Library", trackCount: 3,
    missingCount: 1, tracks,
  });
  const play = vi.fn(async () => true);
  render(<PlaylistsPage
    active playlists={[{ id: 3, name: "Saved", description: "", trackCount: 3, updatedAt: "today" }]}
    selectedId={3} listLoading={false} listError={null}
    onSelect={vi.fn()} onRefresh={vi.fn()} onPlay={play}
  />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Play First from here" })).toBeInTheDocument());
  expect(screen.getByText("Second").compareDocumentPosition(screen.getByText("First")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(screen.getByText(/1 unavailable in the current catalog/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Play First from here" }));
  await waitFor(() => expect(play).toHaveBeenCalledWith(tracks, 1));
  fireEvent.click(screen.getByRole("button", { name: "Shuffle playlist" }));
  await waitFor(() => expect(play).toHaveBeenCalledWith(tracks, 0, true));
});

it("disables both playlist actions when no saved songs are available", async () => {
  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValue({
    id: 4, name: "Unavailable", description: "", trackCount: 1,
    missingCount: 1, tracks: [],
  });
  render(<PlaylistsPage
    active playlists={[{ id: 4, name: "Unavailable", description: "", trackCount: 1, updatedAt: "today" }]}
    selectedId={4} listLoading={false} listError={null}
    onSelect={vi.fn()} onRefresh={vi.fn()} onPlay={vi.fn(async () => true)}
  />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Shuffle playlist" })).toBeDisabled());
  expect(screen.getByRole("button", { name: "Play playlist" })).toBeDisabled();
});

function authoringPage() {
  const refresh=vi.fn();const select=vi.fn();
  render(<PlaylistsPage active playlists={[{id:3,name:"Saved",description:"",trackCount:4,updatedAt:"rev"}]} selectedId={3} listLoading={false} listError={null} onSelect={select} onRefresh={refresh} onPlay={vi.fn(async()=>true)} />);
  return {refresh,select};
}

it("edits saved positions across unavailable songs and retains the opened revision on rename",async()=>{
  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValue({id:3,name:"Saved",description:"",trackCount:4,missingCount:2,tracks,positions:[1,3],revision:"rev",editable:true});
  vi.mocked(authorPlaylist).mockResolvedValue({id:3});
  const {refresh}=authoringPage();
  fireEvent.click(await screen.findByRole("button",{name:"Move Second up"}));
  await waitFor(()=>expect(authorPlaylist).toHaveBeenCalledWith({action:"move",id:3,expectedUpdatedAt:"rev",from:1,to:0}));
  await waitFor(()=>expect(refresh).toHaveBeenCalled());
  fireEvent.click(await screen.findByRole("button",{name:"Rename"}));
  fireEvent.change(screen.getByLabelText("Name"),{target:{value:"New name"}});
  vi.mocked(authorPlaylist).mockRejectedValue(new Error("This playlist changed in another app"));
  fireEvent.submit(screen.getByRole("form",{name:"Playlist authoring"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("changed in another app");
  expect(authorPlaylist).toHaveBeenLastCalledWith({action:"rename",id:3,expectedUpdatedAt:"rev",name:"New name"});
});

it("previews import before saving and confirms shared deletion",async()=>{
  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValue({id:3,name:"Saved",description:"",trackCount:2,missingCount:0,tracks,revision:"rev"});
  const preview={path:"songs.m3u8",fingerprint:"digest",name:"Imported",trackCount:3,samples:["first.mp3","second.mp3","first.mp3"]};
  vi.mocked(previewPlaylistImport).mockResolvedValue(preview);
  vi.mocked(savePlaylistImport).mockResolvedValue({id:9});
  vi.mocked(authorPlaylist).mockResolvedValue({id:3});
  const {select}=authoringPage();
  fireEvent.click(screen.getByRole("button",{name:"Import M3U8"}));
  expect(await screen.findByText(/3 songs resolved/)).toBeInTheDocument();
  expect(savePlaylistImport).not.toHaveBeenCalled();
  fireEvent.submit(screen.getByRole("form",{name:"Playlist authoring"}));
  await waitFor(()=>expect(savePlaylistImport).toHaveBeenCalledWith(preview,"Imported"));
  fireEvent.click(await screen.findByRole("button",{name:"Export M3U8"}));
  await waitFor(()=>expect(exportPlaylist).toHaveBeenCalledWith(3,"rev"));
  fireEvent.click(screen.getByRole("button",{name:"Delete playlist"}));
  expect(screen.getByText(/Deletes the shared playlist/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Delete shared playlist"}));
  await waitFor(()=>expect(authorPlaylist).toHaveBeenLastCalledWith({action:"delete",id:3,expectedUpdatedAt:"rev",name:"Saved"}));
  await waitFor(()=>expect(select).toHaveBeenLastCalledWith(null));
});
