import { afterEach, expect, it, vi } from "vitest";
import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { subscribeNativeEvent } from "./nativeEvents";

vi.mock("./library", () => ({ isTauriRuntime: () => true }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
afterEach(() => vi.resetAllMocks());

it("subscribes before loading initial state and ignores events after disposal", async () => {
  const release = vi.fn();
  vi.mocked(listen).mockResolvedValue(release);
  const receive = vi.fn();
  const ready = vi.fn();
  const stop = subscribeNativeEvent<string>("catalog://revision", receive, ready);
  expect(ready).not.toHaveBeenCalled();
  await Promise.resolve();
  expect(ready).toHaveBeenCalledOnce();
  const callback = vi.mocked(listen).mock.calls[0][1] as EventCallback<string>;
  callback({ event: "catalog://revision", id: 1, payload: "next" });
  expect(receive).toHaveBeenCalledWith("next");
  stop();
  callback({ event: "catalog://revision", id: 1, payload: "late" });
  expect(receive).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

it("releases a listener whose registration completes after unmount", async () => {
  let finish!: (release: UnlistenFn) => void;
  vi.mocked(listen).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const ready = vi.fn();
  const stop = subscribeNativeEvent<string>("sync://status", vi.fn(), ready);
  stop();
  const release = vi.fn();
  finish(release);
  await Promise.resolve();
  expect(release).toHaveBeenCalledOnce();
  expect(ready).not.toHaveBeenCalled();
});
