import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import App from "./App";
import * as library from "./library";
import * as libraryAutoTag from "./libraryAutoTag";

vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));

beforeAll(async () => {
  await Promise.all([
    import("./app/routes/LibraryRoute"),
    import("./components/inbox/LibraryAutoTagDialog"),
  ]);
});

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

function markAlbums(naming: library.AlbumNamingCheck) {
  const exploreAlbums = library.exploreAlbums;
  vi.spyOn(library, "exploreAlbums").mockImplementation(async (...args) => {
    const page = await exploreAlbums(...args);
    return { ...page, items: page.items.map((album) => ({ ...album, naming })) };
  });
}

async function openVivaLaVida() {
  render(<App />);
  const primary = await screen.findByRole("navigation", { name: "Primary" });
  fireEvent.click(within(primary).getByRole("button", { name: "Albums" }));
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  await screen.findByRole("complementary", { name: "Viva la Vida album details" });
}

it("renames the open misnamed album with Ctrl+R instead of reloading the window", async () => {
  markAlbums({ status: "misnamed", issues: ["Folder should be named Coldplay - Viva la Vida (2008)."] });
  const rename = vi.spyOn(libraryAutoTag, "renameLibraryAlbum").mockResolvedValue({ albumId: "x", sourcePath: "D:/Music/Old", destinationPath: "D:/Music/Old", folderRenamed: false, renamedFiles: 0, warnings: [] });
  await openVivaLaVida();
  expect(fireEvent.keyDown(window, { key: "r", ctrlKey: true })).toBe(false);
  await waitFor(() => expect(rename).toHaveBeenCalledTimes(1));
});

it("leaves an organized album alone on Ctrl+R but still blocks the reload", async () => {
  markAlbums({ status: "organized" });
  const rename = vi.spyOn(libraryAutoTag, "renameLibraryAlbum");
  await openVivaLaVida();
  expect(fireEvent.keyDown(window, { key: "r", ctrlKey: true })).toBe(false);
  expect(rename).not.toHaveBeenCalled();
});

it("opens the auto-tagger for the open album with Ctrl+Shift+T", async () => {
  const load = vi.spyOn(libraryAutoTag, "loadLibraryAutoTagAlbum").mockReturnValue(new Promise(() => {}));
  await openVivaLaVida();
  expect(fireEvent.keyDown(window, { key: "T", ctrlKey: true, shiftKey: true })).toBe(false);
  await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
});
