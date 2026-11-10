import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import * as tags from "../tags";
import { describeNextRetry } from "../app/catalog/librarySyncFailure";
import { LibrarySyncDialog } from "./LibrarySyncDialog";

const NOW = 1_800_000_000_000;
const blocked: tags.LibrarySyncFolder = {
  directory: String.raw`G:\_BACKUP\SCORES\Jake Monaco - Playing It Cool (2014)`,
  blocked: true,
  attemptCount: 3,
  lastError: String.raw`Music Library could not complete syncExistingFolders: Could not sync \\?\G:\_BACKUP\SCORES\Jake Monaco - Playing It Cool (2014): Aurora background sync requires reviewed reconciliation: the album could not be updated. (importFailed)`,
  lastAttemptAtMs: NOW - 60_000,
  nextRetryAtMs: NOW + 5 * 60 * 60_000,
  pendingEditCount: 12,
};
const waiting: tags.LibrarySyncFolder = {
  directory: String.raw`D:\MUSIC\Andrew Powell - Ladyhawke (1985)`,
  blocked: false,
  attemptCount: 1,
  lastError: String.raw`Music Library could not complete syncExistingFolders: \\?\D:\MUSIC\Andrew Powell - Ladyhawke (1985)\2-01 - Navarre.mp3 has unsupported MusicBee rating byte 153 (validationFailed)`,
  lastAttemptAtMs: NOW - 60_000,
  nextRetryAtMs: 0,
  pendingEditCount: 1,
};

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) { this.open = true; });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

function renderDialog(onCatalogSync = vi.fn(), onOpenAlbum = vi.fn()) {
  render(<LibrarySyncDialog onClose={vi.fn()} onCatalogSync={onCatalogSync} onOpenAlbum={onOpenAlbum} now={() => NOW} />);
  return onCatalogSync;
}

describe("LibrarySyncDialog", () => {
  it("lists blocked folders first with their reason, fix and edit count", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([blocked, waiting]);
    renderDialog();

    expect(await screen.findByText(/2 folders · 13 edits waiting/)).toBeInTheDocument();
    const attention = screen.getByRole("region", { name: "Needs your attention" });
    expect(within(attention).getByText("Jake Monaco - Playing It Cool (2014)")).toBeInTheDocument();
    expect(within(attention).getByText("12 edits waiting")).toBeInTheDocument();
    expect(within(attention).getByText(/Album Artist is the same on every track/)).toBeInTheDocument();
    expect(within(attention).getByText(/in about 5 hours/)).toBeInTheDocument();

    const automatic = screen.getByRole("region", { name: "Retrying automatically" });
    expect(within(automatic).getByText(/rating byte 153/)).toBeInTheDocument();
    expect(within(automatic).getByText("2-01 - Navarre.mp3")).toBeInTheDocument();
    expect(within(automatic).getByText(/within a minute/)).toBeInTheDocument();
  });

  it("retries one folder, reports the new status and drops it once synced", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders")
      .mockResolvedValueOnce([blocked, waiting])
      .mockResolvedValueOnce([waiting]);
    const sync: tags.CatalogSync = { status: "pending", pendingFolderCount: 1 };
    const retry = vi.spyOn(tags, "retryLibrarySyncFolder").mockResolvedValue({ completed: true, catalogSync: sync });
    const onCatalogSync = renderDialog();

    fireEvent.click(await screen.findByRole("button", { name: "Retry Jake Monaco - Playing It Cool (2014)" }));

    expect(retry).toHaveBeenCalledWith(blocked.directory);
    expect(await screen.findByText("Jake Monaco - Playing It Cool (2014) is synced.")).toBeInTheDocument();
    expect(onCatalogSync).toHaveBeenCalledWith(sync);
    expect(screen.queryByRole("region", { name: "Needs your attention" })).not.toBeInTheDocument();
  });

  it("shows a retry that could not run next to its folder", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([blocked]);
    vi.spyOn(tags, "retryLibrarySyncFolder").mockRejectedValue(new Error("Music writes are paused in Laptop Mode."));
    renderDialog();

    fireEvent.click(await screen.findByRole("button", { name: /^Retry / }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Music writes are paused in Laptop Mode.");
  });

  it("disables every Retry while one runs", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([blocked, waiting]);
    let finish: (value: tags.LibrarySyncFolderRetry) => void = () => {};
    vi.spyOn(tags, "retryLibrarySyncFolder").mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    renderDialog();

    fireEvent.click(await screen.findByRole("button", { name: "Retry Jake Monaco - Playing It Cool (2014)" }));

    await waitFor(() => {
      for (const button of screen.getAllByRole("button", { name: /^Retry / })) expect(button).toBeDisabled();
    });
    finish({ completed: false, catalogSync: { status: "blocked", pendingFolderCount: 2 } });
    await waitFor(() => expect(screen.getAllByRole("button", { name: /^Retry / })[0]).toBeEnabled());
  });

  it("opens a folder in the file browser", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([waiting]);
    const reveal = vi.spyOn(tags, "revealLibrarySyncFolder").mockResolvedValue();
    renderDialog();

    fireEvent.click(await screen.findByRole("button", { name: "Open folder Andrew Powell - Ladyhawke (1985)" }));

    expect(reveal).toHaveBeenCalledWith(waiting.directory);
  });

  it("opens the folder's album in Aurora when the catalog has it", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([
      { ...blocked, albumId: "mb:42", albumTitle: "Playing It Cool" },
      waiting,
    ]);
    const onOpenAlbum = vi.fn();
    renderDialog(vi.fn(), onOpenAlbum);

    fireEvent.click(await screen.findByRole("button", { name: "Show album Jake Monaco - Playing It Cool (2014)" }));

    expect(onOpenAlbum).toHaveBeenCalledWith("mb:42", "Playing It Cool");
    // A folder without a catalog album offers no album link.
    expect(screen.queryByRole("button", { name: "Show album Andrew Powell - Ladyhawke (1985)" })).not.toBeInTheDocument();
  });

  it("names a track missing from the folder as the reason", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([
      { ...blocked, albumId: "mb:42", albumTitle: "Playing It Cool", missingTracks: ["Are You Serious?"] },
    ]);
    renderDialog();

    expect(await screen.findByText(/"Are You Serious\?" is missing from this folder/)).toBeInTheDocument();
    expect(screen.getByText(/use Move to Inbox on the album/)).toBeInTheDocument();
    expect(screen.queryByText(/Album Artist/)).not.toBeInTheDocument();
  });

  it("says so when nothing is waiting", async () => {
    vi.spyOn(tags, "loadLibrarySyncFolders").mockResolvedValue([]);
    renderDialog();
    expect(await screen.findByText("Every saved edit is in Music Library.")).toBeInTheDocument();
  });
});

describe("describeNextRetry", () => {
  it("reads minutes and hours", () => {
    expect(describeNextRetry(NOW + 30_000, NOW)).toMatch(/within a minute/);
    expect(describeNextRetry(NOW + 10 * 60_000, NOW)).toMatch(/in 10 minutes/);
    expect(describeNextRetry(NOW + 23 * 60 * 60_000, NOW)).toMatch(/about 23 hours/);
  });
});
