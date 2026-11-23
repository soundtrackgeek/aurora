import { expect, it } from "vitest";
import { searchSuggestions, searchSyntaxHint } from "./searchHelp";

it("offers only fields supported by the current view", () => {
  const albums = searchSuggestions("", "albums").fields.map(({ name }) => name);
  expect(albums).toEqual(expect.arrayContaining(["sbb", "vibe", "sound", "sort", "group", "genres"]));
  expect(albums).not.toContain("ti");
  expect(searchSuggestions("", "artists").fields.map(({ name }) => name)).not.toContain("group");
});

it("completes the current clause and preserves earlier filters", () => {
  expect(searchSuggestions("year:1985 AND vi", "albums").matches).toContainEqual({
    label: "vibe:", query: "year:1985 AND vibe:",
  });
  expect(searchSuggestions('title:"AND OR NOT" NOT sound=ch', "tracks").matches).toContainEqual({
    label: "choir", query: 'title:"AND OR NOT" NOT sound=choir',
  });
  expect(searchSuggestions("country:France OR uni", "albums").matches).toContainEqual({
    label: "United Kingdom", query: 'country:France OR "United Kingdom"',
  });
});

it("flags common syntax slips without treating quoted operators as syntax", () => {
  expect(searchSyntaxHint('vibe:"dark ambient')).toMatch(/quote/u);
  expect(searchSyntaxHint("year:1985 genre:scores")).toMatch(/Add AND/u);
  expect(searchSyntaxHint('vibe:"dark ambient AND drone" sort:vibe')).toBeNull();
});
