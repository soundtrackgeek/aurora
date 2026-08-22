import { describe, expect, it } from "vitest";
import { loadYearAlbumTracks, loadYearDetail, loadYearOverview, loadYearQueue } from "./years";

describe("Years browser adapter", () => {
  it("keeps Original and Release Year as separate clocks", async () => {
    const overview = await loadYearOverview();
    expect(overview.originalYears.length).toBeGreaterThan(70);
    expect(overview.releaseYears.length).toBeGreaterThan(70);
    expect(overview.stats.differentTracks).toBe(209_671);
    expect(overview.initialDetail.selection).toEqual({ basis: "release", year: 2025 });

    const original = await loadYearDetail({ basis: "original", year: 1982 });
    expect(original.selection).toEqual({ basis: "original", year: 1982 });
    expect(original.albums.every((album) => album.originalYear === 1982)).toBe(true);
    expect(original.flows.some((flow) => flow.year === 2025)).toBe(true);
  });

  it("returns only bounded playable tracks and album details", async () => {
    const detail = await loadYearDetail({ basis: "release", year: 2025 });
    const queue = await loadYearQueue(detail.selection, 3);
    expect(queue).toHaveLength(3);
    const albumTracks = await loadYearAlbumTracks(detail.albums[0]);
    expect(albumTracks.length).toBeGreaterThan(0);
    expect(albumTracks.length).toBeLessThanOrEqual(8);
  });
});
