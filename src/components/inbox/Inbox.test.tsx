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
  it("selects Inbox album ranges with Shift and toggles albums with Ctrl", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    const first = snapshot.albums[0];
    const albums = ["Freak", "Neon Nights", "Afterglow", "Static Bloom"].map((album, index) => ({
      ...first,
      id: `inbox-${index}`,
      album,
      folderName: album,
      path: `C:\\Music\\Inbox\\${album}`,
      tracks: first.tracks.map((track) => ({
        ...track,
        album,
        path: track.path.replace(first.path, `C:\\Music\\Inbox\\${album}`),
      })),
    }));
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({ ...snapshot, albums });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    const freak = await screen.findByRole("row", { name: /Freak by/ });
    const neon = screen.getByRole("row", { name: /Neon Nights by/ });
    const afterglow = screen.getByRole("row", { name: /Afterglow by/ });
    const staticBloom = screen.getByRole("row", { name: /Static Bloom by/ });

    expect(freak).toHaveAttribute("aria-selected", "true");
    fireEvent.click(afterglow, { shiftKey: true });
    expect(freak).toHaveAttribute("aria-selected", "true");
    expect(neon).toHaveAttribute("aria-selected", "true");
    expect(afterglow).toHaveAttribute("aria-selected", "true");
    expect(staticBloom).toHaveAttribute("aria-selected", "false");
    expect(screen.getByText("3 selected · 4 albums outside the library")).toBeInTheDocument();
    expect(afterglow).toHaveAttribute("aria-current", "true");

    fireEvent.click(neon, { ctrlKey: true });
    expect(freak).toHaveAttribute("aria-selected", "true");
    expect(neon).toHaveAttribute("aria-selected", "false");
    expect(afterglow).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("2 selected · 4 albums outside the library")).toBeInTheDocument();

    fireEvent.click(staticBloom);
    expect(freak).toHaveAttribute("aria-selected", "false");
    expect(afterglow).toHaveAttribute("aria-selected", "false");
    expect(staticBloom).toHaveAttribute("aria-selected", "true");
  });

  it("edits and saves tracks from every selected Inbox album", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    const first = snapshot.albums[0];
    const secondPath = "C:\\Music\\Inbox\\Neon Nights";
    const second = {
      ...first,
      id: "inbox-neon",
      album: "Neon Nights",
      folderName: "Neon Nights",
      path: secondPath,
      tracks: first.tracks.map((track) => ({
        ...track,
        album: "Neon Nights",
        genre: "Electronic",
        path: track.path.replace(first.path, secondPath),
      })),
    };
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({ ...snapshot, albums: [first, second] });
    const apply = vi.spyOn(inboxAdapter, "applyInboxTags").mockImplementation(async (request) => ({
      changedTracks: request.tracks.length,
      renamedTracks: 0,
      albumPath: request.albumPath,
    }));
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    const firstRow = await screen.findByRole("row", { name: /Freak by/ });
    const secondRow = screen.getByRole("row", { name: /Neon Nights by/ });
    fireEvent.click(firstRow);
    fireEvent.click(secondRow, { shiftKey: true });
    fireEvent.click(screen.getByRole("tab", { name: "Tags" }));

    expect(await screen.findByRole("heading", { name: "2 albums" })).toBeInTheDocument();
    expect(screen.getByText("20 MP3s")).toBeInTheDocument();
    expect(screen.getByText("20 of 20 selected across 2 albums")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Genre" }), { target: { value: "Soundtrack" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 20 MP3s" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
    expect(apply).toHaveBeenNthCalledWith(1, expect.objectContaining({
      albumPath: first.path,
      fields: ["genre"],
      tracks: expect.arrayContaining([expect.objectContaining({ path: expect.stringContaining("Baltimoore - Freak") })]),
    }));
    expect(apply.mock.calls[0]?.[0].tracks).toHaveLength(10);
    expect(apply).toHaveBeenNthCalledWith(2, expect.objectContaining({
      albumPath: secondPath,
      fields: ["genre"],
      tracks: expect.arrayContaining([expect.objectContaining({ path: expect.stringContaining("Neon Nights") })]),
    }));
    expect(apply.mock.calls[1]?.[0].tracks).toHaveLength(10);
  });

  it("keeps staged albums outside the library and opens Auto-Tagger from Ctrl+Shift+T", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByText("1 selected · 1 album outside the library")).toBeInTheDocument();
    expect(screen.getByText("1 issue")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Freak cover" })).toHaveAttribute(
      "src",
      "/__aurora-preview-cover/preview-freak?size=128",
    );
    await waitFor(() => expect(screen.getByRole("button", { name: /Auto-tag.*Ctrl Shift T/ })).toBeEnabled());

    fireEvent.keyDown(window, { key: "t", ctrlKey: true, shiftKey: true });

    expect(await screen.findByRole("dialog", { name: "Album Auto-Tagger" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByText("MusicBrainz").length).toBeGreaterThan(0));
    const genre = screen.getByRole("combobox", { name: "Genre" });
    expect(genre).toHaveAttribute("list", "inbox-auto-tagger-genre-suggestions");
    await waitFor(() => expect(document.querySelector(
      '#inbox-auto-tagger-genre-suggestions option[value="Electronic"]',
    )).toBeInTheDocument());
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
    fireEvent.change(screen.getByRole("combobox", { name: "Genre" }), { target: { value: "Hard Rock" } });
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
    let previewSequence = 0;
    const preview = vi.spyOn(libraryIntakeAdapter, "preview").mockImplementation(async ({ sourcePath, category }) => {
      previewSequence += 1;
      return libraryPreview(`folder-plan-${previewSequence}`, 40 + previewSequence, sourcePath, category, 1);
    });
    const apply = vi.spyOn(libraryIntakeAdapter, "apply").mockResolvedValue({
      planId: "folder-plan-2", sessionId: 42, status: "completed", albumCount: 1, trackCount: 10,
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

    await waitFor(() => expect(preview).toHaveBeenCalledTimes(2));
    expect(apply).toHaveBeenCalledWith({ planId: "folder-plan-2", sessionId: 42 });
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
    let previewSequence = 0;
    const preview = vi.spyOn(libraryIntakeAdapter, "preview").mockImplementation(async ({ sourcePath, category }) => {
      previewSequence += 1;
      return libraryPreview(`all-plan-${previewSequence}`, 50 + previewSequence, sourcePath, category, 1);
    });
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
    expect(preview).toHaveBeenCalledTimes(4);
    expect(apply).toHaveBeenNthCalledWith(1, { planId: "all-plan-3", sessionId: 53 });
    expect(apply).toHaveBeenNthCalledWith(2, { planId: "all-plan-4", sessionId: 54 });
    expect(await screen.findByText("2 albums moved, covers archived, and library catalog updated.")).toBeInTheDocument();
  });

  it("stops when a fresh apply-time preview no longer matches the reviewed intake", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({
      ...snapshot,
      albums: snapshot.albums.map((album) => ({ ...album, genre: "Hard Rock", readiness: { ready: true, issues: [] } })),
    });
    let previewSequence = 0;
    vi.spyOn(libraryIntakeAdapter, "preview").mockImplementation(async ({ sourcePath, category }) => {
      previewSequence += 1;
      const preview = libraryPreview(`changed-plan-${previewSequence}`, 60 + previewSequence, sourcePath, category, 1);
      return previewSequence === 1 ? preview : { ...preview, trackCount: 11 };
    });
    const apply = vi.spyOn(libraryIntakeAdapter, "apply");
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("button", { name: "Add Inbox to library" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Inbox" }), { target: { value: "general" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview destinations" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add 1 album" }));

    expect(await screen.findByText("Inbox changed after review. Preview destinations again before adding it to the library.")).toBeInTheDocument();
    expect(apply).not.toHaveBeenCalled();
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
