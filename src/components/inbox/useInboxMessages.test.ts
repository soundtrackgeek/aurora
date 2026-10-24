import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useInboxMessages } from "./useInboxMessages";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("Inbox messages", () => {
  it("expires repeated identical results independently and keeps them in timestamped history", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-24T12:00:00Z"));
    const { result } = renderHook(useInboxMessages);
    act(() => result.current.notify("10 tracks renamed."));
    act(() => vi.advanceTimersByTime(3_000));
    act(() => result.current.notify("10 tracks renamed."));
    act(() => vi.advanceTimersByTime(1_999));
    expect(result.current.messages.every((message) => message.visible)).toBe(true);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.messages.map((message) => message.visible)).toEqual([true, false]);
    expect(result.current.messages[0].id).not.toBe(result.current.messages[1].id);
    expect(result.current.messages[0].createdAt - result.current.messages[1].createdAt).toBe(3_000);
    act(() => vi.advanceTimersByTime(3_000));
    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages.every((message) => !message.visible)).toBe(true);
  });

  it("retains dismissed failures and bounds history to the newest 100 results", () => {
    vi.useFakeTimers();
    const { result } = renderHook(useInboxMessages);
    act(() => {
      for (let index = 0; index < 101; index++) result.current.notify(`Result ${index}`);
      result.current.notify("File is locked.", "error");
    });
    const latest = result.current.messages[0];
    act(() => result.current.dismiss(latest.id));
    expect(result.current.messages).toHaveLength(100);
    expect(result.current.messages[0]).toMatchObject({ text: "File is locked.", kind: "error", visible: false });
    expect(result.current.messages[result.current.messages.length - 1].text).toBe("Result 2");
  });

  it("cleans up the expiry timer when Inbox unmounts", () => {
    vi.useFakeTimers();
    const { result, unmount } = renderHook(useInboxMessages);
    act(() => result.current.notify("Saved."));
    expect(vi.getTimerCount()).toBe(1);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
