export interface WorkspaceCheckpoint {
  scroll: Record<string, number>;
  explorerKey: string | null;
  loaded: number;
  trackKey: string | null;
}

const key = "aurora:workspace:v1";

export function loadWorkspaceCheckpoint(): WorkspaceCheckpoint {
  const fallback: WorkspaceCheckpoint = { scroll: {}, explorerKey: null, loaded: 0, trackKey: null };
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? "null");
    if (!value || typeof value !== "object") return fallback;
    return {
      scroll: Object.fromEntries(Object.entries(value.scroll ?? {}).filter((entry): entry is [string, number] =>
        typeof entry[1] === "number" && Number.isFinite(entry[1]) && entry[1] >= 0)),
      explorerKey: typeof value.explorerKey === "string" ? value.explorerKey : null,
      loaded: Number.isSafeInteger(value.loaded) && value.loaded >= 0 ? value.loaded : 0,
      trackKey: typeof value.trackKey === "string" ? value.trackKey : null,
    };
  } catch { return fallback; }
}

export function saveWorkspaceCheckpoint(value: WorkspaceCheckpoint) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* Storage may be unavailable. */ }
}

export async function loadWorkspacePages<Page extends { tracks: unknown[]; albums: unknown[]; artists: unknown[]; nextCursor: unknown }>(
  load: (cursor?: NonNullable<Page["nextCursor"]>) => Promise<Page>,
  targetCount: number,
  cancelled: () => boolean,
): Promise<Page> {
  const page = await load();
  while (!cancelled() && page.nextCursor && page.tracks.length + page.albums.length + page.artists.length < targetCount) {
    const next = await load(page.nextCursor);
    if (next.tracks.length + next.albums.length + next.artists.length === 0) break;
    page.tracks.push(...next.tracks);
    page.albums.push(...next.albums);
    page.artists.push(...next.artists);
    page.nextCursor = next.nextCursor;
  }
  return page;
}

// Reapply after asynchronous content changes, without imposing a startup timeout.
// Explicit user scrolling takes precedence over restoration.
export function restoreWorkspaceScroll(element: HTMLElement, target: number, ready: () => boolean, done: () => void) {
  let stopped = false;
  const finish = () => {
    if (stopped) return;
    stopped = true;
    window.clearInterval(timer);
    element.removeEventListener("wheel", finish);
    element.removeEventListener("touchstart", finish);
    element.removeEventListener("pointerdown", finish);
    element.removeEventListener("keydown", onKey);
    done();
  };
  const onKey = (event: KeyboardEvent) => {
    if (["ArrowDown", "ArrowUp", "PageDown", "PageUp", "Home", "End", " "].includes(event.key)) finish();
  };
  const apply = () => {
    element.scrollTop = target;
    if (ready()) finish();
  };
  const timer = window.setInterval(apply, 50);
  element.addEventListener("wheel", finish, { passive: true });
  element.addEventListener("touchstart", finish, { passive: true });
  element.addEventListener("pointerdown", finish, { passive: true });
  element.addEventListener("keydown", onKey);
  apply();
  return finish;
}
