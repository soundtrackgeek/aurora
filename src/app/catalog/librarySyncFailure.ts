export interface LibrarySyncFailure {
  /** What went wrong, in plain words. */
  reason: string;
  /** What the user can do about it, or that nothing is needed. */
  fix: string;
  /** The file the failure names, relative to the folder when possible. */
  file?: string;
}

const BRIDGE_PREFIX = /^Music Library could not complete \w+:\s*/i;
const SYNC_PREFIX = /^Could not sync .+?:\s+(?=Aurora |The |Music Library )/i;
const ERROR_CODE = /\s*\((\w+)\)\s*$/;

function withoutVerbatimPrefix(path: string): string {
  return path.replace(/^\\\\\?\\(UNC\\)?/i, (_match, unc: string | undefined) => (unc ? "\\\\" : ""));
}

/** Shows a file relative to its album folder, so long library paths stay readable. */
function relativeFile(path: string, directory: string): string {
  const file = withoutVerbatimPrefix(path.trim());
  const folder = withoutVerbatimPrefix(directory).replace(/[\\/]+$/, "");
  if (file.toLowerCase().startsWith(`${folder.toLowerCase()}\\`)) return file.slice(folder.length + 1);
  return file;
}

/** The Music Library error without the bridge's wrapper text and status code. */
export function cleanLibrarySyncError(error: string): string {
  return error.trim().replace(BRIDGE_PREFIX, "").replace(SYNC_PREFIX, "").replace(ERROR_CODE, "").trim();
}

function quotedList(titles: string[]): string {
  const quoted = titles.slice(0, 3).map((title) => `"${title}"`);
  const more = titles.length - quoted.length;
  if (more > 0) return `${quoted.join(", ")} and ${more} more`;
  return quoted.length > 1 ? `${quoted.slice(0, -1).join(", ")} and ${quoted[quoted.length - 1]}` : quoted[0];
}

/**
 * Turns a stored Music Library sync error into a reason and a suggested fix.
 * `missingTracks` are catalog tracks whose MP3 is gone from the folder; they explain
 * any failure better than the error text, which only says the album no longer matches.
 */
export function describeLibrarySyncFailure(
  error: string | null | undefined,
  directory: string,
  missingTracks: readonly string[] = [],
): LibrarySyncFailure {
  if (!error?.trim()) {
    return { reason: "Waiting for its first sync.", fix: "Nothing to do; Aurora syncs it shortly." };
  }
  if (missingTracks.length > 0) {
    const one = missingTracks.length === 1;
    return {
      reason: `${quotedList([...missingTracks])} ${one ? "is" : "are"} missing from this folder, but Music Library still lists ${one ? "it" : "them"}.`,
      fix: `Put the ${one ? "file" : "files"} back, or use Move to Inbox on the album: Music Library drops the missing ${one ? "track" : "tracks"}, and you can add the album again from the Inbox.`,
    };
  }
  const message = cleanLibrarySyncError(error);

  const rating = /^(.+?\.mp3) has unsupported MusicBee rating byte (\d+)/i.exec(message);
  if (rating) {
    return {
      reason: `A track has a rating Music Library cannot read (rating byte ${rating[2]}).`,
      fix: "Rate the track again in Aurora or MusicBee so it gets a standard star rating.",
      file: relativeFile(rating[1], directory),
    };
  }
  const extra = /unsupported non-MP3 file:\s*(.+?)(?:\. Keep only|$)/i.exec(message);
  if (extra) {
    return {
      reason: "The album folder holds a file Music Library does not accept.",
      fix: "Move that file out of the album folder.",
      file: relativeFile(extra[1], directory),
    };
  }
  const missing = /not represented in the active Music Library catalog:\s*(.+)$/i.exec(message);
  if (missing) {
    return {
      reason: "This album is not in the Music Library catalog yet.",
      fix: "Import the folder into Music Library, for example with Add music.",
      file: relativeFile(missing[1], directory),
    };
  }
  if (/exact MP3 identity set/i.test(message)) {
    return {
      reason: "The folder's MP3s no longer match the catalog; files were added, removed or renamed.",
      fix: "Run a reviewed import of this folder in Music Library.",
    };
  }
  if (/requires reviewed reconciliation/i.test(message)) {
    return {
      reason: "Music Library cannot apply this change on its own.",
      fix: "Make sure Album Artist is the same on every track. If it already is, run a reviewed import of this folder in Music Library.",
    };
  }
  if (/background sync was interrupted/i.test(message)) {
    return {
      reason: "The album or the catalog changed while it was syncing.",
      fix: "Nothing to do; Aurora retries it shortly.",
    };
  }
  if (/must be updated \(or installed if missing\)/i.test(message)) {
    return { reason: message, fix: "Update or install Music Library; Aurora then retries." };
  }
  return { reason: message, fix: "Aurora retries it automatically; use Retry to try now." };
}

/** When the background worker tries a folder next, in words. */
export function describeNextRetry(nextRetryAtMs: number, nowMs: number): string {
  const remaining = nextRetryAtMs - nowMs;
  if (remaining <= 60_000) return "Aurora tries it again within a minute.";
  const minutes = Math.round(remaining / 60_000);
  if (minutes < 60) return `Aurora tries it again in ${minutes} minute${minutes === 1 ? "" : "s"}.`;
  const hours = Math.round(minutes / 60);
  return `Aurora tries it again in about ${hours} hour${hours === 1 ? "" : "s"}, or after a Music Library update.`;
}
