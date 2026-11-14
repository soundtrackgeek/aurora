import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "../library";
import type { EditableTagValues, TagEditorSnapshot, TagEditorTarget } from "../tags";
import { TagEditor } from "./TagEditor";

const tagMocks = vi.hoisted(() => ({
  read: vi.fn(),
  update: vi.fn(),
}));

const renameMocks = vi.hoisted(() => ({ rename: vi.fn() }));

vi.mock("../libraryAutoTag", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../libraryAutoTag")>();
  return { ...actual, renameLibraryAlbum: renameMocks.rename };
});

vi.mock("../tags", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../tags")>();
  return {
    ...actual,
    readTagEditorState: tagMocks.read,
    updateTagEditor: tagMocks.update,
  };
});

const target: TagEditorTarget = { kind: "album", albumId: "album-1", label: "America Town" };

function values(overrides: Partial<EditableTagValues> = {}): EditableTagValues {
  return {
    albumArtist: "Five for Fighting",
    artist: "Five for Fighting",
    album: "America Town",
    title: "Superman",
    genre: "Pop Rock",
    publisher: "Aware Records",
    rating: 4.5,
    year: 2000,
    releaseYear: 2000,
    trackNumber: 1,
    trackTotal: 12,
    discNumber: 1,
    discTotal: 1,
    ...overrides,
  };
}

function snapshot(): TagEditorSnapshot {
  return {
    tracks: [
      { trackId: "track-1", trackKey: "c:/music/01.mp3", revision: "revision-1", values: values() },
      { trackId: "track-2", trackKey: "c:/music/02.mp3", revision: "revision-2", values: values({ title: "Easy Tonight", trackNumber: 2 }) },
    ],
  };
}

function updatedTracks(): Track[] {
  return snapshot().tracks.map((state) => ({
    id: state.trackId,
    trackKey: state.trackKey,
    albumId: "album-1",
    title: state.values.title ?? "",
    artist: state.values.albumArtist ?? "",
    displayArtist: state.values.artist ?? undefined,
    album: state.values.album ?? "",
    originalYear: state.values.year,
    releaseYear: state.values.releaseYear,
    publisher: state.values.publisher,
    rating: state.values.rating,
    loved: false,
    loveState: "neutral",
    tagSyncState: "pendingImport",
    canUndoTagEdit: false,
    durationSeconds: 180,
    genre: state.values.genre,
    playCount: 0,
  }));
}

beforeEach(() => {
  tagMocks.read.mockReset().mockResolvedValue(snapshot());
  tagMocks.update.mockReset().mockResolvedValue({ state: snapshot(), tracks: updatedTracks() });
  renameMocks.rename.mockReset().mockResolvedValue({
    albumId: "album-1", sourcePath: "", destinationPath: "", folderRenamed: false, renamedFiles: 0, warnings: [],
  });
  try { window.localStorage.clear(); } catch { /* storage may be unavailable */ }
});

afterEach(cleanup);

