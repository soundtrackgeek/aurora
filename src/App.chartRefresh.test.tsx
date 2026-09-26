import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import * as charts from "./charts";
import * as library from "./library";
import type { CatalogSync } from "./tags";
import { defaultExplorerFilters, saveViewPreferences } from "./viewPreferences";

vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));
vi.mock("./components/TagEditor", () => ({
  TagEditor: ({ onCatalogSync }: { onCatalogSync: (sync: CatalogSync) => void }) => (
    <button onClick={() => onCatalogSync({ status: "synced", pendingFolderCount: 0, projectionToken: 100 })}>Complete tag sync</button>
  ),
}));
afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

it("refreshes retained Charts after tag sync without a new import revision", async () => {
  const snapshot = await library.loadLibrarySnapshot();
  vi.spyOn(library, "loadCatalogRevision").mockResolvedValue(snapshot.catalogRevision);
  const load = vi.spyOn(charts, "loadChartPage");
  saveViewPreferences({ activeNav: "Charts", explorerView: "albums", explorerFilters: defaultExplorerFilters,
    inspectorView: "track", tagSelectionKind: "track", selectedAlbumId: null });
  render(<App />);
  await screen.findByRole("heading", { name: "Official UK Singles Chart" });
  const tags = within(screen.getByRole("tablist", { name: "Library details" })).getByRole("tab", { name: "Tags" });
  await waitFor(() => expect(tags).toBeEnabled());
  fireEvent.click(tags);
  const previousLoads = load.mock.calls.length;
  fireEvent.click(await screen.findByRole("button", { name: "Complete tag sync" }));
  await waitFor(() => expect(load.mock.calls.length).toBeGreaterThan(previousLoads));
  expect(tags).toHaveAttribute("aria-selected", "true");
});
