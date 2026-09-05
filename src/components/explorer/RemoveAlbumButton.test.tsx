import { useState, StrictMode } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ingest from "../../ingest";
import * as inbox from "../../inbox";
import { RemoveAlbumButton } from "./RemoveAlbumButton";
import { AlbumMoveOperation, type AlbumMoveRequest } from "./AlbumMoveOperation";
beforeEach(() => { HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); }; });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
const album = { id: "album-1", title: "Night Geometry" };
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function Harness({ mode = "remove", onRemoved }: { mode?: "remove" | "inbox"; onRemoved: (id: string, warnings: string[], destination: string) => Promise<void> }) {
  const [selected, setSelected] = useState(album);
  const [request, setRequest] = useState<AlbumMoveRequest | null>(null);
  return <>
    <h1>{selected.title}</h1>
    <RemoveAlbumButton key={selected.id} onRequest={() => setRequest({ album: selected, mode })} disabled={request !== null} />
    <button onClick={() => setSelected({ id: "other-album", title: "Currently playing album" })}>Go to playing album</button>
    {request && <AlbumMoveOperation request={request} onDismiss={() => setRequest(null)} onRemoved={onRemoved} />}
  </>;
}
describe("album moves across navigation", () => {
  function removalPreview(canApply = true): ingest.LibraryIntakePreview {
    return {
      planId: "remove-plan", sessionId: 7, sourcePath: "H:\\Synthwave\\Night Geometry",
      category: { id: "inbox", label: "Removed albums", destinationRoot: "D:\\MUSIC\\_NOT\\_ALBUMS" },
      albumCount: 1, trackCount: 12, canApply,
      delta: { addedTracks: 0, changedTracks: 0, removedTracks: 12, addedAlbums: 0, changedAlbums: 0, removedAlbums: 1 },
      albums: [{ sourcePath: "H:\\Synthwave\\Night Geometry", destinationPath: "D:\\MUSIC\\_NOT\\_ALBUMS\\Night Geometry", artist: "Aurora Lines", album: "Night Geometry", year: "1985", trackCount: 12, action: "remove", existingTrackCount: 12, matchedTrackCount: 0, existingRatedTrackCount: 0, existingLovedTrackCount: 0 }],
    };
  }

  const result: ingest.LibraryIntakeApplyResult = {
    planId: "remove-plan", sessionId: 7, status: "completed", albumCount: 1, trackCount: 12,
    movedAlbumCount: 1, importRunId: 2, backupPath: null, albums: [], cleanupWarnings: [],
  };
  it.each(["remove", "inbox"] as const)("keeps a %s preview and apply attached to the original album after navigation", async (mode) => {
    const pendingPreview = deferred<ingest.LibraryIntakePreview>();
    const preview = mode === "remove"
      ? vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockReturnValue(pendingPreview.promise)
      : vi.spyOn(ingest.libraryIntakeAdapter, "previewMoveToInbox").mockReturnValue(pendingPreview.promise);
    vi.spyOn(inbox, "loadInboxSettings").mockResolvedValue({ monitoredFolders: ["C:\\Inbox"], discogsConfigured: false, discogsAuthMode: null, discogsIncompleteConsumerKey: false, lastFmConfigured: false, lastFmSecretConfigured: false, warning: null });
    const pendingApply = deferred<ingest.LibraryIntakeApplyResult>();
    const apply = vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockReturnValue(pendingApply.promise);
    const onRemoved = vi.fn().mockResolvedValue(undefined);
    render(<StrictMode><Harness mode={mode} onRemoved={onRemoved} /></StrictMode>);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to playing album" }));
    expect(screen.getByRole("heading", { name: "Currently playing album" })).toBeInTheDocument();
    pendingPreview.resolve(removalPreview());
    fireEvent.click(await screen.findByRole("button", { name: "Review move" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Night Geometry");
    expect(dialog).not.toHaveTextContent("Currently playing album");
    expect(preview).toHaveBeenCalledTimes(1);
    expect(apply).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: mode === "remove" ? "Remove Album" : "Move to Inbox" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Go to playing album" }));
    pendingApply.resolve({ ...result, cleanupWarnings: ["Source retained for recovery"] });
    await waitFor(() => expect(onRemoved).toHaveBeenCalledWith("album-1", ["Source retained for recovery"], removalPreview().albums[0].destinationPath));
    expect(await screen.findByRole("alert")).toHaveTextContent("Source retained for recovery");
    expect(screen.getByRole("heading", { name: "Currently playing album" })).toBeInTheDocument();
    expect(apply).toHaveBeenCalledTimes(1);
  });
  it("retains a late preview error after the initiating sidebar unmounts", async () => {
    const pending = deferred<ingest.LibraryIntakePreview>();
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockReturnValue(pending.promise);
    render(<Harness onRemoved={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to playing album" }));
    pending.reject(new Error("Music Library needs updating"));
    expect(await screen.findByRole("alert")).toHaveTextContent("Music Library needs updating");
    expect(screen.getByRole("alert")).toHaveTextContent("Night Geometry");
  });
  it("blocks unsafe previews and cancellation never applies", async () => {
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockResolvedValue(removalPreview(false));
    const apply = vi.spyOn(ingest.libraryIntakeAdapter, "apply");
    render(<Harness onRemoved={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    fireEvent.click(await screen.findByRole("button", { name: "Review move" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("button", { name: "Remove Album" })).toBeDisabled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(apply).not.toHaveBeenCalled();
  });
  it("keeps apply failure visible after navigation without clearing the current album", async () => {
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockResolvedValue(removalPreview());
    const pending = deferred<ingest.LibraryIntakeApplyResult>();
    vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockReturnValue(pending.promise);
    const onRemoved = vi.fn();
    render(<Harness onRemoved={onRemoved} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Album" }));
    fireEvent.click(await screen.findByRole("button", { name: "Review move" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Remove Album" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to playing album" }));
    pending.reject(new Error("Destination occupied"));
    expect(await screen.findByRole("alert")).toHaveTextContent("Destination occupied");
    expect(onRemoved).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Currently playing album" })).toBeInTheDocument();
  });
});
