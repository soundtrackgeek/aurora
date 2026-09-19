import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { libraryIntakeAdapter, type LibraryIntakePreview, type LibraryIntakeSelectionRequest } from "../../ingest";
import { InboxLibraryIntakeDialog } from "./InboxLibraryIntakeDialog";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const targets = Array.from({ length: 6 }, (_, index) => ({
  sourcePath: `C:\\Inbox\\Album ${index}`, label: `Album ${index}`, albumCount: 1, unreadyAlbumCount: 0, albumOnly: true,
}));

function preview(request: LibraryIntakeSelectionRequest, sessionId = 1): LibraryIntakePreview {
  return {
    planId: `batch-${sessionId}`, sessionId, sourcePath: targets[0].sourcePath,
    category: { id: "general", label: "General", destinationRoot: "D:\\Music" },
    albumCount: 6, trackCount: 6, canApply: true,
    delta: { addedAlbums: 6, changedAlbums: 0, removedAlbums: 0, addedTracks: 6, changedTracks: 0, removedTracks: 0 },
    albums: request.targets.map((target, index) => ({
      sourcePath: target.sourcePath, destinationPath: `D:\\${target.category}\\Album ${index}`,
      artist: "Artist", album: `Album ${index}`, year: "2026", trackCount: 1, action: "add",
      existingTrackCount: 0, matchedTrackCount: 0, existingRatedTrackCount: 0, existingLovedTrackCount: 0,
    })),
  };
}

async function review() {
  render(<InboxLibraryIntakeDialog scopeLabel="selected albums" targets={targets} onClose={vi.fn()} onApplied={vi.fn()} onCompleted={vi.fn()} />);
  fireEvent.change(screen.getByRole("combobox", { name: "Destination for all albums" }), { target: { value: "general" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Album 5" }), { target: { value: "synthwave" } });
  fireEvent.click(screen.getByRole("button", { name: "Preview destinations" }));
}

it("retries the entire unchanged six-album batch after a stale catalog preview", async () => {
  let session = 0;
  const prepare = vi.spyOn(libraryIntakeAdapter, "previewSelection").mockImplementation(async (request) => preview(request, ++session));
  const apply = vi.spyOn(libraryIntakeAdapter, "apply")
    .mockRejectedValueOnce(new Error("Catalog changed (stalePlan)"))
    .mockImplementation(() => new Promise(() => {}));
  await review();
  fireEvent.click(await screen.findByRole("button", { name: "Add 6 albums" }));
  await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
  expect(prepare).toHaveBeenCalledTimes(2);
  expect(apply).toHaveBeenNthCalledWith(1, { planId: "batch-1", sessionId: 1 });
  expect(apply).toHaveBeenNthCalledWith(2, { planId: "batch-2", sessionId: 2 });
  expect(screen.getByRole("status")).toHaveTextContent("6 albums:");
  expect(screen.getByRole("status")).not.toHaveTextContent("1 of 6");
});

it("requires review again if a later album changes during a stale-plan retry", async () => {
  let session = 0;
  vi.spyOn(libraryIntakeAdapter, "previewSelection").mockImplementation(async (request) => {
    const result = preview(request, ++session);
    if (session > 1) result.albums[5].action = "replace";
    return result;
  });
  const apply = vi.spyOn(libraryIntakeAdapter, "apply").mockRejectedValue(new Error("Catalog changed (stalePlan)"));
  await review();
  fireEvent.click(await screen.findByRole("button", { name: "Add 6 albums" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Album 5 changed after review");
  expect(apply).toHaveBeenCalledTimes(1);
});

it("keeps replacement confirmation required for the combined batch", async () => {
  vi.spyOn(libraryIntakeAdapter, "previewSelection").mockImplementation(async (request) => {
    const result = preview(request);
    result.albums[5].action = "replace";
    return result;
  });
  const apply = vi.spyOn(libraryIntakeAdapter, "apply").mockImplementation(() => new Promise(() => {}));
  await review();
  const add = await screen.findByRole("button", { name: "Add 6 albums" });
  expect(add).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: /I reviewed these replacements/ }));
  fireEvent.click(add);
  await waitFor(() => expect(apply).toHaveBeenCalledExactlyOnceWith({ planId: "batch-1", sessionId: 1 }));
});

it("does not fall back to individual imports when the helper needs updating", async () => {
  vi.spyOn(libraryIntakeAdapter, "previewSelection").mockRejectedValue(new Error("Update Music Library for batch intake."));
  const individual = vi.spyOn(libraryIntakeAdapter, "preview");
  const apply = vi.spyOn(libraryIntakeAdapter, "apply");
  await review();
  expect(await screen.findByRole("alert")).toHaveTextContent("Update Music Library");
  expect(individual).not.toHaveBeenCalled();
  expect(apply).not.toHaveBeenCalled();
});
