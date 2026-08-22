import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { browserPreview, type Track } from "../../library";
import type { HistoryItem, HistoryPage } from "../../history";
import { ListeningHistory } from "./ListeningHistory";

const track = browserPreview.tracks[0];

afterEach(cleanup);

function historyItem(overrides: Partial<HistoryItem> = {}): HistoryItem {
  return {
    sessionId: "session-1",
    trackKey: track.trackKey,
    title: track.title,
    artist: track.artist,
    album: track.album,
    genre: track.genre,
    durationSeconds: track.durationSeconds,
    deviceId: "device-desktop",
    deviceName: "JornComputer",
    startedAtMs: Date.now(),
    endedAtMs: Date.now(),
    listenedSeconds: 54,
    registeredPlay: true,
    registeredAtMs: Date.now(),
    outcome: "completed",
    track,
    ...overrides,
  };
}

function historyPage(items: HistoryItem[]): HistoryPage {
  return {
    items,
    summary: { sessions: 8, plays: 5, skips: 2, uniqueTracks: 4, listenedSeconds: 1_872 },
    topTracks: [{
      trackKey: track.trackKey,
      title: track.title,
      artist: track.artist,
      album: track.album,
      plays: 3,
      listenedSeconds: 720,
      lastPlayedAtMs: Date.now(),
      track,
    }],
    devices: [{ deviceId: "device-desktop", deviceName: "JornComputer", sessions: 8, lastListenedAtMs: Date.now(), isThisDevice: true }],
    nextCursor: null,
    playThresholdSeconds: 30,
    syncState: "synced",
    syncMessage: "Device histories combined.",
  };
}

function renderHistory(items: HistoryItem[] = [historyItem()]) {
  const onSaveThreshold = vi.fn();
  const onPlayTrack = vi.fn<(track: Track) => void>();
  const onSelectTrack = vi.fn<(track: Track) => void>();
  const onOutcomeChange = vi.fn();
  render(
    <ListeningHistory
      page={historyPage(items)}
      loadState="ready"
      errorMessage={null}
      search=""
      outcome="all"
      deviceId={null}
      dateRange="all"
      isLoadingMore={false}
      isSavingThreshold={false}
      thresholdMessage={null}
      onSearchChange={() => undefined}
      onOutcomeChange={onOutcomeChange}
      onDeviceChange={() => undefined}
      onDateRangeChange={() => undefined}
      onSaveThreshold={onSaveThreshold}
      onSelectTrack={onSelectTrack}
      onPlayTrack={onPlayTrack}
      onLoadMore={() => undefined}
      onRefresh={() => undefined}
    />,
  );
  return { onSaveThreshold, onPlayTrack, onSelectTrack, onOutcomeChange };
}

describe("ListeningHistory", () => {
  it("edits the played threshold and exposes the listening summary", () => {
    const { onSaveThreshold } = renderHistory();
    expect(screen.getByText("5")).toBeInTheDocument();
    expect(screen.getByText("31 min")).toBeInTheDocument();

    const seconds = screen.getByRole("spinbutton", { name: /Seconds/ });
    fireEvent.change(seconds, { target: { value: "45" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSaveThreshold).toHaveBeenCalledWith(45);
  });

  it("replays catalog tracks and disables unavailable historical files", () => {
    const unavailable = historyItem({ sessionId: "session-missing", title: "Missing track", track: null });
    const { onPlayTrack, onSelectTrack } = renderHistory([historyItem(), unavailable]);

    fireEvent.click(screen.getByRole("button", { name: `Play ${track.title} again` }));
    expect(onPlayTrack).toHaveBeenCalledWith(track);
    fireEvent.click(screen.getByRole("button", { name: `Inspect ${track.title}` }));
    expect(onSelectTrack).toHaveBeenCalledWith(track);
    expect(screen.getByRole("button", { name: "Play Missing track again" })).toBeDisabled();
  });
});
