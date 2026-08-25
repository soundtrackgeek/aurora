import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  libraryIntakeCategories,
  type LibraryBridgeCapabilities,
  type LibraryIntakeAdapter,
  type LibraryIntakeApplyResult,
  type LibraryIntakePreview,
} from "../ingest";
import { AddFolderDialog } from "./AddFolderDialog";

const capabilities: LibraryBridgeCapabilities = {
  bridgeVersion: 1,
  categories: [
    { id: "general", label: "General music", destinationRoot: "D:\\Music\\General", available: true },
    { id: "scores", label: "Movie / TV / game music", destinationRoot: "D:\\Music\\Scores", available: true },
    { id: "synthwave", label: "Synthwave", destinationRoot: "D:\\Music\\Synthwave", available: true },
  ],
  supports: {
    singleAlbum: true,
    batchFolders: true,
    crossVolumeCopy: true,
    previewRequired: true,
  },
};

const preview: LibraryIntakePreview = {
  planId: "plan-42",
  sessionId: 42,
  sourcePath: "C:\\Intake",
  category: {
    id: "scores",
    label: "Movie / TV / game music",
    destinationRoot: "D:\\Music\\Scores",
  },
  albumCount: 2,
  trackCount: 19,
  delta: {
    addedTracks: 19,
    changedTracks: 2,
    removedTracks: 0,
    addedAlbums: 2,
    changedAlbums: 1,
    removedAlbums: 0,
  },
  albums: [
    {
      sourcePath: "C:\\Intake\\John Williams - Example (2026)",
      destinationPath: "D:\\Music\\Scores\\John Williams - Example (2026)",
      artist: "John Williams",
      album: "Example",
      year: "2026",
      trackCount: 10,
    },
    {
      sourcePath: "C:\\Intake\\Bear McCreary - Another (2026)",
      destinationPath: "D:\\Music\\Scores\\Bear McCreary - Another (2026)",
      artist: "Bear McCreary",
      album: "Another",
      year: "2026",
      trackCount: 9,
    },
  ],
  canApply: true,
};

const applyResult: LibraryIntakeApplyResult = {
  planId: "plan-42",
  sessionId: 42,
  status: "completed",
  albumCount: 2,
  trackCount: 19,
  movedAlbumCount: 2,
  importRunId: 91,
  backupPath: "C:\\Backups\\catalog.sqlite3",
  albums: preview.albums.map((album) => ({
    sourcePath: album.sourcePath,
    destinationPath: album.destinationPath,
    cleanupStatus: "removed" as const,
  })),
  cleanupWarnings: [],
};

afterEach(cleanup);

function createAdapter(overrides: Partial<LibraryIntakeAdapter> = {}): LibraryIntakeAdapter {
  return {
    capabilities: vi.fn().mockResolvedValue(capabilities),
    selectFolder: vi.fn().mockResolvedValue("C:\\Intake"),
    preview: vi.fn().mockResolvedValue(preview),
    apply: vi.fn().mockResolvedValue(applyResult),
    ...overrides,
  };
}

function renderDialog(adapter = createAdapter(), overrides: Partial<Parameters<typeof AddFolderDialog>[0]> = {}) {
  const props: Parameters<typeof AddFolderDialog>[0] = {
    adapter,
    onClose: vi.fn(),
    onCatalogChanged: vi.fn(),
    ...overrides,
  };
  return { ...render(<AddFolderDialog {...props} />), adapter, props };
}

async function chooseScoresAndPreview() {
  await screen.findByText("Music Library companion ready");
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  await screen.findByText("C:\\Intake");
  fireEvent.click(screen.getByRole("radio", { name: /Movie \/ TV \/ game music/ }));
  fireEvent.click(screen.getByRole("button", { name: "Preview batch" }));
  await screen.findByRole("heading", { name: "2 albums · 19 tracks" });
}

