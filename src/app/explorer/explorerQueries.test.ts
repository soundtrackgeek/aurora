import { describe, expect, it } from "vitest";
import { withCatalogGroups } from "./explorerQueries";

describe("withCatalogGroups", () => {
  it("puts each item under the heading its page gave it", () => {
    const items = [{ id: "a" }, { id: "b" }];
    expect(withCatalogGroups(items, [
      { id: "a", key: "0iceland", label: "Iceland" },
      { id: "b", key: "1", label: "No country" },
    ])).toEqual([
      { id: "a", catalogGroup: { key: "0iceland", label: "Iceland" } },
      { id: "b", catalogGroup: { key: "1", label: "No country" } },
    ]);
  });

  it("leaves items alone without a group:", () => {
    const items = [{ id: "a" }];
    expect(withCatalogGroups(items, undefined)).toBe(items);
    expect(withCatalogGroups(items, [])).toBe(items);
  });
});
