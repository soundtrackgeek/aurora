import { expect, it } from "vitest";
import { shufflePlaylistTracks } from "./playlistOrder";

it("shuffles a long playlist without losing, repeating, or changing its saved order", () => {
  const savedOrder = Array.from({ length: 205 }, (_, index) => index);
  const shuffled = shufflePlaylistTracks(savedOrder, () => 0);

  expect(shuffled).toHaveLength(savedOrder.length);
  expect(new Set(shuffled).size).toBe(savedOrder.length);
  expect([...shuffled].sort((a, b) => a - b)).toEqual(savedOrder);
  expect(shuffled).not.toEqual(savedOrder);
  expect(savedOrder).toEqual(Array.from({ length: 205 }, (_, index) => index));
});
