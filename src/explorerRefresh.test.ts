import { describe, expect, it } from "vitest";
import { mergeRefreshedExplorerPage } from "./explorerRefresh";

describe("mergeRefreshedExplorerPage", () => {
  it("updates loaded items without moving the user's current position", () => {
    const current = [
      { id: "album-1", title: "First" },
      { id: "album-2", title: "Second" },
      { id: "album-3", title: "Third" },
    ];
    const refreshed = [
      { id: "album-new", title: "Imported" },
      { id: "album-2", title: "Second (retagged)" },
      { id: "album-1", title: "First" },
    ];

    expect(mergeRefreshedExplorerPage(current, refreshed)).toEqual([
      { id: "album-1", title: "First" },
      { id: "album-2", title: "Second (retagged)" },
      { id: "album-3", title: "Third" },
      { id: "album-new", title: "Imported" },
    ]);
  });
});
