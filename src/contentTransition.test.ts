import { afterEach, describe, expect, it, vi } from "vitest";
import { transitionContent } from "./contentTransition";

const { start, addType } = vi.hoisted(() => ({
  start: vi.fn((update: () => void) => update()),
  addType: vi.fn(),
}));
vi.mock("react", () => ({ startTransition: start, addTransitionType: addType }));

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("content transitions", () => {
  it("keeps preserved catalog refreshes immediate even with animation support", () => {
    vi.stubGlobal("document", { startViewTransition: vi.fn() });
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false })));
    const update = vi.fn();
    transitionContent(update, "album-detail", false);
    expect(update).toHaveBeenCalledOnce();
    expect(start).not.toHaveBeenCalled();
  });

  it("updates synchronously on unsupported WebViews", () => {
    const update = vi.fn();
    transitionContent(update, "album-detail");
    expect(update).toHaveBeenCalledOnce();
    expect(start).not.toHaveBeenCalled();
  });

  it("bypasses animation when reduced motion is requested", () => {
    vi.stubGlobal("document", { startViewTransition: vi.fn() });
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: true })));
    const update = vi.fn();
    transitionContent(update, "collection");
    expect(update).toHaveBeenCalledOnce();
    expect(start).not.toHaveBeenCalled();
  });

  it("marks supported updates with their scoped transition type", () => {
    vi.stubGlobal("document", { startViewTransition: vi.fn() });
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false })));
    const update = vi.fn();
    transitionContent(update, "artist-detail");
    expect(start).toHaveBeenCalledOnce();
    expect(addType).toHaveBeenCalledWith("artist-detail");
    expect(update).toHaveBeenCalledOnce();
  });
});
