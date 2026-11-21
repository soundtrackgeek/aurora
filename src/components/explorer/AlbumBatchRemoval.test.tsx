import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ingest from "../../ingest";
import { AlbumBatchRemoval, type AlbumBatchRemovalRequest } from "./AlbumBatchRemoval";
import { isLibraryBusyFailure } from "./albumBatchRemovalRetry";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  vi.spyOn(ingest, "listenLibraryIntakeProgress").mockResolvedValue(() => {});
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const request: AlbumBatchRemovalRequest = {
  albums: [
    { id: "a1", title: "Album One", artist: "Whale", totalTracks: 10 },
    { id: "a2", title: "Album Two", artist: "Whale", totalTracks: 11 },
    { id: "a3", title: "Album Three", artist: "Whale", totalTracks: 12 },
  ],
};

function preview(albumId: string, canApply = true): ingest.LibraryIntakePreview {
  return {
    planId: `plan-${albumId}`, sessionId: 7, sourcePath: `H:\\Whale\\${albumId}`,
    category: { id: "inbox", label: "Removed albums", destinationRoot: "D:\\MUSIC_NOT_ALBUMS" },
    albumCount: 1, trackCount: 10, canApply,
    delta: { addedTracks: 0, changedTracks: 0, removedTracks: 10, addedAlbums: 0, changedAlbums: 0, removedAlbums: 1 },
    albums: [{ sourcePath: `H:\\Whale\\${albumId}`, destinationPath: `D:\\MUSIC_NOT_ALBUMS\\${albumId}`, artist: "Whale", album: albumId, year: "1995", trackCount: 10, action: "remove", existingTrackCount: 10, matchedTrackCount: 10, existingRatedTrackCount: 0, existingLovedTrackCount: 0 }],
  };
}

function applied(planId: string): ingest.LibraryIntakeApplyResult {
  return { planId, sessionId: 7, status: "completed", albumCount: 1, trackCount: 10, movedAlbumCount: 1, importRunId: 2, backupPath: null, albums: [], cleanupWarnings: [] };
}