describe("AddFolderDialog", () => {
  it("uses stable category IDs and shows every resolved destination root", async () => {
    renderDialog();

    await screen.findByText("Music Library companion ready");
    await waitFor(() => expect(screen.getByRole("button", { name: "Close add music" })).toHaveFocus());
    expect(libraryIntakeCategories.map((category) => category.id)).toEqual(["general", "scores", "synthwave"]);
    expect(screen.getByRole("radio", { name: /General music/ })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: /Movie \/ TV \/ game music/ })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: /Synthwave/ })).not.toBeChecked();
    expect(screen.getByText("D:\\Music\\General")).toBeInTheDocument();
    expect(screen.getByText("D:\\Music\\Scores")).toBeInTheDocument();
    expect(screen.getByText("D:\\Music\\Synthwave")).toBeInTheDocument();
    expect(screen.getByText(/keeps your tags, filenames, and album folder names unchanged/i)).toBeInTheDocument();
  });

  it("invalidates a stale preview when its category or source changes", async () => {
    const selectFolder = vi.fn()
      .mockResolvedValueOnce("C:\\Intake")
      .mockResolvedValueOnce("C:\\Friday scores");
    const adapter = createAdapter({ selectFolder });
    renderDialog(adapter);
    await chooseScoresAndPreview();

    expect(adapter.preview).toHaveBeenCalledWith({ sourcePath: "C:\\Intake", category: "scores" });
    fireEvent.click(screen.getByRole("radio", { name: /Synthwave/ }));
    expect(screen.queryByRole("heading", { name: "2 albums · 19 tracks" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("radio", { name: /Movie \/ TV \/ game music/ }));
    fireEvent.click(screen.getByRole("button", { name: "Preview batch" }));
    await screen.findByRole("heading", { name: "2 albums · 19 tracks" });
    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    await screen.findByText("C:\\Friday scores");
    expect(screen.queryByRole("heading", { name: "2 albums · 19 tracks" })).not.toBeInTheDocument();
  });

  it("invalidates a stale preview when the same source folder is chosen again", async () => {
    const adapter = createAdapter();
    renderDialog(adapter);
    await chooseScoresAndPreview();

    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    await waitFor(() => expect(adapter.selectFolder).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("heading", { name: "2 albums · 19 tracks" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preview batch" })).toBeEnabled();
  });

  it("can replace a stale plan without closing while preserving explicit apply retry", async () => {
    const refreshedPreview = { ...preview, planId: "plan-2" };
    const previewBatch = vi.fn()
      .mockResolvedValueOnce(preview)
      .mockResolvedValueOnce(refreshedPreview);
    const apply = vi.fn().mockRejectedValue(new Error("The source changed after preview (stalePlan)"));
    const adapter = createAdapter({ preview: previewBatch, apply });
    renderDialog(adapter);
    await chooseScoresAndPreview();

    fireEvent.click(screen.getByRole("button", { name: "Review apply" }));
    fireEvent.click(screen.getByRole("button", { name: "Move and catalog 2" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("stalePlan");
    await waitFor(() => expect(screen.getByRole("button", { name: "Move and catalog 2" })).toBeEnabled());

    fireEvent.click(screen.getByRole("button", { name: "Preview again" }));
    await waitFor(() => expect(previewBatch).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("heading", { name: "2 albums · 19 tracks" })).toBeInTheDocument();
  });

  it("renders every source-to-destination mapping and the exact catalog delta", async () => {
    renderDialog();
    await chooseScoresAndPreview();

    expect(screen.getByText("John Williams · 2026 · 10 tracks")).toBeInTheDocument();
    expect(screen.getByText("C:\\Intake\\John Williams - Example (2026)")).toBeInTheDocument();
    expect(screen.getByText("D:\\Music\\Scores\\John Williams - Example (2026)")).toBeInTheDocument();
    expect(screen.getByText("+19")).toBeInTheDocument();
    expect(screen.getAllByText("changed")).toHaveLength(2);
    expect(screen.getAllByText("removed")).toHaveLength(2);
  });

  it("makes a multi-album plan a keyboard-accessible scroll region", async () => {
    const scrollPreview: LibraryIntakePreview = {
      ...preview,
      albumCount: 3,
      trackCount: 20,
      albums: [
        ...preview.albums,
        {
          sourcePath: "C:\\Intake\\Third Artist - Last Album (2026)",
          destinationPath: "D:\\Music\\Scores\\Third Artist - Last Album (2026)",
          artist: "Third Artist",
          album: "Last Album",
          year: "2026",
          trackCount: 1,
        },
      ],
    };
    renderDialog(createAdapter({ preview: vi.fn().mockResolvedValue(scrollPreview) }));
    await screen.findByText("Music Library companion ready");
    fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
    await screen.findByText("C:\\Intake");
    fireEvent.click(screen.getByRole("radio", { name: /Movie \/ TV \/ game music/ }));
    fireEvent.click(screen.getByRole("button", { name: "Preview batch" }));
    await screen.findByRole("heading", { name: "3 albums · 20 tracks" });

    const albumMoves = screen.getByRole("list", { name: "Album moves" });
    expect(albumMoves).toHaveClass("is-scrollable");
    expect(albumMoves).toHaveAttribute("tabindex", "0");
  });

  it("hides confirmation while applying and prevents duplicate apply requests", async () => {
    let resolveApply: (value: LibraryIntakeApplyResult) => void = () => undefined;
    const pendingApply = new Promise<LibraryIntakeApplyResult>((resolve) => { resolveApply = resolve; });
    const apply = vi.fn().mockReturnValue(pendingApply);
    renderDialog(createAdapter({ apply }));
    await chooseScoresAndPreview();

    fireEvent.click(screen.getByRole("button", { name: "Review apply" }));
    const moveButton = screen.getByRole("button", { name: "Move and catalog 2" });
    fireEvent.click(moveButton);
    fireEvent.click(moveButton);

    expect(apply).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: "Move and catalog 2" })).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Copying and verifying albums");

    await act(async () => { resolveApply(applyResult); });
    await screen.findByText("2 of 2 albums fully moved · 2 cataloged · 19 tracks");
  });

  it("hands apply to the app background worker and closes immediately", async () => {
    const onApplyInBackground = vi.fn();
    const onClose = vi.fn();
    const adapter = createAdapter();
    renderDialog(adapter, { onApplyInBackground, onClose });
    await chooseScoresAndPreview();

    fireEvent.click(screen.getByRole("button", { name: "Review apply" }));
    fireEvent.click(screen.getByRole("button", { name: "Move and catalog 2" }));

    expect(onApplyInBackground).toHaveBeenCalledWith(
      { planId: "plan-42", sessionId: 42 },
      preview,
    );
    expect(adapter.apply).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("requires explicit confirmation, applies the locked plan, and refreshes Aurora", async () => {
    const onCatalogChanged = vi.fn().mockResolvedValue(true);
    const adapter = createAdapter();
    renderDialog(adapter, { onCatalogChanged });
    await chooseScoresAndPreview();

    fireEvent.click(screen.getByRole("button", { name: "Review apply" }));
    expect(adapter.apply).not.toHaveBeenCalled();
    expect(screen.getByText("Destination root: D:\\Music\\Scores")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Move and catalog 2" }));

    await screen.findByText("2 of 2 albums fully moved · 2 cataloged · 19 tracks");
    expect(adapter.apply).toHaveBeenCalledWith({ planId: "plan-42", sessionId: 42 });
    expect(onCatalogChanged).toHaveBeenCalledOnce();
  });

  it("stays candid when the immediate Aurora catalog refresh is still pending", async () => {
    renderDialog(createAdapter(), { onCatalogChanged: vi.fn().mockResolvedValue(false) });
    await chooseScoresAndPreview();
    fireEvent.click(screen.getByRole("button", { name: "Review apply" }));
    fireEvent.click(screen.getByRole("button", { name: "Move and catalog 2" }));

    expect(await screen.findByText(/has not detected the new catalog revision yet/)).toBeInTheDocument();
    expect(screen.getByText(/Aurora refresh is still pending/)).toBeInTheDocument();
  });

  it("keeps intake disabled when the Music Library companion is unavailable", async () => {
    renderDialog(createAdapter({
      capabilities: vi.fn().mockRejectedValue(new Error("Music Library helper was not found.")),
    }));

    expect(await screen.findByText("Music Library companion unavailable")).toBeInTheDocument();
    expect(screen.getByText("Music Library helper was not found.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose folder" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Preview batch" })).toBeDisabled();
  });

  it("does not claim a complete move when source cleanup leaves folders behind", async () => {
    const warningResult: LibraryIntakeApplyResult = {
      ...applyResult,
      status: "completedWithWarnings",
      movedAlbumCount: 1,
      albums: applyResult.albums.map((album, index) => ({
        ...album,
        cleanupStatus: index === 0 ? "removed" : "retained",
      })),
      cleanupWarnings: ["Close the file using the retained source folder, then remove it manually."],
    };
    renderDialog(createAdapter({ apply: vi.fn().mockResolvedValue(warningResult) }));
    await chooseScoresAndPreview();
    fireEvent.click(screen.getByRole("button", { name: "Review apply" }));
    fireEvent.click(screen.getByRole("button", { name: "Move and catalog 2" }));

    expect(await screen.findByText("1 of 2 albums fully moved · 2 cataloged · 19 tracks")).toBeInTheDocument();
    expect(screen.getByText("1 source album folder was retained and must be cleaned up manually.")).toBeInTheDocument();
    expect(screen.getByText(/Close the file using the retained source folder/)).toBeInTheDocument();
  });

  it("ignores Escape and disables closing while a preview is running", async () => {
    let resolvePreview: (value: LibraryIntakePreview) => void = () => undefined;
    const pendingPreview = new Promise<LibraryIntakePreview>((resolve) => { resolvePreview = resolve; });
    const onClose = vi.fn();
    renderDialog(createAdapter({ preview: vi.fn().mockReturnValue(pendingPreview) }), { onClose });
    await screen.findByText("Music Library companion ready");
    fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
    await screen.findByText("C:\\Intake");
    fireEvent.click(screen.getByRole("radio", { name: /Movie \/ TV \/ game music/ }));
    fireEvent.click(screen.getByRole("button", { name: "Preview batch" }));

    expect(screen.getByRole("button", { name: "Close add music" })).toBeDisabled();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();

    await act(async () => { resolvePreview(preview); });
    await waitFor(() => expect(screen.getByRole("button", { name: "Close add music" })).not.toBeDisabled());
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });
});
