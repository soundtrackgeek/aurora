import { describe, expect, it } from "vitest";
import type { InboxTrack, ReleaseTrack } from "./inbox";
import { normalizeInboxTrackTitle, reconcileInboxTracks } from "./inboxMatching";

function local(title: string, trackNumber: number, durationMs = 200_000): InboxTrack {
  return {
    path: `C:\\Inbox\\${trackNumber}.mp3`, fileName: `${trackNumber}.mp3`, format: "MP3",
    durationMs, albumArtist: "Rogue Male", title, artist: "Rogue Male", album: "Animal Man",
    genre: null, publisher: null, rating: null, year: null, releaseYear: null,
    trackNumber, trackTotal: 11, discNumber: null, discTotal: null,
  };
}

function release(title: string, trackNumber: number, durationMs = 200_000): ReleaseTrack {
  return { title, artist: "Rogue Male", trackNumber, trackTotal: 10, discNumber: 1, discTotal: 1, durationMs };
}

describe("Inbox release reconciliation", () => {
  it("matches an original track list and isolates a trailing bonus track", () => {
    const titles = ["Progress", "L.U.S.T.", "Take No Shit", "You're on Fire", "The Real Me", "Animal Man", "Belfast", "The Job Centre", "Low Rider", "The Passing"];
    const result = reconcileInboxTracks(
      [...titles.map((title, index) => local(title, index + 1)), local("Rough Tough (Pretty Too) (Bonus)", 11)],
      titles.map((title, index) => release(title === "The Job Centre" ? "Job Centre" : title, index + 1)),
    );

    expect(result).toMatchObject({ matchedCount: 10, exactCount: 10, extraCount: 1, missingCount: 0, cleanupSafe: true });
    expect(result.rows[result.rows.length - 1]).toEqual({ localIndex: 10, releaseIndex: null, status: "extra" });
  });

  it("matches exact titles across inserted bonus tracks instead of shifting by position", () => {
    const result = reconcileInboxTracks(
      [local("One", 1), local("Inserted Bonus", 2), local("Two", 3), local("Three", 4)],
      [release("One", 1), release("Two", 2), release("Three", 3)],
    );
    expect(result.matchedCount).toBe(3);
    expect(result.extraCount).toBe(1);
    expect(result.rows.find((row) => row.releaseIndex === 1)?.localIndex).toBe(2);
  });

  it("blocks cleanup when a release track is missing or duplicated ambiguously", () => {
    const missing = reconcileInboxTracks([local("One", 1)], [release("One", 1), release("Two", 2)]);
    expect(missing).toMatchObject({ missingCount: 1, cleanupSafe: false });

    const ambiguous = reconcileInboxTracks(
      [local("Intro", 4), local("Intro", 5)],
      [release("Intro", 1)],
    );
    expect(ambiguous).toMatchObject({ ambiguousCount: 1, cleanupSafe: false });
  });

  it("normalizes edition notes and leading articles without erasing the title", () => {
    expect(normalizeInboxTrackTitle("The Job Centre (Remastered Version)")).toBe("job centre");
  });
});
