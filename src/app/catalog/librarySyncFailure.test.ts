import { describe, expect, it } from "vitest";
import { cleanLibrarySyncError, describeLibrarySyncFailure } from "./librarySyncFailure";

const scores = String.raw`G:\_BACKUP\SCORES\Don Davis - The Matrix Reloaded (2003)`;

describe("describeLibrarySyncFailure", () => {
  it("names the track whose rating Music Library cannot read", () => {
    const failure = describeLibrarySyncFailure(
      String.raw`Music Library could not complete syncExistingFolders: \\?\G:\_BACKUP\SCORES\Don Davis - The Matrix Reloaded (2003)\1-02 - Don Davis - Enter The Neb.mp3 has unsupported MusicBee rating byte 204 (validationFailed)`,
      scores,
    );
    expect(failure.reason).toContain("rating byte 204");
    expect(failure.file).toBe("1-02 - Don Davis - Enter The Neb.mp3");
    expect(failure.fix).toMatch(/rate the track again/i);
  });

  it("names the stray file to move out of the album folder", () => {
    const folder = String.raw`D:\MUSIC\Aretha Franklin - Sparkle (1976)`;
    const failure = describeLibrarySyncFailure(
      String.raw`Music Library could not complete syncExistingFolders: The selected folder contains an unsupported non-MP3 file: \\?\D:\MUSIC\Aretha Franklin - Sparkle (1976)\Aretha Franklin - Sparkle_files\album.css. Keep only MP3 audio and recognized artwork, playlist, checksum, or liner-note sidecars in the album folder (validationFailed)`,
      folder,
    );
    expect(failure.file).toBe(String.raw`Aretha Franklin - Sparkle_files\album.css`);
    expect(failure.fix).toMatch(/move that file out/i);
  });

  it("asks for an import when the album is missing from the catalog", () => {
    const folder = String.raw`d:\MUSIC\David Archuleta - Christmas From The Heart (2009)`;
    const failure = describeLibrarySyncFailure(
      String.raw`Music Library could not complete syncExistingFolders: The requested folder contains an MP3 that is not represented in the active Music Library catalog: D:\MUSIC\David Archuleta - Christmas From The Heart (2009)\01 - David Archuleta - Joy To The World.mp3 (validationFailed)`,
      folder,
    );
    expect(failure.reason).toMatch(/not in the Music Library catalog/);
    expect(failure.file).toBe("01 - David Archuleta - Joy To The World.mp3");
  });

  it("points a reviewed-reconciliation block at Album Artist and a reviewed import", () => {
    const failure = describeLibrarySyncFailure(
      String.raw`Music Library could not complete syncExistingFolders: Could not sync \\?\G:\_BACKUP\SCORES\Jake Monaco - Playing It Cool (2014): Aurora background sync requires reviewed reconciliation: the selected album could not be safely updated with an album-sized metadata sync. (importFailed)`,
      String.raw`G:\_BACKUP\SCORES\Jake Monaco - Playing It Cool (2014)`,
    );
    expect(failure.fix).toMatch(/Album Artist/);
    expect(failure.fix).toMatch(/reviewed import/);
    expect(failure.file).toBeUndefined();
  });

  it("names tracks missing from the folder instead of guessing at Album Artist", () => {
    const error = "Music Library could not complete syncExistingFolders: Could not sync D:\\Music\\Voices Carry: Aurora background sync requires reviewed reconciliation (importFailed)";
    const one = describeLibrarySyncFailure(error, "D:\\Music\\Voices Carry", ["Are You Serious?"]);
    expect(one.reason).toBe(`"Are You Serious?" is missing from this folder, but Music Library still lists it.`);
    expect(one.fix).toMatch(/Move to Inbox/);
    expect(one.fix).not.toMatch(/Album Artist/);

    const many = describeLibrarySyncFailure(error, "D:\\Music\\A", ["One", "Two", "Three", "Four"]);
    expect(many.reason).toBe(`"One", "Two", "Three" and 1 more are missing from this folder, but Music Library still lists them.`);
    expect(describeLibrarySyncFailure(error, "D:\\Music\\A", ["One", "Two"]).reason).toMatch(/^"One" and "Two" are missing/);
    // Before the first attempt there is nothing to explain yet.
    expect(describeLibrarySyncFailure(null, "D:\\Music\\A", ["One"]).reason).toMatch(/first sync/);
  });

  it("tells the user an interrupted sync needs nothing from them", () => {
    const failure = describeLibrarySyncFailure(
      "Music Library could not complete syncExistingFolders: Could not sync D:\\Music\\A: Aurora background sync was interrupted: the album's MP3s changed (importFailed)",
      "D:\\Music\\A",
    );
    expect(failure.fix).toMatch(/^Nothing to do/);
  });

  it("covers a folder that has not been tried yet and unknown errors", () => {
    expect(describeLibrarySyncFailure(null, "D:\\Music\\A").reason).toMatch(/first sync/);
    const unknown = describeLibrarySyncFailure("Music Library could not complete syncExistingFolders: database is locked (busy)", "D:\\Music\\A");
    expect(unknown.reason).toBe("database is locked");
  });
});

describe("cleanLibrarySyncError", () => {
  it("drops the bridge wrapper, folder prefix and status code", () => {
    expect(cleanLibrarySyncError(
      "Music Library could not complete syncExistingFolders: Could not sync \\\\?\\D:\\Music\\A (1999): Aurora background sync requires reviewed reconciliation (importFailed)",
    )).toBe("Aurora background sync requires reviewed reconciliation");
  });
});
