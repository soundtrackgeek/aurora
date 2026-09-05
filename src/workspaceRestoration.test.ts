import { afterEach, describe, expect, it, vi } from "vitest";
import { loadWorkspaceCheckpoint, restoreWorkspaceScroll, saveWorkspaceCheckpoint, loadWorkspacePages } from "./workspaceRestoration";

afterEach(() => { vi.useRealTimers(); localStorage.clear(); });

describe("restart workspace", () => {
  it("reloads through the saved page and keeps the continuation cursor", async () => {
    const load = vi.fn(async (cursor?: number) => ({ tracks: [], artists: [], albums: Array.from({ length: 50 }, (_, i) => (cursor ?? 0) + i), nextCursor: (cursor ?? 0) + 50 }));
    const page = await loadWorkspacePages(load, 150, () => false);
    expect(page.albums).toEqual(Array.from({ length: 150 }, (_, i) => i));
    expect(page.nextCursor).toBe(150);
    expect(load.mock.calls).toEqual([[], [50], [100]]);
  });

  it("stops at the end of a changed catalog and cancels obsolete requests", async () => {
    const load = vi.fn(async () => ({ tracks: [], artists: [], albums: ["last"], nextCursor: null }));
    expect((await loadWorkspacePages(load, 150, () => false)).albums).toEqual(["last"]);
    expect(load).toHaveBeenCalledOnce();
    const cancelledLoad = vi.fn(async () => ({ tracks: [], artists: [], albums: ["first"], nextCursor: 50 }));
    await loadWorkspacePages(cancelledLoad, 150, () => true);
    expect(cancelledLoad).toHaveBeenCalledOnce();
  });
  it("persists deep pagination, selected track and exact per-view offsets", () => {
    const checkpoint = { scroll: { Albums: 9123, Songs: 231 }, explorerKey: "albums:filters", loaded: 350, trackKey: "track-9" };
    saveWorkspaceCheckpoint(checkpoint);
    expect(loadWorkspaceCheckpoint()).toEqual(checkpoint);
  });

  it("ignores malformed and invalid stored offsets", () => {
    localStorage.setItem("aurora:workspace:v1", '{"scroll":{"Albums":-1,"Songs":"12"},"loaded":-50}');
    expect(loadWorkspaceCheckpoint()).toEqual({ scroll: {}, explorerKey: null, loaded: 0, trackKey: null });
    localStorage.setItem("aurora:workspace:v1", "broken");
    expect(loadWorkspaceCheckpoint().scroll).toEqual({});
  });

  it("waits beyond the old 500ms timeout for pages and album detail, then releases scrolling", () => {
    vi.useFakeTimers();
    const element = document.createElement("div");
    let maximum = 0;
    let position = 0;
    let ready = false;
    Object.defineProperty(element, "scrollTop", { get: () => position, set: (value: number) => { position = Math.min(maximum, value); } });
    const done = vi.fn();
    const cleanup = restoreWorkspaceScroll(element, 9123, () => ready, done);
    vi.advanceTimersByTime(2000);
    expect(position).toBe(0);
    expect(done).not.toHaveBeenCalled();
    maximum = 12000;
    ready = true;
    vi.advanceTimersByTime(50);
    expect(position).toBe(9123);
    expect(done).toHaveBeenCalledOnce();
    element.scrollTop = 200;
    vi.advanceTimersByTime(500);
    expect(position).toBe(200);
    cleanup();
  });

  it("lets user input cancel pending restoration", () => {
    vi.useFakeTimers();
    const element = document.createElement("div");
    const cleanup = restoreWorkspaceScroll(element, 900, () => false, vi.fn());
    element.dispatchEvent(new WheelEvent("wheel"));
    element.scrollTop = 123;
    vi.advanceTimersByTime(2000);
    expect(element.scrollTop).toBe(123);
    cleanup();
  });
});
