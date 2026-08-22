import { beforeEach, describe, expect, it } from "vitest";
import {
  loadHistoryPage,
  loadTrackHistoryInsight,
  resetHistoryPreview,
  saveHistoryPlayThreshold,
} from "./history";

describe("browser listening-history adapter", () => {
  beforeEach(() => resetHistoryPreview());

  it("defaults to 30 seconds and persists a valid configured threshold", async () => {
    expect((await loadHistoryPage({ pageSize: 10 })).playThresholdSeconds).toBe(30);

    await saveHistoryPlayThreshold(42);

    expect((await loadHistoryPage({ pageSize: 10 })).playThresholdSeconds).toBe(42);
  });

  it("rejects invalid thresholds and applies timeline filters", async () => {
    await expect(saveHistoryPlayThreshold(0)).rejects.toThrow(/between 1 and 3600/);
    await expect(saveHistoryPlayThreshold(3_601)).rejects.toThrow(/between 1 and 3600/);

    const all = await loadHistoryPage({ pageSize: 100 });
    const laptop = await loadHistoryPage({
      pageSize: 100,
      deviceId: "device-keiya-preview",
      outcome: "completed",
    });

    expect(all.items.length).toBeGreaterThan(laptop.items.length);
    expect(laptop.items.length).toBeGreaterThan(0);
    expect(laptop.items.every((item) => item.deviceName === "Keiya" && item.outcome === "completed")).toBe(true);
  });

  it("summarizes personal listening for a selected track", async () => {
    const page = await loadHistoryPage({ pageSize: 1 });
    const trackKey = page.items[0].trackKey;
    const insight = await loadTrackHistoryInsight(trackKey);

    expect(insight.sessions).toBeGreaterThan(0);
    expect(insight.plays).toBeGreaterThan(0);
    expect(insight.listenedSeconds).toBeGreaterThan(0);
    expect(insight.lastListenedAtMs).not.toBeNull();
  });
});
