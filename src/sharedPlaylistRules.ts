import type { ExplorerFilters, ExplorerView } from "./components/explorer/DeepExplorer";
import type { SharedPlaylistRequest } from "./playlists";

export const emptySharedRequest = (): SharedPlaylistRequest => ({ view: "tracks", searchText: "", filters: {}, sort: { field: "title", direction: "asc" }, offset: 0, limit: 100 });

/** Convert only rules with equivalent Music Library meanings. Unsupported syntax is never silently flattened. */
export function sharedRequestFromExplorer(view: ExplorerView, filters: ExplorerFilters): SharedPlaylistRequest {
  if (view === "artists") throw new Error("Create playlists from Songs or Albums.");
  const result = emptySharedRequest();
  result.view = view;
  const f = result.filters;
  if (filters.query.trim()) {
    if (filters.query.includes('""') || (filters.query.match(/"/g)?.length??0)%2) throw new Error("Quoted escapes need an Aurora saved view.");
    const terms = filters.query.split(/,\s*|\s+AND\s+/).map(term=>term.trim());
    for (const term of terms) {
      const match = /^(title|album|aartist|artist|publisher|genre|year|ryear)(:|=)(.+)$/i.exec(term);
      if (!match || /[()]/.test(term)) throw new Error("This query needs Aurora's search language. Save it as a view, or use the shared rule fields to create a Smart playlist.");
      const [, field, , raw] = match;
      const value = raw.replace(/^"|"$/g, "");
      if (["year", "ryear"].includes(field.toLowerCase())) {
        const bounds = value.split("..");
        if (bounds.length > 2 || bounds.every(v => !v) || bounds.some(v => v && !/^\d{4}$/.test(v))) throw new Error("Use an ordered year or year range.");
        const prefix = field.toLowerCase() === "year" ? "year" : "releaseYear";
        if (f[`${prefix}From`] !== undefined || f[`${prefix}To`] !== undefined) throw new Error("Repeated year fields need a saved view.");
        if (bounds[0] && bounds[1] && Number(bounds[0]) > Number(bounds[1])) throw new Error("Use an ordered year range.");
        f[`${prefix}From`] = bounds[0] ? Number(bounds[0]) : null;
        f[`${prefix}To`] = (bounds[1] ?? bounds[0]) ? Number(bounds[1] ?? bounds[0]) : null;
      } else {
        if (!raw.startsWith('"') || !raw.endsWith('"') || raw.slice(1,-1).includes('"')) throw new Error("Compound and prefix searches need Aurora's search language. Use exact quoted fields or save a view.");
        if (field.toLowerCase() === "genre") {
          // Aurora contains/exact genre terms and ML's exact genre picker differ.
          if (f.genres) throw new Error("Combine genre rules in the shared editor.");
          if (/^scores?$/i.test(value)) throw new Error("Music Library expands score genre groups. Save an exact score query as a view.");
          f.genres = [value];
        } else {
          const keys: Record<string, string> = { title: "trackTitle", album: "albumTitle", aartist: "albumArtist", artist: "displayArtist", publisher: "publisher" };
          const key = keys[field.toLowerCase()];
          if (f[key]) throw new Error("Repeated fields need Aurora's search language. Save a view instead.");
          f[key] = { operator: "equals", value };
        }
      }
    }
  }
  if (filters.genre) {
    if (/^scores?$/i.test(filters.genre)) throw new Error("Music Library expands score genre groups. Save this exact genre filter as a view.");
    if (f.genres && JSON.stringify(f.genres) !== JSON.stringify([filters.genre])) throw new Error("Combined genre conditions need a saved view.");
    f.genres = [filters.genre];
  }
  if (filters.artist) {
    const existing = f.albumArtist as { value: string } | undefined;
    if (existing && existing.value.trim().toLowerCase() !== filters.artist.trim().toLowerCase()) throw new Error("Combined artist conditions need a saved view.");
    f.albumArtist = { operator: "equals", value: filters.artist };
  }
  if (filters.rating === "unrated") throw new Error("Aurora's unrated filter includes zero ratings. Preserve it as a saved view.");
  else if (filters.rating !== "all") {
    // Album picker rounds to the nearest half star, unlike exact song ratings.
    f[view === "albums" ? "albumRatingMin" : "trackRatingMin"] = view === "albums" ? filters.rating*20-5 : filters.rating;
    f[view === "albums" ? "albumRatingMax" : "trackRatingMax"] = view === "albums" ? Math.min(100, filters.rating*20+4) : filters.rating;
  }
  if (filters.love === "loved") f.lovedTracksMin = 1;
  else if (filters.love !== "all") throw new Error("Neutral and banned filters need an Aurora saved view.");
  const prefix = filters.yearBasis === "release" ? "releaseYear" : "year";
  if (filters.yearFrom !== null || filters.yearTo !== null) {
    if (f[`${prefix}From`] !== undefined || f[`${prefix}To`] !== undefined) throw new Error("Combined year ranges need a saved view.");
    f[`${prefix}From`] = filters.yearFrom; f[`${prefix}To`] = filters.yearTo;
  }
  if (filters.yearMissing) throw new Error("Missing-year rules need an Aurora saved view.");
  const sort = filters.sort.replace(/Asc|Desc$/, "");
  const fields: Record<string, string> = { newest:"added",oldest:"added",releaseYear:"releaseYear", title: view === "albums" ? "album" : "title", artist: "artist", album: "album", year: "year", rating: view === "albums" ? "albumRating" : "trackRating" };
  if (!fields[sort]) throw new Error("Choose Title, Artist, Album, Year, or Rating sorting for a shared Smart playlist.");
  result.sort = { field: fields[sort], direction: filters.sort.endsWith("Desc") || filters.sort === "newest" ? "desc" : "asc" };
  return result;
}
