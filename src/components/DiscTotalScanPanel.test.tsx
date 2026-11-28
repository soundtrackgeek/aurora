import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as data from "../discTotals";
import { describeDiscTotalScan, formatRemaining } from "../discTotals";
import { DiscTotalScanPanel } from "./DiscTotalScanPanel";

afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); });

const base = { activity: "reading" as const, readFiles: 250_000, catalogFiles: 1_000_000, filesPerMinute: 5_000, error: null };

it("shows read progress, percent and time left while reading", async () => {
  vi.spyOn(data, "loadDiscTotalScanStatus").mockResolvedValue(base);
  render(<DiscTotalScanPanel />);
  expect(await screen.findByText("Reading tags · 250,000 of 1,000,000 files read · about 2.5 h left")).toBeInTheDocument();
  expect(screen.getByText("25%")).toBeInTheDocument();
  expect(screen.getByLabelText("Disc Total scan progress")).toHaveAttribute("value", "250000");
});

it("polls and reports completion", async () => {
  vi.useFakeTimers();
  const load = vi.spyOn(data, "loadDiscTotalScanStatus")
    .mockResolvedValueOnce(base)
    .mockResolvedValue({ ...base, activity: "resting", readFiles: 1_000_000, filesPerMinute: null });
  render(<DiscTotalScanPanel />);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { vi.advanceTimersByTime(5000); await Promise.resolve(); });
  expect(load).toHaveBeenCalledTimes(2);
  expect(screen.getByText("Complete")).toBeInTheDocument();
  expect(screen.getByText("Up to date · 1,000,000 of 1,000,000 files read")).toBeInTheDocument();
});

it("describes paused, unreadable and failing scans", async () => {
  expect(describeDiscTotalScan({ ...base, activity: "paused" })).toBe("Paused in Network or Laptop Mode · 250,000 of 1,000,000 files read on the home PC");
  expect(describeDiscTotalScan({ ...base, activity: "resting", readFiles: 999_990 })).toBe("Up to date · 999,990 of 1,000,000 files read · 10 could not be read");
  expect(describeDiscTotalScan({ ...base, filesPerMinute: null })).toBe("Reading tags · 250,000 of 1,000,000 files read");
  expect(formatRemaining(0.5)).toBe("less than a minute");
  expect(formatRemaining(42)).toBe("about 42 min");
  expect(formatRemaining(60 * 14)).toBe("about 14 h");
  vi.spyOn(data, "loadDiscTotalScanStatus").mockResolvedValue({ ...base, error: "Could not open the Disc Total cache" });
  render(<DiscTotalScanPanel />);
  expect(await screen.findByText("Could not open the Disc Total cache")).toBeInTheDocument();
});