describe("removing several albums", () => {
  it("lists the selection for review and touches nothing until confirmed", () => {
    const previewSpy = vi.spyOn(ingest, "previewLibraryRemoveAlbum");
    const onDismiss = vi.fn();
    render(<AlbumBatchRemoval request={request} onDismiss={onDismiss} onAlbumRemoved={vi.fn()} onFinished={vi.fn()} />);
    expect(screen.getByRole("heading", { name: "Remove 3 Albums?" })).toBeInTheDocument();
    expect(screen.getByText("Whale – Album Two")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onDismiss).toHaveBeenCalled();
    expect(previewSpy).not.toHaveBeenCalled();
  });

  it("previews and applies one album at a time, in order", async () => {
    const calls: string[] = [];
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockImplementation(async (albumId) => {
      calls.push(`preview:${albumId}`);
      return preview(albumId);
    });
    vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockImplementation(async ({ planId }) => {
      calls.push(`apply:${planId}`);
      return applied(planId);
    });
    const onAlbumRemoved = vi.fn();
    const onFinished = vi.fn().mockResolvedValue(undefined);
    render(<AlbumBatchRemoval request={request} onDismiss={vi.fn()} onAlbumRemoved={onAlbumRemoved} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove 3 Albums" }));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
    // A new preview discards an unapplied plan, so each album must be applied before the next preview.
    expect(calls).toEqual(["preview:a1", "apply:plan-a1", "preview:a2", "apply:plan-a2", "preview:a3", "apply:plan-a3"]);
    expect(onAlbumRemoved.mock.calls.map(([id]) => id)).toEqual(["a1", "a2", "a3"]);
    expect(onFinished).toHaveBeenCalledWith(expect.objectContaining({ removed: 3, total: 3, failure: null, stopped: false }));
    expect(await screen.findByRole("status")).toHaveTextContent("Removed 3 of 3 albums");
  });

  it("stops at a blocked album and leaves the rest untouched", async () => {
    const previewSpy = vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockImplementation(async (albumId) => preview(albumId, albumId !== "a2"));
    const apply = vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockImplementation(async ({ planId }) => applied(planId));
    const onFinished = vi.fn().mockResolvedValue(undefined);
    render(<AlbumBatchRemoval request={request} onDismiss={vi.fn()} onAlbumRemoved={vi.fn()} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove 3 Albums" }));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
    expect(previewSpy.mock.calls.map(([id]) => id)).toEqual(["a1", "a2"]);
    expect(apply).toHaveBeenCalledTimes(1);
    expect(onFinished).toHaveBeenCalledWith(expect.objectContaining({ removed: 1, failure: expect.stringContaining("Whale – Album Two") }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The remaining 1 albums were not touched.");
  });

  it("finishes the album in progress before stopping on request", async () => {
    let finishFirst!: () => void;
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockImplementation(async (albumId) => preview(albumId));
    vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockImplementation(({ planId }) => new Promise((resolve) => {
      finishFirst = () => resolve(applied(planId));
    }));
    const onFinished = vi.fn().mockResolvedValue(undefined);
    render(<AlbumBatchRemoval request={request} onDismiss={vi.fn()} onAlbumRemoved={vi.fn()} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove 3 Albums" }));
    fireEvent.click(await screen.findByRole("button", { name: "Stop after this album" }));
    await waitFor(() => expect(finishFirst).toBeDefined());
    finishFirst();
    await waitFor(() => expect(onFinished).toHaveBeenCalledWith(expect.objectContaining({ removed: 1, stopped: true, failure: null })));
  });

  const busy = "Music Library could not complete applyBatch: Copied destination folders were removed after exact verification; sources remain: The catalog apply failed; sources remain: Music Library database is locked by another operation; retry the removal after it finishes (transferFailed)";

  it("recognises only Music Library's busy catalog failures as retryable", () => {
    expect(isLibraryBusyFailure(busy)).toBe(true);
    expect(isLibraryBusyFailure("The catalog changed while its recovery backup was being prepared; retry the removal")).toBe(true);
    expect(isLibraryBusyFailure("The removal destination already exists")).toBe(false);
  });

  it("retries an album with a fresh preview when the catalog was busy, then continues", async () => {
    const calls: string[] = [];
    let failOnce = true;
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockImplementation(async (albumId) => {
      calls.push(`preview:${albumId}`);
      return preview(albumId);
    });
    vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockImplementation(async ({ planId }) => {
      calls.push(`apply:${planId}`);
      if (planId === "plan-a2" && failOnce) {
        failOnce = false;
        throw new Error(busy);
      }
      return applied(planId);
    });
    const onFinished = vi.fn().mockResolvedValue(undefined);
    render(<AlbumBatchRemoval request={request} busyRetryDelaysMs={[0, 0, 0]} onDismiss={vi.fn()} onAlbumRemoved={vi.fn()} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove 3 Albums" }));
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
    expect(calls).toEqual(["preview:a1", "apply:plan-a1", "preview:a2", "apply:plan-a2", "preview:a2", "apply:plan-a2", "preview:a3", "apply:plan-a3"]);
    expect(onFinished).toHaveBeenCalledWith(expect.objectContaining({ removed: 3, failure: null }));
  });

  it("does not retry other failures, and gives up after the last busy retry", async () => {
    vi.spyOn(ingest, "previewLibraryRemoveAlbum").mockImplementation(async (albumId) => preview(albumId));
    const apply = vi.spyOn(ingest.libraryIntakeAdapter, "apply").mockRejectedValue(new Error(busy));
    const onFinished = vi.fn().mockResolvedValue(undefined);
    const { unmount } = render(<AlbumBatchRemoval request={request} busyRetryDelaysMs={[0, 0]} onDismiss={vi.fn()} onAlbumRemoved={vi.fn()} onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove 3 Albums" }));
    await waitFor(() => expect(onFinished).toHaveBeenCalledWith(expect.objectContaining({ removed: 0, failure: expect.stringContaining("locked") })));
    expect(apply).toHaveBeenCalledTimes(3);
    unmount();

    apply.mockClear();
    apply.mockRejectedValue(new Error("The removal destination already exists"));
    const second = vi.fn().mockResolvedValue(undefined);
    render(<AlbumBatchRemoval request={request} busyRetryDelaysMs={[0, 0]} onDismiss={vi.fn()} onAlbumRemoved={vi.fn()} onFinished={second} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove 3 Albums" }));
    await waitFor(() => expect(second).toHaveBeenCalled());
    expect(apply).toHaveBeenCalledTimes(1);
  });
});
