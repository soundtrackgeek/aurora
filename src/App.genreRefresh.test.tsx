import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import * as genres from "./genres";
import * as library from "./library";
import type { CatalogSync } from "./tags";
import { defaultExplorerFilters, saveViewPreferences } from "./viewPreferences";

vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));
vi.mock("./components/TagEditor", () => ({
  TagEditor: ({ onCatalogSync }: { onCatalogSync: (sync: CatalogSync) => void }) => (
    <button onClick={() => onCatalogSync({ status: "pending", pendingFolderCount: 1, projectionToken: 100 })}>Complete tag sync</button>
  ),
}));
afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

// This integration test boots the full app before the scheduled chart request.
// Shared CI runners can need longer than Testing Library's one-second default.
const readiness = { timeout: 5_000 };

it("refreshes cached Genres after saved tags even while import is pending", async () => {
  const snapshot = await library.loadLibrarySnapshot();
  vi.spyOn(library, "loadLibrarySnapshot").mockResolvedValue(snapshot);
  vi.spyOn(library, "loadCatalogRevision").mockResolvedValue(snapshot.catalogRevision);
  const index = vi.spyOn(genres, "loadGenreIndex");
  const detail = vi.spyOn(genres, "loadGenreDetail");
  saveViewPreferences({ activeNav: "Genres", explorerView: "albums", explorerFilters: defaultExplorerFilters,
    inspectorView: "track", tagSelectionKind: "track", selectedAlbumId: null });
  render(<App />);
  await waitFor(() => expect(detail).toHaveBeenCalled(), readiness);
  const tags = within(screen.getByRole("tablist", { name: "Library details" })).getByRole("tab", { name: "Tags" });
  await waitFor(() => expect(tags).toBeEnabled(), readiness);
  fireEvent.click(tags);
  const indexLoads = index.mock.calls.length;
  const detailLoads = detail.mock.calls.length;
  fireEvent.click(await screen.findByRole("button", { name: "Complete tag sync" }, readiness));
  await waitFor(() => expect(index.mock.calls.length).toBeGreaterThan(indexLoads), readiness);
  await waitFor(() => expect(detail.mock.calls.length).toBeGreaterThan(detailLoads), readiness);
}, 20_000);
