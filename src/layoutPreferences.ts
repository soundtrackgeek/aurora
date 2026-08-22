export type LeftSidebarMode = "expanded" | "icons" | "collapsed";
export type RightSidebarMode = "expanded" | "collapsed";

export type LayoutPreferences = {
  leftSidebar: LeftSidebarMode;
  rightSidebar: RightSidebarMode;
};

type StoredLayoutPreferences = LayoutPreferences & {
  schemaVersion: 1;
};

type LayoutStorage = Pick<Storage, "getItem" | "setItem">;

const STORAGE_KEY = "aurora:layout-preferences:v1";

export const defaultLayoutPreferences: LayoutPreferences = {
  leftSidebar: "expanded",
  rightSidebar: "expanded",
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
    const parsed = JSON.parse(raw) as Partial<StoredLayoutPreferences>;
    if (
      parsed.schemaVersion !== 1
      || !leftSidebarModes.has(parsed.leftSidebar as LeftSidebarMode)
      || !rightSidebarModes.has(parsed.rightSidebar as RightSidebarMode)
    ) {
      return { ...defaultLayoutPreferences };
    }
    return {
      leftSidebar: parsed.leftSidebar as LeftSidebarMode,
      rightSidebar: parsed.rightSidebar as RightSidebarMode,
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
  const stored: StoredLayoutPreferences = { schemaVersion: 1, ...preferences };
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
