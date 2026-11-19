import { describe, expect, it } from "vitest";
import { matchGenreSuggestions } from "./genreMatch";

const genres = ["Alternative Dance", "Alternative Rock", "Dance", "Dance-Pop", "Dancehall", "Electronic", "Indie Dance", "Música Popular Brasileira", "Rock"];

describe("matchGenreSuggestions", () => {
  it("matches the start of each word in order", () => {
    expect(matchGenreSuggestions("alt dan", genres)).toEqual(["Alternative Dance"]);
    expect(matchGenreSuggestions("alt r", genres)).toEqual(["Alternative Rock"]);
    expect(matchGenreSuggestions("dan alt", genres)).toEqual([]);
  });

  it("ranks exact and leading matches before later words and substrings", () => {
    expect(matchGenreSuggestions("dance", genres)).toEqual(["Dance", "Dance-Pop", "Dancehall", "Indie Dance", "Alternative Dance"]);
    expect(matchGenreSuggestions("ance", genres)).toEqual(["Dance", "Dance-Pop", "Dancehall", "Indie Dance", "Alternative Dance"]);
  });

  it("ignores case, accents and word separators", () => {
    expect(matchGenreSuggestions("musica pop", genres)).toEqual(["Música Popular Brasileira"]);
    expect(matchGenreSuggestions("dance pop", genres)).toEqual(["Dance-Pop"]);
    expect(matchGenreSuggestions("   ", genres)).toEqual([]);
  });
});
