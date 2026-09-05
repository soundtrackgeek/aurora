import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { check } from "@tauri-apps/plugin-updater";
import { saveWindowState } from "@tauri-apps/plugin-window-state";
import { useAuroraUpdater } from "./updater";

vi.mock("./library", () => ({ isTauriRuntime: () => true }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-window-state", () => ({ saveWindowState: vi.fn(), StateFlags: { SIZE: 1, POSITION: 2, MAXIMIZED: 4 } }));
afterEach(() => { vi.unstubAllEnvs(); vi.clearAllMocks(); });

it("awaits the window checkpoint after downloading and before installer exit", async () => {
  vi.stubEnv("DEV", false);
  const events: string[] = [];
  const update = { version: "1.0.0", download: vi.fn(async () => { events.push("download"); }), install: vi.fn(async () => { events.push("install"); }), close: vi.fn() };
  vi.mocked(check).mockResolvedValue(update as unknown as Awaited<ReturnType<typeof check>>);
  let finishSave: () => void = () => undefined;
  vi.mocked(saveWindowState).mockImplementation(() => new Promise<void>((resolve) => { events.push("save"); finishSave = resolve; }));
  const { result, unmount } = renderHook(useAuroraUpdater);
  await waitFor(() => expect(result.current.state.phase).toBe("available"));
  let installing: Promise<void>;
  await act(async () => { installing = result.current.install(); });
  expect(events).toEqual(["download", "save"]);
  expect(saveWindowState).toHaveBeenCalledWith(7);
  await act(async () => { finishSave(); await installing; });
  expect(events).toEqual(["download", "save", "install"]);
  unmount();
});

it("reports a failed checkpoint and keeps Aurora open instead of losing geometry", async () => {
  vi.stubEnv("DEV", false);
  const update = { version: "1.0.0", download: vi.fn(), install: vi.fn(), close: vi.fn() };
  vi.mocked(check).mockResolvedValue(update as unknown as Awaited<ReturnType<typeof check>>);
  vi.mocked(saveWindowState).mockRejectedValue(new Error("Cannot save window state"));
  const { result, unmount } = renderHook(useAuroraUpdater);
  await waitFor(() => expect(result.current.state.phase).toBe("available"));
  await act(async () => { await result.current.install(); });
  expect(result.current.state.phase).toBe("error");
  expect(update.install).not.toHaveBeenCalled();
  unmount();
});
