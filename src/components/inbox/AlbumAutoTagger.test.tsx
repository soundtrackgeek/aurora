import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as inboxAdapter from "../../inbox";
import * as libraryAutoTag from "../../libraryAutoTag";
import { AlbumAutoTagger } from "./AlbumAutoTagger";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("AlbumAutoTagger for a library album", () => {
  it("applies the reviewed release in place and asks for the folder rename", async () => {
    const album = (await inboxAdapter.loadInboxSnapshot()).albums[0];
    const inboxApply = vi.spyOn(inboxAdapter, "applyInboxTags");
    const apply = vi.spyOn(libraryAutoTag, "applyLibraryAutoTags").mockResolvedValue({
      changedTracks: album.tracks.length,
      removedTracks: 0,
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
      rename: {
        albumId: "library-album",
        sourcePath: album.path,
        destinationPath: "D:\\Music\\Baltimoore - Freak (1994)",
        folderRenamed: true,
        renamedFiles: album.tracks.length,
        warnings: [],
      },
    });
    const onApplied = vi.fn();
    render(<AlbumAutoTagger
      albums={[album]}
      tracks={album.tracks}
      discogsConfigured
      onOpenSettings={vi.fn()}
      onClose={vi.fn()}
      onApplied={onApplied}
      library={{ albumId: "library-album", coverUrl: null }}
    />);

    expect(screen.getByText("Library metadata")).toBeInTheDocument();
    expect(screen.getByText("Rename folder after tagging")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Apply & rename" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Apply & rename" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(apply.mock.calls[0][0]).toMatchObject({
      albumId: "library-album",
      renameFolder: true,
      tags: { albumPath: album.path, removeTrackPaths: [] },
    });
    expect(apply.mock.calls[0][0].tags.tracks).toHaveLength(album.tracks.length);
    expect(inboxApply).not.toHaveBeenCalled();
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(
      expect.stringContaining("Renamed the folder to Baltimoore - Freak (1994)"),
      "D:\\Music\\Baltimoore - Freak (1994)",
      expect.objectContaining({ changedTracks: album.tracks.length }),
    ));
  });
});
