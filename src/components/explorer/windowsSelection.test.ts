import { describe, expect, it } from "vitest";
import { applyWindowsSelection } from "./windowsSelection";

const ordered = ["one", "two", "three", "four"];

describe("applyWindowsSelection", () => {
  it("replaces the selection on a plain click", () => {
    const result = applyWindowsSelection(ordered, new Set(["one", "two"]), "one", "three", { ctrl: false, shift: false });
    expect([...result.selectedKeys]).toEqual(["three"]);
    expect(result.anchorKey).toBe("three");
  });

  it("toggles one item on Ctrl-click", () => {
    const added = applyWindowsSelection(ordered, new Set(["one"]), "one", "three", { ctrl: true, shift: false });
    expect([...added.selectedKeys]).toEqual(["one", "three"]);
    const removed = applyWindowsSelection(ordered, added.selectedKeys, added.anchorKey, "one", { ctrl: true, shift: false });
    expect([...removed.selectedKeys]).toEqual(["three"]);
  });

  it("selects an inclusive range on Shift-click and preserves the anchor", () => {
    const result = applyWindowsSelection(ordered, new Set(["two"]), "two", "four", { ctrl: false, shift: true });
    expect([...result.selectedKeys]).toEqual(["two", "three", "four"]);
    expect(result.anchorKey).toBe("two");
  });

  it("adds a range on Ctrl-Shift-click", () => {
    const result = applyWindowsSelection(ordered, new Set(["one"]), "three", "four", { ctrl: true, shift: true });
    expect([...result.selectedKeys]).toEqual(["one", "three", "four"]);
  });
});
