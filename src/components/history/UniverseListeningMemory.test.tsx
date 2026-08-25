import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HistoryPage } from "../../history";
import type { Track } from "../../library";
import { UniverseListeningMemory } from "./UniverseListeningMemory";

afterEach(cleanup);

const track: Track = {
  id: "1",
  trackKey: "preview:1",
  albumId: "preview-score-rocky",
  title: "Living in America",
  artist: "Various Artists",
  displayArtist: "James Brown",
  album: "Rocky IV",
  releaseYear: 1985,
  rating: null,
  loved: false,
  loveState: "neutral",
  tagSyncState: null,
  canUndoTagEdit: false,
  durationSeconds: 283,
  genre: "Soundtrack",
  playCount: null,
};

function page(resolvedTrack: Track | null): HistoryPage {
  return {
    items: [{
      sessionId: "session-1",
      trackKey: track.trackKey,
      title: track.title,
      artist: "Various Artists",
      album: track.album,
      genre: track.genre,
      durationSeconds: track.durationSeconds,
      deviceId: "desktop",
      deviceName: "Desktop",
      startedAtMs: Date.now(),
      endedAtMs: Date.now(),
      listenedSeconds: 90,
      registeredPlay: true,
      registeredAtMs: Date.now(),
      outcome: "completed",
      track: resolvedTrack,
    }],
    summary: { sessions: 1, plays: 1, skips: 0, uniqueTracks: 1, listenedSeconds: 90 },
    topTracks: [],
    devices: [{ deviceId: "desktop", deviceName: "Desktop", sessions: 1, lastListenedAtMs: Date.now(), isThisDevice: true }],
    nextCursor: null,
    playThresholdSeconds: 30,
    syncState: "synced",
    syncMessage: "Synced",
  };
}

describe("UniverseListeningMemory", () => {
  it("shows the track artist, album, and resolved album cover for the last-heard song", () => {
    render(<UniverseListeningMemory page={page(track)} onOpenHistory={vi.fn()} />);

    expect(screen.getByText("James Brown · Rocky IV")).toBeInTheDocument();
    expect(screen.queryByText("Various Artists · Rocky IV")).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Rocky IV cover" })).toHaveAttribute(
      "src",
      "/__aurora-preview-cover/preview-score-rocky?size=64",
    );
  });

  it("keeps historical artist and album metadata when the catalog track is unavailable", () => {
    render(<UniverseListeningMemory page={page(null)} onOpenHistory={vi.fn()} />);

    expect(screen.getByText("Various Artists · Rocky IV")).toBeInTheDocument();
  });
});
