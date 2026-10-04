import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { defaultViewPreferences } from "../../viewPreferences";
import { useExplorerWorkspace } from "../explorer/useExplorerWorkspace";
import { useWorkspaceCheckpoint, type WorkspaceRestoration } from "./useWorkspaceRestoration";

afterEach(() => { cleanup(); vi.useRealTimers(); localStorage.clear(); });

function restorationFixture() {
  const element = document.createElement("div");
  let maximum = 0;
  let position = 0;
  Object.defineProperty(element, "scrollTop", {
    get: () => position,
    set: (value: number) => { position = Math.min(maximum, value); },
  });
  Object.defineProperty(element, "scrollHeight", { get: () => maximum + 100 });
  const checkpoint = { scroll: { Songs: 600 }, explorerKey: null, loaded: 0, trackKey: null };
  const workspace: WorkspaceRestoration = {
    initialWorkspace: checkpoint,
    workspaceRef: { current: checkpoint },
    restoringScrollRef: { current: false },
    scrollReadyRef: { current: true },
    mainScrollRef: { current: element },
    scrollPositionByDestinationRef: { current: checkpoint.scroll },
    rememberScroll: vi.fn(),
  };
  function mount() {
    return renderHook(() => {
      const explorer = useExplorerWorkspace(defaultViewPreferences);
      useWorkspaceCheckpoint({ workspace, explorer, selectedTrack: null, pageKey: "Songs", artistPageName: null, navigationRevision: 0 });
    });
  }
  return { element, workspace, mount, reveal: () => { maximum = 1200; } };
}

it("waits for a lazy route reveal beyond 250ms before settling the saved scroll", () => {
  vi.useFakeTimers();
  const fixture = restorationFixture();
  const fallback = document.createElement("section");
  fallback.dataset.routeLoading = "";
  fixture.element.append(fallback);
  fixture.mount();

  act(() => vi.advanceTimersByTime(2000));
  expect(fixture.element.scrollTop).toBe(0);
  expect(fixture.workspace.restoringScrollRef.current).toBe(true);

  fallback.remove();
  fixture.reveal();
  act(() => vi.advanceTimersByTime(50));
  expect(fixture.element.scrollTop).toBe(600);
  expect(fixture.workspace.restoringScrollRef.current).toBe(false);
});

it.each(["fallback", "ancestor"])("ignores a retained route hidden by Activity on its %s", (hiddenTarget) => {
  vi.useFakeTimers();
  const fixture = restorationFixture();
  const retainedPage = document.createElement("div");
  const nestedContent = document.createElement("div");
  const fallback = document.createElement("section");
  fallback.dataset.routeLoading = "";
  (hiddenTarget === "fallback" ? fallback : retainedPage).style.display = "none";
  nestedContent.append(fallback);
  retainedPage.append(nestedContent);
  fixture.element.append(retainedPage);
  fixture.reveal();
  fixture.mount();

  expect(fixture.element.scrollTop).toBe(600);
  expect(fixture.workspace.restoringScrollRef.current).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
});
