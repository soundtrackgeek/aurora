import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { check } from "@tauri-apps/plugin-updater";
import { saveWindowState } from "@tauri-apps/plugin-window-state";
import { invoke } from "@tauri-apps/api/core";
import { useAuroraUpdater } from "./updater";

vi.mock("./library", () => ({ isTauriRuntime: () => true }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-window-state", () => ({ saveWindowState: vi.fn(), StateFlags: { SIZE: 1, POSITION: 2, MAXIMIZED: 4 } }));
afterEach(() => { vi.unstubAllEnvs(); vi.resetAllMocks(); });

it("awaits the window checkpoint after downloading and before installer exit", async () => {
  vi.stubEnv("DEV", false);
  const events: string[] = [];
  const update = { version: "1.0.0", download: vi.fn(async () => { events.push("download"); }), install: vi.fn(async () => { events.push("install"); }), close: vi.fn() };
  vi.mocked(check).mockResolvedValue(update as unknown as Awaited<ReturnType<typeof check>>);
  vi.mocked(invoke).mockImplementation(async () => { events.push("playback"); });
  let finishSave: () => void = () => undefined;
  vi.mocked(saveWindowState).mockImplementation(() => new Promise<void>((resolve) => { events.push("save"); finishSave = resolve; }));
  const { result, unmount } = renderHook(useAuroraUpdater);
  await waitFor(() => expect(result.current.state.phase).toBe("available"));
  let installing: Promise<void>;
  await act(async () => { installing = result.current.install(); });
  expect(events).toEqual(["download", "save"]);
  expect(saveWindowState).toHaveBeenCalledWith(7);
  await act(async () => { finishSave(); await installing; });
  expect(events).toEqual(["download", "save", "playback", "install"]);
  expect(invoke).toHaveBeenCalledWith("prepare_playback_shutdown");
  unmount();
});

it("waits for pending playback writes and cancels installation if they cannot drain", async () => {
  vi.stubEnv("DEV", false);
  const update = { version: "1.0.0", download: vi.fn(), install: vi.fn(), close: vi.fn() };
  vi.mocked(check).mockResolvedValue(update as unknown as Awaited<ReturnType<typeof check>>);
  vi.mocked(saveWindowState).mockResolvedValue(undefined);
  let rejectSave: (error: Error) => void = () => undefined;
  vi.mocked(invoke).mockImplementation(() => new Promise((_, reject) => { rejectSave = reject; }));
  const { result, unmount } = renderHook(useAuroraUpdater);
  await waitFor(() => expect(result.current.state.phase).toBe("available"));
  let installing: Promise<void>;
  await act(async () => { installing = result.current.install(); });
  expect(invoke).toHaveBeenCalledWith("prepare_playback_shutdown");
  expect(update.install).not.toHaveBeenCalled();
  await act(async () => { rejectSave(new Error("History storage is busy")); await installing; });
  expect(update.install).not.toHaveBeenCalled();
  expect(result.current.state.phase).toBe("error");
  expect(result.current.state.message).toBe("History storage is busy");
  unmount();
});

it("releases prepared playback if the installer fails", async () => {
  vi.stubEnv("DEV", false);
  const update = { version: "1.0.0", download: vi.fn(), install: vi.fn().mockRejectedValue(new Error("Installer failed")), close: vi.fn() };
  vi.mocked(check).mockResolvedValue(update as unknown as Awaited<ReturnType<typeof check>>);
  vi.mocked(saveWindowState).mockResolvedValue(undefined);
  vi.mocked(invoke).mockResolvedValue(undefined);
  const { result, unmount } = renderHook(useAuroraUpdater);
  await waitFor(() => expect(result.current.state.phase).toBe("available"));
  await act(async () => { await result.current.install(); });
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["prepare_playback_shutdown", "cancel_playback_shutdown"]);
  expect(result.current.state.message).toBe("Installer failed");
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