describe("TagEditor", () => {
  it("projects freshly read file tags once without rereading when the parent updates", async () => {
    const liveTracks = updatedTracks().map((track) => ({ ...track, genre: "Southern Soul" }));
    const catalogSync = { status: "pending" as const, pendingFolderCount: 1, projectionToken: 4 };
    tagMocks.read.mockResolvedValue({ ...snapshot(), liveTracks, catalogSync });
    const onTracksChange = vi.fn();
    const onCatalogSync = vi.fn();
    const view = render(<TagEditor target={target} onTracksChange={onTracksChange} onCatalogSync={onCatalogSync} />);
    await waitFor(() => expect(onTracksChange).toHaveBeenCalledWith(liveTracks, catalogSync));
    await waitFor(() => expect(onCatalogSync).toHaveBeenCalledWith(catalogSync));
    view.rerender(<TagEditor target={target} onTracksChange={vi.fn()} onCatalogSync={onCatalogSync} />);
    expect(onCatalogSync).toHaveBeenCalledTimes(1);
    expect(tagMocks.read).toHaveBeenCalledTimes(1);
    expect(onTracksChange).toHaveBeenCalledTimes(1);
    expect(tagMocks.update).not.toHaveBeenCalled();
  });

  it("does not announce a stale file-read projection as pending sync", async () => {
    const catalogSync = { status: "pending" as const, pendingFolderCount: 1, projectionToken: 4 };
    tagMocks.read.mockResolvedValue({ ...snapshot(), liveTracks: updatedTracks(), catalogSync });
    const onCatalogSync = vi.fn();
    const onTracksChange = vi.fn().mockReturnValue(false);
    render(<TagEditor target={target} onTracksChange={onTracksChange} onCatalogSync={onCatalogSync} />);
    await screen.findByDisplayValue("America Town");
    expect(onTracksChange).toHaveBeenCalledTimes(1);
    expect(onCatalogSync).not.toHaveBeenCalled();
  });

  it("shows common album values, mixed track values, and the album MP3 count", async () => {
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);

    expect(await screen.findByDisplayValue("America Town")).toBeInTheDocument();
    const genre = screen.getByLabelText("Genre");
    expect(genre).toHaveAttribute("list", "tag-editor-genre-suggestions");
    await waitFor(() => expect(document.querySelector(
      '#tag-editor-genre-suggestions option[value="Electronic"]',
    )).toBeInTheDocument());
    expect(screen.getByLabelText("Track title")).toHaveAttribute("placeholder", "Mixed");
    expect(screen.getByLabelText("Write Track title")).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Save 0 fields to 2 MP3s" })).toBeDisabled();
    expect(screen.getByText("2 MP3s")).toBeInTheDocument();
  });

  it("omits an untouched mixed title and sends the full revision snapshot", async () => {
    const onTracksChange = vi.fn();
    render(<TagEditor target={target} onTracksChange={onTracksChange} />);
    const album = await screen.findByLabelText("Album");

    fireEvent.change(album, { target: { value: "America Town Deluxe" } });
    expect(screen.getByLabelText("Write Album")).toBeChecked();
    expect(screen.getByLabelText("Write Track title")).not.toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    await waitFor(() => expect(tagMocks.update).toHaveBeenCalledTimes(1));
    const [sentTarget, expected, fields, sentValues] = tagMocks.update.mock.calls[0];
    expect(sentTarget).toEqual(target);
    expect(expected).toEqual(snapshot());
    expect(fields).toEqual(["album"]);
    expect(sentValues).toMatchObject({ album: "America Town Deluxe", title: null });
    expect(onTracksChange).toHaveBeenCalledWith(updatedTracks());
  });

  it("stages a replacement cover as an album-wide save change", async () => {
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Choose replacement album cover" }));

    expect(await screen.findByText("selected-cover.jpg")).toBeInTheDocument();
    expect(screen.getByText("Pending · saves to all 2 album MP3s")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save 1 change to 2 MP3s" }));

    await waitFor(() => expect(tagMocks.update).toHaveBeenCalledTimes(1));
    expect(tagMocks.update).toHaveBeenCalledWith(
      target,
      snapshot(),
      [],
      expect.any(Object),
      "preview-selected-cover",
    );
    expect(await screen.findByText(
      "Embedded the replacement cover in 2 MP3s.",
    )).toBeInTheDocument();
  });

  it("treats a checked blank value as an explicit clear", async () => {
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);
    const genre = await screen.findByLabelText("Genre");

    fireEvent.change(genre, { target: { value: "" } });
    expect(screen.getByLabelText("Write Genre")).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    await waitFor(() => expect(tagMocks.update).toHaveBeenCalledTimes(1));
    expect(tagMocks.update.mock.calls[0][2]).toEqual(["genre"]);
    expect(tagMocks.update.mock.calls[0][3].genre).toBeNull();
  });

  it("notifies App and names Music Library after a successful catalog sync", async () => {
    const onCatalogSync = vi.fn().mockResolvedValue(undefined);
    tagMocks.update.mockResolvedValue({
      state: snapshot(),
      tracks: updatedTracks(),
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
    });
    render(
      <TagEditor
        target={target}
        onTracksChange={vi.fn()}
        onCatalogSync={onCatalogSync}
      />,
    );

    fireEvent.change(await screen.findByLabelText("Genre"), { target: { value: "Pop" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    expect(await screen.findByText(/Saved 1 field directly to 2 MP3s\. Music Library updated\./)).toBeInTheDocument();
    await waitFor(() => expect(onCatalogSync).toHaveBeenCalledWith({
      status: "synced",
      message: "Music Library updated.",
      pendingFolderCount: 0,
    }));
  });

  it("keeps a verified MP3 save candid when Music Library sync is pending", async () => {
    const onCatalogSync = vi.fn().mockResolvedValue(undefined);
    tagMocks.update.mockResolvedValue({
      state: snapshot(),
      tracks: updatedTracks(),
      catalogSync: {
        status: "pending",
        message: "Music Library update pending; Aurora will retry automatically.",
        pendingFolderCount: 1,
      },
    });
    render(
      <TagEditor
        target={target}
        onTracksChange={vi.fn()}
        onCatalogSync={onCatalogSync}
      />,
    );

    fireEvent.change(await screen.findByLabelText("Genre"), { target: { value: "Pop" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    expect(await screen.findByText(
      "Saved 1 field directly to 2 MP3s. Music Library update pending; Aurora will retry automatically.",
    )).toBeInTheDocument();
    await waitFor(() => expect(onCatalogSync).toHaveBeenCalledWith(expect.objectContaining({
      status: "pending",
      pendingFolderCount: 1,
    })));
  });

  it("reports when Music Library retries are paused without obscuring the MP3 save", async () => {
    tagMocks.update.mockResolvedValue({
      state: snapshot(),
      tracks: updatedTracks(),
      catalogSync: {
        status: "blocked",
        message: "Music Library update needs attention; automatic retries are paused.",
        pendingFolderCount: 1,
        blockedFolderCount: 1,
      },
    });
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);

    fireEvent.change(await screen.findByLabelText("Genre"), { target: { value: "Pop" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    expect(await screen.findByText(
      "Saved 1 field directly to 2 MP3s. Music Library update needs attention; automatic retries are paused.",
    )).toBeInTheDocument();
  });

  it("reloads authoritative tags instead of projecting a stale edit result", async () => {
    const authoritative = snapshot();
    authoritative.tracks = authoritative.tracks.map((track) => ({
      ...track,
      values: { ...track.values, album: "America Town Newer" },
    }));
    tagMocks.read
      .mockResolvedValueOnce(snapshot())
      .mockResolvedValueOnce(authoritative);
    tagMocks.update.mockResolvedValue({
      state: snapshot(),
      tracks: updatedTracks(),
      catalogSync: {
        status: "synced",
        message: "Music Library updated.",
        pendingFolderCount: 0,
        projectionToken: 1,
      },
    });
    const onTracksChange = vi.fn().mockReturnValue(false);
    const onCatalogSync = vi.fn();
    render(
      <TagEditor
        target={target}
        onTracksChange={onTracksChange}
        onCatalogSync={onCatalogSync}
      />,
    );

    fireEvent.change(await screen.findByLabelText("Genre"), { target: { value: "Pop" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    expect(await screen.findByDisplayValue("America Town Newer")).toBeInTheDocument();
    expect(tagMocks.read).toHaveBeenCalledTimes(2);
    expect(onCatalogSync).toHaveBeenCalledWith({
      status: "synced",
      message: "Music Library updated.",
      pendingFolderCount: 0,
      projectionToken: 1,
    });
  });

  it("refuses to clear Music Library identity fields", async () => {
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);
    const album = await screen.findByLabelText("Album");

    fireEvent.change(album, { target: { value: "" } });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Music Library requires this field; enter a value or leave it unchecked.",
    );
    expect(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" })).toBeDisabled();
    expect(tagMocks.update).not.toHaveBeenCalled();
  });

  it("refreshes on focus only while the draft is clean", async () => {
    await act(async () => {
      render(<TagEditor target={target} onTracksChange={vi.fn()} />);
    });
    const album = await screen.findByLabelText("Album");
    expect(tagMocks.read).toHaveBeenCalledTimes(1);

    fireEvent.focus(window);
    await waitFor(() => expect(tagMocks.read).toHaveBeenCalledTimes(2));

    fireEvent.change(album, { target: { value: "Unsaved album name" } });
    fireEvent.focus(window);
    expect(tagMocks.read).toHaveBeenCalledTimes(2);
  });
});

describe("TagEditor auto rename folder", () => {
  const renamed = {
    albumId: "album-1",
    sourcePath: "D:\\Music\\Five for Fighting - America Town (2000)",
    destinationPath: "D:\\Music\\Five for Fighting - America Town (1999)",
    folderRenamed: true,
    renamedFiles: 2,
    warnings: [],
  };

  it("renames the album folder after a year change, then rereads the moved files", async () => {
    renameMocks.rename.mockResolvedValue(renamed);
    const onAlbumRenamed = vi.fn().mockResolvedValue(undefined);
    render(<TagEditor target={target} onTracksChange={vi.fn()} onAlbumRenamed={onAlbumRenamed} />);

    expect(await screen.findByRole("checkbox", { name: /Auto rename folder/ })).toBeChecked();
    fireEvent.change(screen.getByLabelText("Year"), { target: { value: "1999" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    expect(await screen.findByText(/Renamed the folder to Five for Fighting - America Town \(1999\) and 2 tracks\./)).toBeInTheDocument();
    expect(renameMocks.rename).toHaveBeenCalledWith("album-1");
    expect(onAlbumRenamed).toHaveBeenCalledWith(renamed);
    expect(tagMocks.read).toHaveBeenCalledTimes(2);
  });

  it("leaves the folder alone for fields that are not in its name", async () => {
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);

    fireEvent.change(await screen.findByLabelText("Release year"), { target: { value: "2001" } });
    fireEvent.change(screen.getByLabelText("Genre"), { target: { value: "Pop" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 2 fields to 2 MP3s" }));

    await waitFor(() => expect(tagMocks.update).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(/Saved 2 fields/)).toBeInTheDocument();
    expect(renameMocks.rename).not.toHaveBeenCalled();
  });

  it("remembers when the user turns it off", async () => {
    const first = render(<TagEditor target={target} onTracksChange={vi.fn()} />);
    fireEvent.click(await screen.findByRole("checkbox", { name: /Auto rename folder/ }));
    fireEvent.change(screen.getByLabelText("Album"), { target: { value: "America Town (Remastered)" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));
    await waitFor(() => expect(tagMocks.update).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(/Saved 1 field/)).toBeInTheDocument();
    expect(renameMocks.rename).not.toHaveBeenCalled();
    first.unmount();

    render(<TagEditor target={target} onTracksChange={vi.fn()} />);
    expect(await screen.findByRole("checkbox", { name: /Auto rename folder/ })).not.toBeChecked();
  });

  it("keeps the saved tags and explains a rename Music Library refused", async () => {
    renameMocks.rename.mockRejectedValue(new Error("Music Library 0.217.0 or later is required."));
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);

    fireEvent.change(await screen.findByLabelText("Album artist"), { target: { value: "John Ondrasik" } });
    fireEvent.click(screen.getByRole("button", { name: "Save 1 field to 2 MP3s" }));

    expect(await screen.findByText(/The folder was not renamed: Music Library 0\.217\.0 or later is required\./)).toBeInTheDocument();
  });

  it("is only offered for one album", async () => {
    render(<TagEditor target={{ kind: "track", trackId: "track-1", trackKey: "c:/music/01.mp3", label: "Superman" }} onTracksChange={vi.fn()} />);
    await screen.findByLabelText("Genre");
    expect(screen.queryByRole("checkbox", { name: /Auto rename folder/ })).not.toBeInTheDocument();
  });
});
