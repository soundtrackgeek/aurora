import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as artworkSelection from "../../artworkSelection";
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
    expect(screen.getByText("3 issues")).toBeInTheDocument();
    expect(screen.getByText("320 kbps average")).toBeInTheDocument();
    expect(screen.getByText(/^MP3$/)).toBeInTheDocument();
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

  it("converts a lossless Inbox album before enabling MP3 preparation", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    const original = snapshot.albums[0];
    const lossless = {
      ...original,
      artist: null,
      album: null,
      formats: ["FLAC", "APE"],
      losslessTrackCount: 2,
      artworkPresent: false,
      artworkSourcePath: null,
      artworkTrackCount: 0,
      artworkReady: false,
      readiness: { ready: false, issues: ["Convert 2 FLAC/APE tracks to 320 kbps MP3"] },
      tracks: original.tracks.slice(0, 2).map((track, index) => ({
        ...track,
        path: track.path.replace(/\.mp3$/u, index === 0 ? ".flac" : ".ape"),
        fileName: track.fileName.replace(/\.mp3$/u, index === 0 ? ".flac" : ".ape"),
        format: index === 0 ? "FLAC" : "APE",
      })),
      trackCount: 2,
    };
    vi.spyOn(inboxAdapter, "loadInboxSnapshot")
      .mockResolvedValueOnce({ ...snapshot, albums: [lossless] })
      .mockResolvedValue({ ...snapshot, albums: [original] });
    const convert = vi.spyOn(inboxAdapter, "convertInboxLossless").mockResolvedValue({
      convertedTracks: 2,
      deletedSources: 2,
      failures: [],
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    expect(await screen.findByText("FLAC · APE")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Tags" })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Auto-tag/ })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /Convert to 320 kbps MP3/ }));

    await waitFor(() => expect(convert).toHaveBeenCalledWith(lossless.path));
    expect(await screen.findByText("2 tracks converted to 320 kbps MP3; 2 source files deleted.")).toBeInTheDocument();
  });

  it("repairs partial embedded artwork before intake", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    const album = {
      ...snapshot.albums[0],
      artworkTrackCount: 1,
      artworkReady: false,
      readiness: {
        ready: false,
        issues: ["Embedded front cover is missing or invalid on 9 tracks"],
      },
    };
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({ ...snapshot, albums: [album] });
    const choose = vi.spyOn(inboxAdapter, "selectInboxCoverImage");
    const embed = vi.spyOn(inboxAdapter, "embedInboxAlbumCover").mockResolvedValue({
      changedTracks: 9,
      trackCount: 10,
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    expect(await screen.findByText("1 / 10 embedded")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Embed cover in all tracks/ }));

    await waitFor(() => expect(embed).toHaveBeenCalledWith(album.path, null));
    expect(choose).not.toHaveBeenCalled();
    expect(await screen.findByText("Embedded the album cover in 10 tracks; 9 needed updating.")).toBeInTheDocument();
  });

  it("asks for an image when an album has no usable embedded cover", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    const album = {
      ...snapshot.albums[0],
      artworkPresent: false,
      artworkSourcePath: null,
      artworkTrackCount: 0,
      artworkReady: false,
      readiness: { ready: false, issues: ["Embedded front cover is missing"] },
    };
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({ ...snapshot, albums: [album] });
    vi.spyOn(inboxAdapter, "selectInboxCoverImage").mockResolvedValue("C:\\Pictures\\cover.png");
    const embed = vi.spyOn(inboxAdapter, "embedInboxAlbumCover").mockResolvedValue({
      changedTracks: 10,
      trackCount: 10,
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: /Choose album cover/ }));

    await waitFor(() => expect(embed).toHaveBeenCalledWith(album.path, "C:\\Pictures\\cover.png"));
  });

  it("renames a manually tagged album from Ctrl+R", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    await waitFor(() => expect(screen.getByRole("button", { name: /^Rename\s+Ctrl R$/u })).toBeEnabled());
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });

    expect(await screen.findByText("10 tracks renamed with the album folder.")).toBeInTheDocument();
  });

  it("renames every selected Inbox album from Ctrl+R", async () => {
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
        path: track.path.replace(first.path, secondPath),
      })),
    };
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({ ...snapshot, albums: [first, second] });
    const rename = vi.spyOn(inboxAdapter, "renameInboxAlbums").mockResolvedValue({
      renamedTracks: 20,
      renamedAlbums: 2,
      renamedFolders: 1,
      failures: [],
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    const firstRow = await screen.findByRole("row", { name: /Freak by/ });
    fireEvent.click(firstRow);
    fireEvent.click(screen.getByRole("row", { name: /Neon Nights by/ }), { shiftKey: true });
    expect(screen.getByRole("button", { name: /Rename 2 albums.*Ctrl R/ })).toBeEnabled();
    expect(screen.getByText("Standardize 2 selected album folders and track filenames")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "r", ctrlKey: true });

    await waitFor(() => expect(rename).toHaveBeenCalledTimes(1));
    expect(rename).toHaveBeenCalledWith([first.path, secondPath]);
    expect(await screen.findByText("20 tracks renamed across 2 albums with 1 album folder renamed.")).toBeInTheDocument();
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

  it("saves a chosen Inbox cover album-wide even when one tag track is selected", async () => {
    vi.spyOn(artworkSelection, "selectAlbumCoverImage").mockResolvedValue({
      token: "selected-inbox-cover",
      previewUrl: "/__aurora-preview-cover/preview-freak?size=256",
      fileName: "better-cover.jpg",
    });
    const apply = vi.spyOn(inboxAdapter, "applyInboxTags").mockResolvedValue({
      changedTracks: 10,
      renamedTracks: 0,
      albumPath: "C:\\Music\\Inbox\\Baltimoore - Freak",
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("tab", { name: "Tags" }));
    fireEvent.click(screen.getByRole("button", { name: "None" }));
    fireEvent.click(screen.getByRole("checkbox", { name: /01.*Memories Calling/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Choose replacement album cover" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save 1 change to 10 MP3s" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(apply).toHaveBeenCalledWith(expect.objectContaining({
      fields: [],
      artworkToken: "selected-inbox-cover",
      tracks: [expect.objectContaining({ path: expect.stringContaining("Memories Calling") })],
    }));
    expect(await screen.findByText(
      "Embedded the replacement cover in all 10 album MP3s.",
    )).toBeInTheDocument();
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

  it("prefers an original edition and moves a confidently unmatched bonus track only after review", async () => {
    const searchResult = await inboxAdapter.searchInboxReleases("Baltimoore", "Freak", 10, true);
    const candidate = { ...searchResult.candidates[0], year: 2008, originalYear: 1990, trackCount: 9 };
    const fullDetail = await inboxAdapter.loadInboxReleaseDetail(candidate);
    vi.spyOn(inboxAdapter, "searchInboxReleases").mockResolvedValue({
      ...searchResult,
      candidates: [candidate],
    });
    vi.spyOn(inboxAdapter, "loadInboxReleaseDetail").mockResolvedValue({
      ...fullDetail,
      candidate,
      year: 2008,
      tracks: fullDetail.tracks.slice(0, 9).map((track) => ({ ...track, trackTotal: 9 })),
    });
    const apply = vi.spyOn(inboxAdapter, "applyInboxTags").mockResolvedValue({
      changedTracks: 9,
      renamedTracks: 9,
      removedTracks: 1,
      recoveryPath: "C:\\Aurora\\inbox-recovery\\reviewed",
      albumPath: "C:\\Music\\Inbox\\Baltimoore - Freak (1990)",
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    const autoTag = await waitFor(() => {
      const button = screen.getByRole("button", { name: /Auto-tag.*Ctrl Shift T/u });
      expect(button).toBeEnabled();
      return button;
    });
    fireEvent.click(autoTag);

    expect(await screen.findByRole("checkbox", { name: /Prefer the original edition/u })).toBeChecked();
    expect(await screen.findByText("9 of 9 release tracks matched")).toBeInTheDocument();
    expect(screen.getByText(/1 extra local track/u)).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Original year" })).toHaveValue("1990");
    expect(screen.getByRole("textbox", { name: "Release year" })).toHaveValue("2008");

    fireEvent.click(screen.getByRole("checkbox", { name: /Remove unmatched.*Shadows/u }));
    fireEvent.click(screen.getByRole("button", { name: "Apply, rename & move 1 extra" }));
    const confirmation = screen.getByRole("alertdialog", { name: "Move 1 unmatched track out of this album?" });
    expect(confirmation).toHaveTextContent("Shadows");
    expect(confirmation.parentElement).toHaveClass("inbox-extra-confirmation-backdrop");
    fireEvent.keyDown(confirmation, { key: "Escape" });
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Album Auto-Tagger" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apply, rename & move 1 extra" }));
    fireEvent.click(screen.getByRole("button", { name: "Move to recovery" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(apply).toHaveBeenCalledWith(expect.objectContaining({
      renameAfterApply: true,
      removeTrackPaths: [expect.stringContaining("Shadows")],
      tracks: expect.arrayContaining([expect.objectContaining({
        values: expect.objectContaining({ year: 1990, releaseYear: 2008, trackTotal: 9 }),
      })]),
    }));
    expect(apply.mock.calls[0]?.[0].tracks).toHaveLength(9);
    expect(await screen.findByText(/moved 1 unmatched track to Aurora recovery/u)).toBeInTheDocument();
  });

  it("lets the user confirm that a misspelled release title belongs to an unmatched local file", async () => {
    const searchResult = await inboxAdapter.searchInboxReleases("Baltimoore", "Freak", 10, true);
    const candidate = searchResult.candidates[0];
    const fullDetail = await inboxAdapter.loadInboxReleaseDetail(candidate);
    vi.spyOn(inboxAdapter, "searchInboxReleases").mockResolvedValue({ ...searchResult, candidates: [candidate] });
    vi.spyOn(inboxAdapter, "loadInboxReleaseDetail").mockResolvedValue({
      ...fullDetail,
      tracks: fullDetail.tracks.map((track, index) => index === 1 ? { ...track, title: "Kahlua Conflusion" } : track),
    });
    const apply = vi.spyOn(inboxAdapter, "applyInboxTags").mockResolvedValue({
      changedTracks: 10,
      renamedTracks: 10,
      removedTracks: 0,
      recoveryPath: null,
      albumPath: "C:\\Music\\Inbox\\Baltimoore - Freak (1990)",
    });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    const autoTag = await waitFor(() => {
      const button = screen.getByRole("button", { name: /Auto-tag.*Ctrl Shift T/u });
      expect(button).toBeEnabled();
      return button;
    });
    fireEvent.click(autoTag);

    expect(await screen.findByText("9 of 10 release tracks matched")).toBeInTheDocument();
    expect(screen.getByText(/1 extra local track/u)).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Local file for Kahlua Conflusion" })).toHaveValue("1");
    fireEvent.click(screen.getByRole("button", { name: "Confirm match" }));

    expect(screen.getByText("10 of 10 release tracks matched")).toBeInTheDocument();
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
    expect(screen.getByLabelText("Release title 2")).toHaveValue("Kahlua Conflusion");
    fireEvent.click(screen.getByRole("button", { name: "Apply & rename" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(apply.mock.calls[0]?.[0].tracks).toHaveLength(10);
    expect(apply).toHaveBeenCalledWith(expect.objectContaining({
      tracks: expect.arrayContaining([expect.objectContaining({
        path: expect.stringContaining("Kahlua Confusion"),
        values: expect.objectContaining({ title: "Kahlua Conflusion", trackNumber: 2 }),
      })]),
    }));
  });

  it("keeps the selected release stable while the Inbox refreshes", async () => {
    const search = vi.spyOn(inboxAdapter, "searchInboxReleases");
    const loadSnapshot = vi.spyOn(inboxAdapter, "loadInboxSnapshot");
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    const autoTag = await waitFor(() => {
      const button = screen.getByRole("button", { name: /Auto-tag.*Ctrl Shift T/u });
      expect(button).toBeEnabled();
      return button;
    });
    fireEvent.click(autoTag);
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
      albums: [{ sourcePath: "C:\\Music\\Inbox\\Baltimoore - Freak", destinationPath: "D:\\Music\\Baltimoore - Freak", action: "add", recoveryPath: null, cleanupStatus: "removed" }],
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

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(preview).toHaveBeenCalledTimes(1);
    expect(apply).toHaveBeenCalledWith({ planId: "folder-plan-1", sessionId: 41 });
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
      albums: [{ sourcePath: "source", destinationPath: "destination", action: "add", recoveryPath: null, cleanupStatus: "removed" }],
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
    expect(preview).toHaveBeenCalledTimes(2);
    expect(apply).toHaveBeenNthCalledWith(1, { planId: "all-plan-1", sessionId: 51 });
    expect(apply).toHaveBeenNthCalledWith(2, { planId: "all-plan-2", sessionId: 52 });
    expect(await screen.findByText("2 albums moved, covers archived, and library catalog updated.")).toBeInTheDocument();
  });

  it("retries an unchanged Inbox intake when catalog synchronization makes the fresh plan stale", async () => {
    const snapshot = await inboxAdapter.loadInboxSnapshot();
    vi.spyOn(inboxAdapter, "loadInboxSnapshot").mockResolvedValue({
      ...snapshot,
      albums: snapshot.albums.map((album) => ({ ...album, genre: "Hard Rock", readiness: { ready: true, issues: [] } })),
    });
    let previewSequence = 0;
    const preview = vi.spyOn(libraryIntakeAdapter, "preview").mockImplementation(async ({ sourcePath, category }) => {
      previewSequence += 1;
      return libraryPreview(`retry-plan-${previewSequence}`, 70 + previewSequence, sourcePath, category, 1);
    });
    const apply = vi.spyOn(libraryIntakeAdapter, "apply")
      .mockRejectedValueOnce(new Error("The source albums or active catalog changed after preview. Prepare the batch again (stalePlan)"))
      .mockResolvedValue({
        planId: "retry-plan-2", sessionId: 72, status: "completed", albumCount: 1, trackCount: 10,
        movedAlbumCount: 1, importRunId: 7, backupPath: null, cleanupWarnings: [],
        albums: [{ sourcePath: "source", destinationPath: "destination", action: "add", recoveryPath: null, cleanupStatus: "removed" }],
      });
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("button", { name: "Add Inbox to library" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Inbox" }), { target: { value: "general" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview destinations" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add 1 album" }));

    await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
    expect(preview).toHaveBeenCalledTimes(2);
    expect(apply).toHaveBeenNthCalledWith(1, { planId: "retry-plan-1", sessionId: 71 });
    expect(apply).toHaveBeenNthCalledWith(2, { planId: "retry-plan-2", sessionId: 72 });
    expect(await screen.findByText("1 album moved, covers archived, and library catalog updated.")).toBeInTheDocument();
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
    const apply = vi.spyOn(libraryIntakeAdapter, "apply")
      .mockRejectedValueOnce(new Error("The source albums or active catalog changed after preview. Prepare the batch again (stalePlan)"));
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.click(screen.getByRole("button", { name: "Add Inbox to library" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination for Inbox" }), { target: { value: "general" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview destinations" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add 1 album" }));

    expect(await screen.findByText("Inbox changed after review. Preview destinations again before adding it to the library.")).toBeInTheDocument();
    expect(apply).toHaveBeenCalledTimes(1);
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
