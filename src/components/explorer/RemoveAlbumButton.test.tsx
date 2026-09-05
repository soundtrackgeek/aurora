import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ingest from "../../ingest";
import { RemoveAlbumButton } from "./RemoveAlbumButton";
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
describe("RemoveAlbumButton", () => {
  function removalPreview(canApply = true): ingest.LibraryIntakePreview {
    return {
      planId: "remove-plan", sessionId: 7, sourcePath: "H:\\Synthwave\\Night Geometry",
      category: { id: "inbox", label: "Removed albums", destinationRoot: "D:\\MUSIC\\_NOT\\_ALBUMS" },
      albumCount: 1, trackCount: 12, canApply,
      delta: { addedTracks: 0, changedTracks: 0, removedTracks: 12, addedAlbums: 0, changedAlbums: 0, removedAlbums: 1 },
      albums: [{ sourcePath: "H:\\Synthwave\\Night Geometry", destinationPath: "D:\\MUSIC\\_NOT\\_ALBUMS\\Night Geometry", artist: "Aurora Lines", album: "Night Geometry", year: "1985", trackCount: 12, action: "remove", existingTrackCount: 12, matchedTrackCount: 0, existingRatedTrackCount: 0, existingLovedTrackCount: 0 }],
    };
  }

  it("requires confirmation before removing an album and refreshes only after commit, forwarding cleanup warnings", async () => {
    const preview = vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockResolvedValue(removalPreview());
    const apply = vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockResolvedValue({
      planId: "remove-plan", sessionId: 7, status: "completedWithWarnings", albumCount: 1,
      trackCount: 12, movedAlbumCount: 1, importRunId: 2, backupPath: null, albums: [],
      cleanupWarnings: ["Source retained for recovery"],
    });
    const onAlbumRemoved = vi.fn();
    render(<RemoveAlbumButton album={{ id: "album-1", title: "Night Geometry" }} onRemoved={onAlbumRemoved} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove “Night Geometry”?" });
    expect(preview).toHaveBeenCalledWith("album-1");
    expect(dialog).toHaveTextContent("D:\\MUSIC\\_NOT\\_ALBUMS\\Night Geometry");
    expect(apply).not.toHaveBeenCalled();
    expect(onAlbumRemoved).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove Album" }));
    await waitFor(() => expect(onAlbumRemoved).toHaveBeenCalledWith("album-1", ["Source retained for recovery"]));
    expect(apply).toHaveBeenCalledWith({ planId: "remove-plan", sessionId: 7 });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("does not apply a blocked removal preview or remove the row on failure", async () => {
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockResolvedValue(removalPreview(false));
    const apply = vi.spyOn(ingest.libraryIntakeAdapter, "apply");
    const onAlbumRemoved = vi.fn();
    render(<RemoveAlbumButton album={{ id: "album-1", title: "Night Geometry" }} onRemoved={onAlbumRemoved} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("button", { name: "Remove Album" })).toBeDisabled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(apply).not.toHaveBeenCalled();
    expect(onAlbumRemoved).not.toHaveBeenCalled();
  });

  it("keeps the removal confirmation open when the bridge fails", async () => {
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockResolvedValue(removalPreview());
    vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockRejectedValue(new Error("Destination is occupied"));
    const onAlbumRemoved = vi.fn();
    render(<RemoveAlbumButton album={{ id: "album-1", title: "Night Geometry" }} onRemoved={onAlbumRemoved} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove Album" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Destination is occupied");
    expect(onAlbumRemoved).not.toHaveBeenCalled();
    expect(dialog).toBeInTheDocument();
  });

});
