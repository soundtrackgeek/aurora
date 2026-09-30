import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { refreshAlbumArtwork, refreshLibraryArtwork } from "./albumArtwork";
import { Artwork } from "./components/Artwork";
import type { Track } from "./library";

vi.mock("./library", () => ({
  albumCoverUrl: (id: string, size: number) => `/album/${id}?size=${size}`,
}));
afterEach(cleanup);

function Cover({ albumId }: { albumId: string }) {
  const track = { albumId, album: albumId, artist: "Various Artists" } as Track;
  return <Artwork track={track} decorative={false} />;
}

it("retries failed artwork across mounted views after replacement without changing other albums", () => {
  render(<><Cover albumId="cool-as-ice" /><Cover albumId="cool-as-ice" /><Cover albumId="other" /></>);
  const before = screen.getAllByAltText("cool-as-ice cover")[0].getAttribute("src");
  const otherBefore = screen.getByAltText("other cover").getAttribute("src");
  screen.getAllByAltText("cool-as-ice cover").forEach((image) => fireEvent.error(image));
  expect(screen.queryAllByAltText("cool-as-ice cover")).toHaveLength(0);
  act(() => refreshAlbumArtwork("cool-as-ice"));
  for (const image of screen.getAllByAltText("cool-as-ice cover")) {
    expect(image.getAttribute("src")).not.toBe(before);
  }
  expect(screen.getByAltText("other cover")).toHaveAttribute("src", otherBefore);
});

it("refreshes cached and failed covers across views after batch artwork completes", () => {
  render(<><Cover albumId="batch-one" /><Cover albumId="batch-two" /></>);
  const oldSource = screen.getByAltText("batch-one cover").getAttribute("src");
  fireEvent.error(screen.getByAltText("batch-two cover"));
  act(() => refreshLibraryArtwork());
  expect(screen.getByAltText("batch-one cover").getAttribute("src")).not.toBe(oldSource);
  expect(screen.getByAltText("batch-two cover")).toBeInTheDocument();
});
