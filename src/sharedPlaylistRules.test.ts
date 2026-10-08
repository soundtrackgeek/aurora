import { expect, it } from "vitest";
import { defaultExplorerFilters } from "./viewPreferences";
import { sharedRequestFromExplorer } from "./sharedPlaylistRules";

it("converts exact shared rules without changing rating units or album picker buckets", () => {
  const request = sharedRequestFromExplorer("albums", { ...defaultExplorerFilters, query:'aartist:"Pet Shop Boys" AND year:1984..1988', rating:4.5, genre:"Synthpop", sort:"releaseYearDesc" });
  expect(request).toMatchObject({view:"albums",sort:{field:"releaseYear",direction:"desc"},filters:{albumArtist:{operator:"equals",value:"Pet Shop Boys"},genres:["Synthpop"],yearFrom:1984,yearTo:1988,albumRatingMin:85,albumRatingMax:94}});
  expect(sharedRequestFromExplorer("tracks", {...defaultExplorerFilters,rating:4.5}).filters).toMatchObject({trackRatingMin:4.5,trackRatingMax:4.5});
});

it("rejects conditions that would silently change companion semantics", () => {
  for (const query of ['genre:"scores"','artist:Pet Shop Boys','year:1990..1980','year:1980 AND year:1990','aartist:"A" OR aartist:"B"','plays:0']) {
    expect(()=>sharedRequestFromExplorer("tracks",{...defaultExplorerFilters,query})).toThrow();
  }
  expect(()=>sharedRequestFromExplorer("tracks",{...defaultExplorerFilters,query:'aartist:"A"',artist:"B"})).toThrow();
  expect(()=>sharedRequestFromExplorer("tracks",{...defaultExplorerFilters,rating:"unrated"})).toThrow();
});
