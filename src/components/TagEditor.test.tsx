import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "../library";
import type { EditableTagValues, TagEditorSnapshot, TagEditorTarget } from "../tags";
import { TagEditor } from "./TagEditor";

const tagMocks = vi.hoisted(() => ({
  read: vi.fn(),
  update: vi.fn(),
}));

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
});

afterEach(cleanup);

describe("TagEditor", () => {
  it("shows common album values, mixed track values, and the album MP3 count", async () => {
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);

    expect(await screen.findByDisplayValue("America Town")).toBeInTheDocument();
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
    render(<TagEditor target={target} onTracksChange={vi.fn()} />);
    const album = await screen.findByLabelText("Album");
    expect(tagMocks.read).toHaveBeenCalledTimes(1);

    window.dispatchEvent(new Event("focus"));
    await waitFor(() => expect(tagMocks.read).toHaveBeenCalledTimes(2));

    fireEvent.change(album, { target: { value: "Unsaved album name" } });
    window.dispatchEvent(new Event("focus"));
    expect(tagMocks.read).toHaveBeenCalledTimes(2);
  });
});
