import { afterEach, describe, expect, it } from "vitest";
import {
  changesFolderNameTags,
  readAutoRenamePreference,
  renameSummary,
  writeAutoRenamePreference,
} from "./libraryAutoTag";
import type { EditableTagValues } from "./tags";

function values(overrides: Partial<EditableTagValues> = {}): EditableTagValues {
  return {
    albumArtist: "Andrew Powell",
    artist: "Andrew Powell",
    album: "Ladyhawke",
    title: "Main Title",
    genre: "Soundtrack",
    publisher: "Atlantic",
    rating: null,
    year: 1978,
    releaseYear: 1985,
    trackNumber: 1,
    trackTotal: 11,
    discNumber: 1,
    discTotal: 1,
    ...overrides,
  };
}

afterEach(() => {
  try { window.localStorage.clear(); } catch { /* storage may be unavailable */ }
});

describe("changesFolderNameTags", () => {
  it("renames for Album artist, Album or Year changes", () => {
    expect(changesFolderNameTags([values()], ["year"], values({ year: 1976 }))).toBe(true);
    expect(changesFolderNameTags([values()], ["album"], values({ album: "Ladyhawke (OST)" }))).toBe(true);
    expect(changesFolderNameTags([values()], ["albumArtist"], values({ albumArtist: "Alan Parsons" }))).toBe(true);
  });

  it("ignores Release year, Genre and the other fields", () => {
    expect(changesFolderNameTags([values()], ["releaseYear", "genre", "publisher"], values({ releaseYear: 2000, genre: "Score", publisher: "GNP" }))).toBe(false);
  });

  it("ignores a name field saved with the value every track already has", () => {
    expect(changesFolderNameTags([values(), values()], ["year", "album"], values({ album: " Ladyhawke " }))).toBe(false);
    expect(changesFolderNameTags([values(), values({ year: 1976 })], ["year"], values({ year: 1976 }))).toBe(true);
  });
});

describe("auto rename preference", () => {
  it("is on until the user turns it off", () => {
    expect(readAutoRenamePreference()).toBe(true);
    writeAutoRenamePreference(false);
    expect(readAutoRenamePreference()).toBe(false);
    writeAutoRenamePreference(true);
    expect(readAutoRenamePreference()).toBe(true);
  });
});

describe("renameSummary", () => {
  const base = { albumId: "a", sourcePath: "D:\\Music\\Old", destinationPath: "D:\\Music\\Andrew Powell - Ladyhawke (1976)", warnings: [] };

  it("names the new folder and the renamed tracks", () => {
    expect(renameSummary({ ...base, folderRenamed: true, renamedFiles: 11 }))
      .toBe("Renamed the folder to Andrew Powell - Ladyhawke (1976) and 11 tracks.");
    expect(renameSummary({ ...base, folderRenamed: false, renamedFiles: 1 })).toBe("Renamed 1 track.");
  });

  it("says nothing when the names already matched, and keeps warnings", () => {
    expect(renameSummary({ ...base, folderRenamed: false, renamedFiles: 0 })).toBeNull();
    expect(renameSummary({ ...base, folderRenamed: true, renamedFiles: 2, warnings: ["Play history could not follow."] }))
      .toBe("Renamed the folder to Andrew Powell - Ladyhawke (1976) and 2 tracks. Play history could not follow.");
  });
});
