import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as library from "../../library";
import * as musicbrainz from "../../musicbrainz";
import { useArtistNavigation } from "./useArtistNavigation";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("shows independent artist intelligence promptly and ignores an earlier artist response", async () => {
  const [older, newer, intelligence] = await Promise.all([
    library.loadArtistDetail("M83"), library.loadArtistDetail("Coldplay"),
    musicbrainz.loadArtistIntelligence("Coldplay"),
  ]);
  let finishOlder!: (detail: library.ArtistDetail) => void;
  const pendingOlder = new Promise<library.ArtistDetail>((resolve) => { finishOlder = resolve; });
  let finishNewer!: (detail: library.ArtistDetail) => void;
  const pendingNewer = new Promise<library.ArtistDetail>((resolve) => { finishNewer = resolve; });
  vi.spyOn(library, "loadArtistDetail").mockImplementation((artist) => artist === "M83" ? pendingOlder : pendingNewer);
  vi.spyOn(musicbrainz, "loadArtistIntelligence").mockImplementation(async (artist) => ({ ...intelligence, artist }));
  const show = vi.fn();
  const clear = vi.fn();
  const { result } = renderHook(() => useArtistNavigation({ onShowArtistInspector: show, onClearCurationError: clear }));

  act(() => result.current.openArtistInspector("M83"));
  await act(async () => { result.current.openArtistInspector("Coldplay"); });
  expect(result.current.artistDetail).toBeNull();
  expect(result.current.artistIntelligence?.artist).toBe("Coldplay");
  expect(result.current.artistWorldState).toBe("loading");
  await act(async () => { finishNewer(newer); });
  await act(async () => { finishOlder(older); });
  expect(result.current.inspectorArtistName).toBe("Coldplay");
  expect(result.current.artistDetail?.artist.name).toBe("Coldplay");
  expect(result.current.artistIntelligence?.artist).toBe("Coldplay");
  expect(result.current.artistWorldState).toBe("ready");
  expect(show).toHaveBeenCalledTimes(2);
  expect(clear).toHaveBeenCalledTimes(2);
});
