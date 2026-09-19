import { describe, expect, it } from "vitest";
import { artistFacts, biographyText, listeningMonths, findArtistTrack } from "./artistPage";
import type { ArtistProfile } from "./musicbrainz";

const profile: ArtistProfile = { artistType: "Person", lifeBeginDate: "1950-01-02", lifeEndDate: null, lifeEnded: true, countryCode: "NO", countryName: "Norway", beginAreaName: "Oslo", areaName: null, endAreaName: null, sortName: null, gender: null };
describe("artist profile presentation", () => {
  it("distinguishes birth/death from formation/dissolution without inventing dates", () => {
    expect(artistFacts(profile)).toContainEqual(["Born", "1950-01-02"]);
    expect(artistFacts(profile)).toContainEqual(["Died", "Date unknown"]);
    expect(artistFacts({ ...profile, artistType: "Group", lifeEnded: false })).toContainEqual(["Founded", "1950-01-02"]);
    expect(artistFacts({ ...profile, artistType: "Group", lifeEnded: false }).some(([label]) => label === "Dissolved")).toBe(false);
    expect(artistFacts(null)).toEqual([]);
  });
  it("turns provider markup into safe readable text and removes the attribution anchor", () => {
    expect(biographyText('A &amp; B <b>formed</b> here.<script>alert(1)</script><a href="https://last.fm">Read more</a>')).toBe("A & B formed here.");
  });
  it("fills a twelve-month UTC series with zeroes across a year boundary", () => {
    const months = listeningMonths({ "2025-12": 3, "2026-01": 2, "2024-02": 100 }, new Date("2026-01-20T12:00:00Z"));
    expect(months).toHaveLength(12);
    expect(months[0]).toMatchObject({ key: "2025-02", plays: 0 });
    expect(months[11]).toMatchObject({ key: "2026-01", plays: 2 });
    expect(months.reduce((n, month) => n + month.plays, 0)).toBe(5);
  });
  it("does not play a similarly named song for a missing provider title", async () => {
    expect(await findArtistTrack("Coldplay", "Viva")).toBeNull();
  });
});
