import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { Track } from "../../library";
import { loadMusicLibraryPlaylist } from "../../playlists";
import { PlaylistsPage } from "./PlaylistsPage";

vi.mock("../../playlists", () => ({
  loadMusicLibraryPlaylist: vi.fn(),
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
});
