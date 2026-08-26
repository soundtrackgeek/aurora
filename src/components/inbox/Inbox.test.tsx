import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as inboxAdapter from "../../inbox";
import { libraryIntakeAdapter, type LibraryIntakeCategoryId, type LibraryIntakePreview } from "../../ingest";
import { Inbox } from "./Inbox";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Inbox", () => {
  it("keeps staged albums outside the library and opens Auto-Tagger from Ctrl+Shift+T", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByText("1 album outside the library")).toBeInTheDocument();
    expect(screen.getByText("1 issue")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Freak cover" })).toHaveAttribute(
      "src",
      "/__aurora-preview-cover/preview-freak?size=128",
    );
    await waitFor(() => expect(screen.getByRole("button", { name: /Auto-tag.*Ctrl Shift T/ })).toBeEnabled());

    fireEvent.keyDown(window, { key: "t", ctrlKey: true, shiftKey: true });

    expect(await screen.findByRole("dialog", { name: "Album Auto-Tagger" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByText("MusicBrainz").length).toBeGreaterThan(0));
    expect(screen.getAllByText("Discogs").length).toBeGreaterThan(0);
    expect(screen.getByLabelText("Release title 1")).toHaveValue("Memories Calling");
    expect(screen.getByRole("checkbox", { name: /Rename after tagging/ })).toBeChecked();
    expect(screen.getByRole("button", { name: "Apply & rename" })).toBeEnabled();
  });

  it("renames a manually tagged album from Ctrl+R", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });

    expect(await screen.findByText("10 tracks renamed with the album folder.")).toBeInTheDocument();
  });

  it("uses the vertical tag editor for album batches and individual Inbox tracks", async () => {
    const apply = vi.spyOn(inboxAdapter, "applyInboxTags").mockResolvedValue({
      changedTracks: 10,
      renamedTracks: 0,
      albumPath: "C:\\Music\\Inbox\\Baltimoore - Freak",
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("tab", { name: "Tags" }));

    expect(await screen.findByText("10 MP3s")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Genre" }), { target: { value: "Hard Rock" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 10 MP3s" }));

    await waitFor(() => expect(apply).toHaveBeenCalledWith(expect.objectContaining({
      fields: ["genre"],
      renameAfterApply: false,
      tracks: expect.arrayContaining([expect.objectContaining({ path: expect.stringContaining("Memories Calling") })]),
    })));
    expect(apply.mock.calls[0]?.[0].tracks).toHaveLength(10);

    fireEvent.click(screen.getByRole("button", { name: "None" }));
    fireEvent.click(screen.getByRole("checkbox", { name: /01.*Memories Calling/ }));
    expect(await screen.findByText("1 MP3")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Track title" }), { target: { value: "Memories Calling (Edit)" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 1 MP3" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
    expect(apply.mock.calls[1]?.[0]).toMatchObject({
      fields: ["title"],
      renameAfterApply: false,
      tracks: [{ path: expect.stringContaining("Memories Calling") }],
    });
  });

  it("auto-tags only selected disc tracks and exposes disc overrides", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("button", { name: "None" }));
    fireEvent.click(screen.getByRole("checkbox", { name: /01.*Memories Calling/ }));
    fireEvent.click(screen.getByRole("checkbox", { name: /02.*Kahlua Confusion/ }));
    fireEvent.keyDown(window, { key: "t", ctrlKey: true, shiftKey: true });

    expect(await screen.findByRole("dialog", { name: "Album Auto-Tagger" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByLabelText("Release title 1")).toHaveValue("Memories Calling"));
    expect(screen.getByLabelText("Release title 2")).toHaveValue("Kahlua Confusion");
    expect(screen.queryByLabelText("Release title 3")).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /Rename after tagging/ })).toBeDisabled();
    expect(screen.getByLabelText("Disc number override")).toHaveValue("1");
    expect(screen.getByRole("button", { name: "Apply to 2 tracks" })).toBeEnabled();
  });

  it("keeps the selected release stable while the Inbox refreshes", async () => {
    const search = vi.spyOn(inboxAdapter, "searchInboxReleases");
    const loadSnapshot = vi.spyOn(inboxAdapter, "loadInboxSnapshot");
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.keyDown(window, { key: "t", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(screen.getByLabelText("Release title 1")).toHaveValue("Memories Calling"));
    expect(search).toHaveBeenCalledTimes(1);

    fireEvent.focus(window);
    await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(2));

    expect(search).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText("Release title 1")).toHaveValue("Memories Calling");
    expect(screen.getByRole("button", { name: "Apply & rename" })).toBeEnabled();
  });

  it("requires readiness before previewing a library move", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination" }), { target: { value: "general" } });

    expect(screen.getByRole("button", { name: "Preview move" })).toBeDisabled();
    expect(screen.getByText("Uses the same reviewed, preview-first flow as Add Music.")).toBeInTheDocument();
  });

  it("adds a monitored folder through the reviewed library, cover, and catalog workflow", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({
      ...snapshot,
      albums: snapshot.albums.map((album) => ({ ...album, genre: "Hard Rock", readiness: { ready: true, issues: [] } })),
    });
    const preview = vi.spyOn(libraryIntakeAdapter, "preview").mockImplementation(async ({ sourcePath, category }) => libraryPreview("folder-plan", 41, sourcePath, category, 1));
    const apply = vi.spyOn(libraryIntakeAdapter, "apply").mockResolvedValue({
      planId: "folder-plan", sessionId: 41, status: "completed", albumCount: 1, trackCount: 10,
      movedAlbumCount: 1, importRunId: 7, backupPath: null, cleanupWarnings: [],
      albums: [{ sourcePath: "C:\\Music\\Inbox\\Baltimoore - Freak", destinationPath: "D:\\Music\\Baltimoore - Freak", cleanupStatus: "removed" }],
    });
    const catalogChanged = vi.fn();
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={catalogChanged} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("button", { name: "Add Inbox to library" }));
    expect(screen.getByRole("dialog", { name: "Add Inbox to library" })).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Inbox" }), { target: { value: "general" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview destinations" }));

    await waitFor(() => expect(preview).toHaveBeenCalledWith({ sourcePath: "C:\\Music\\Inbox", category: "general" }));
    expect(await screen.findByText(/1 album · 10 tracks → D:\\Music/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add 1 album" }));

    await waitFor(() => expect(apply).toHaveBeenCalledWith({ planId: "folder-plan", sessionId: 41 }));
    expect(catalogChanged).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("1 album moved, covers archived, and library catalog updated.")).toBeInTheDocument();
  });

  it("previews and applies every non-empty monitored folder from All folders", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    const first = { ...snapshot.albums[0], genre: "Hard Rock", readiness: { ready: true, issues: [] } };
    const second = {
      ...first,
      id: "preview-bandcamp",
      path: "D:\\Bandcamp\\Neon Nights",
      folderName: "Neon Nights",
      album: "Neon Nights",
      tracks: first.tracks.map((track) => ({ ...track, path: track.path.replace("C:\\Music\\Inbox\\Baltimoore - Freak", "D:\\Bandcamp\\Neon Nights") })),
    };
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({ ...snapshot, albums: [first, second] });
    const preview = vi.spyOn(libraryIntakeAdapter, "preview").mockImplementation(async ({ sourcePath, category }) => libraryPreview(sourcePath.includes("Bandcamp") ? "all-plan-2" : "all-plan-1", sourcePath.includes("Bandcamp") ? 52 : 51, sourcePath, category, 1));
    const apply = vi.spyOn(libraryIntakeAdapter, "apply").mockImplementation(async ({ planId, sessionId }) => ({
      planId, sessionId, status: "completed", albumCount: 1, trackCount: 10, movedAlbumCount: 1,
      importRunId: sessionId, backupPath: null, cleanupWarnings: [],
      albums: [{ sourcePath: "source", destinationPath: "destination", cleanupStatus: "removed" }],
    }));
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("button", { name: "Add All folders to library" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Inbox" }), { target: { value: "general" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Bandcamp" }), { target: { value: "synthwave" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview destinations" }));

    await waitFor(() => expect(preview).toHaveBeenCalledTimes(2));
    expect(preview).toHaveBeenNthCalledWith(1, { sourcePath: "C:\\Music\\Inbox", category: "general" });
    expect(preview).toHaveBeenNthCalledWith(2, { sourcePath: "D:\\Bandcamp", category: "synthwave" });
    fireEvent.click(await screen.findByRole("button", { name: "Add 2 albums" }));
    await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("2 albums moved, covers archived, and library catalog updated.")).toBeInTheDocument();
  });
});

function libraryPreview(planId: string, sessionId: number, sourcePath: string, category: LibraryIntakeCategoryId, albumCount: number): LibraryIntakePreview {
  const destinationRoot = category === "general" ? "D:\\Music" : category === "scores" ? "D:\\Scores" : "D:\\Synthwave";
  return {
    planId,
    sessionId,
    sourcePath,
    category: { id: category, label: category, destinationRoot },
    albumCount,
    trackCount: albumCount * 10,
    delta: { addedTracks: albumCount * 10, changedTracks: 0, removedTracks: 0, addedAlbums: albumCount, changedAlbums: 0, removedAlbums: 0 },
    albums: [],
    canApply: true,
  };
}
