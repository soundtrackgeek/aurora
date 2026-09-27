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

// This integration test boots the full app before the scheduled chart request.
// Shared CI runners can need longer than Testing Library's one-second default.
const readiness = { timeout: 5_000 };

it.each([0, 1_250])("refreshes retained Charts after tag sync with %i ms startup latency and no new import revision", async (startupLatency) => {
  const snapshot = await library.loadLibrarySnapshot();
  vi.spyOn(library, "loadLibrarySnapshot").mockImplementation(async () => {
    if (startupLatency) await new Promise((resolve) => setTimeout(resolve, startupLatency));
    return snapshot;
  });
  vi.spyOn(library, "loadCatalogRevision").mockResolvedValue(snapshot.catalogRevision);
  const load = vi.spyOn(charts, "loadChartPage");
  saveViewPreferences({ activeNav: "Charts", explorerView: "albums", explorerFilters: defaultExplorerFilters,
    inspectorView: "track", tagSelectionKind: "track", selectedAlbumId: null });
  render(<App />);
  await screen.findByRole("heading", { name: "Official UK Singles Chart" }, readiness);
  const tags = within(screen.getByRole("tablist", { name: "Library details" })).getByRole("tab", { name: "Tags" });
  await waitFor(() => expect(tags).toBeEnabled(), readiness);
  fireEvent.click(tags);
  const previousLoads = load.mock.calls.length;
  fireEvent.click(await screen.findByRole("button", { name: "Complete tag sync" }, readiness));
  await waitFor(() => expect(load.mock.calls.length).toBeGreaterThan(previousLoads), readiness);
  expect(tags).toHaveAttribute("aria-selected", "true");
}, 20_000);
