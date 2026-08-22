export type LeftSidebarMode = "expanded" | "icons" | "collapsed";
export type RightSidebarMode = "expanded" | "collapsed";

export type LayoutPreferences = {
  leftSidebar: LeftSidebarMode;
  rightSidebar: RightSidebarMode;
  libraryExpanded: boolean;
  playlistsExpanded: boolean;
};

type StoredLayoutPreferencesV2 = LayoutPreferences & {
  schemaVersion: 2;
};

type LayoutStorage = Pick<Storage, "getItem" | "setItem">;

const STORAGE_KEY = "aurora:layout-preferences:v1";

export const defaultLayoutPreferences: LayoutPreferences = {
  leftSidebar: "expanded",
  rightSidebar: "expanded",
  libraryExpanded: true,
  playlistsExpanded: true,
};

const leftSidebarModes = new Set<LeftSidebarMode>(["expanded", "icons", "collapsed"]);
const rightSidebarModes = new Set<RightSidebarMode>(["expanded", "collapsed"]);

function browserStorage(): LayoutStorage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function loadLayoutPreferences(storage: LayoutStorage | null = browserStorage()): LayoutPreferences {
  if (!storage) return { ...defaultLayoutPreferences };
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return { ...defaultLayoutPreferences };
    const parsed = JSON.parse(raw) as {
      schemaVersion?: unknown;
      leftSidebar?: unknown;
      rightSidebar?: unknown;
      libraryExpanded?: unknown;
      playlistsExpanded?: unknown;
    };
    if (
      !leftSidebarModes.has(parsed.leftSidebar as LeftSidebarMode)
      || !rightSidebarModes.has(parsed.rightSidebar as RightSidebarMode)
    ) {
      return { ...defaultLayoutPreferences };
    }
    if (parsed.schemaVersion === 1) {
      return {
        leftSidebar: parsed.leftSidebar as LeftSidebarMode,
        rightSidebar: parsed.rightSidebar as RightSidebarMode,
        libraryExpanded: defaultLayoutPreferences.libraryExpanded,
        playlistsExpanded: defaultLayoutPreferences.playlistsExpanded,
      };
    }
    if (
      parsed.schemaVersion !== 2
      || typeof parsed.libraryExpanded !== "boolean"
      || typeof parsed.playlistsExpanded !== "boolean"
    ) {
      return { ...defaultLayoutPreferences };
    }
    return {
      leftSidebar: parsed.leftSidebar as LeftSidebarMode,
      rightSidebar: parsed.rightSidebar as RightSidebarMode,
      libraryExpanded: parsed.libraryExpanded,
      playlistsExpanded: parsed.playlistsExpanded,
    };
  } catch {
    return { ...defaultLayoutPreferences };
  }
}

export function saveLayoutPreferences(
  preferences: LayoutPreferences,
  storage: LayoutStorage | null = browserStorage(),
): boolean {
  if (!storage) return false;
  const stored: StoredLayoutPreferencesV2 = { schemaVersion: 2, ...preferences };
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(stored));
    return true;
  } catch {
    return false;
  }
}

export function nextLeftSidebarMode(mode: LeftSidebarMode): LeftSidebarMode {
  if (mode === "expanded") return "icons";
  if (mode === "icons") return "collapsed";
  return "expanded";
}
