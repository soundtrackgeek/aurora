export const textSizeOptions = [
  { value: "compact", label: "Compact", detail: "Dense" },
  { value: "comfortable", label: "Comfortable", detail: "Recommended · +2 px" },
  { value: "large", label: "Large", detail: "+4 px" },
  { value: "extra-large", label: "Extra large", detail: "+6 px" },
  { value: "maximum", label: "Maximum", detail: "+8 px" },
] as const;

export const coverSizeOptions = [
  { value: "compact", label: "Compact", detail: "80%" },
  { value: "standard", label: "Standard", detail: "100%" },
  { value: "large", label: "Large", detail: "125%" },
  { value: "extra-large", label: "Extra large", detail: "150%" },
] as const;

export type TextSize = typeof textSizeOptions[number]["value"];
export type CoverSize = typeof coverSizeOptions[number]["value"];

export const displayViews = [
  { id: "universe", label: "Universe", supportsCovers: true },
  { id: "observatory", label: "Observatory", supportsCovers: false },
  { id: "songs", label: "Songs", supportsCovers: true },
  { id: "albums", label: "Albums", supportsCovers: true },
  { id: "artists", label: "Artists", supportsCovers: true },
  { id: "publishers", label: "Publishers", supportsCovers: true },
  { id: "genres", label: "Genres", supportsCovers: true },
  { id: "years", label: "Years", supportsCovers: true },
  { id: "ratings", label: "Ratings", supportsCovers: true },
  { id: "tags", label: "Tags", supportsCovers: true },
  { id: "charts", label: "Charts", supportsCovers: true },
  { id: "history", label: "History", supportsCovers: true },
] as const;

export type DisplayViewKey = typeof displayViews[number]["id"];

export type DisplayPreferenceOverride = {
  textSize: TextSize | null;
  coverSize: CoverSize | null;
};

export type DisplayPreferences = {
  global: {
    textSize: TextSize;
    coverSize: CoverSize;
  };
  views: Record<DisplayViewKey, DisplayPreferenceOverride>;
};

type StoredDisplayPreferencesV1 = DisplayPreferences & {
  schemaVersion: 1;
};

type DisplayStorage = Pick<Storage, "getItem" | "setItem">;

const STORAGE_KEY = "aurora:display-preferences:v1";
const textSizes = new Set<TextSize>(textSizeOptions.map(({ value }) => value));
const coverSizes = new Set<CoverSize>(coverSizeOptions.map(({ value }) => value));

function createViewDefaults(): Record<DisplayViewKey, DisplayPreferenceOverride> {
  return Object.fromEntries(displayViews.map(({ id }) => [id, {
    textSize: id === "charts" ? "large" : null,
    coverSize: null,
  }])) as Record<DisplayViewKey, DisplayPreferenceOverride>;
}

export function createDefaultDisplayPreferences(): DisplayPreferences {
  return {
    global: {
      textSize: "comfortable",
      coverSize: "standard",
    },
    views: createViewDefaults(),
  };
}

export const defaultDisplayPreferences = createDefaultDisplayPreferences();

function browserStorage(): DisplayStorage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function optionalTextSize(value: unknown, fallback: TextSize | null): TextSize | null {
  return value === null || textSizes.has(value as TextSize) ? value as TextSize | null : fallback;
}

function optionalCoverSize(value: unknown, fallback: CoverSize | null): CoverSize | null {
  return value === null || coverSizes.has(value as CoverSize) ? value as CoverSize | null : fallback;
}

export function loadDisplayPreferences(storage: DisplayStorage | null = browserStorage()): DisplayPreferences {
  const fallback = createDefaultDisplayPreferences();
  if (!storage) return fallback;
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<StoredDisplayPreferencesV1>;
    if (
      parsed.schemaVersion !== 1
      || !parsed.global
      || !textSizes.has(parsed.global.textSize as TextSize)
      || !coverSizes.has(parsed.global.coverSize as CoverSize)
    ) return fallback;

    const storedViews = parsed.views as Partial<Record<DisplayViewKey, Partial<DisplayPreferenceOverride>>> | undefined;
    const views = createViewDefaults();
    for (const { id } of displayViews) {
      const stored = storedViews?.[id];
      if (!stored) continue;
      views[id] = {
        textSize: optionalTextSize(stored.textSize, views[id].textSize),
        coverSize: optionalCoverSize(stored.coverSize, views[id].coverSize),
      };
    }
    return {
      global: {
        textSize: parsed.global.textSize as TextSize,
        coverSize: parsed.global.coverSize as CoverSize,
      },
      views,
    };
  } catch {
    return fallback;
  }
}

export function saveDisplayPreferences(
  preferences: DisplayPreferences,
  storage: DisplayStorage | null = browserStorage(),
): boolean {
  if (!storage) return false;
  const stored: StoredDisplayPreferencesV1 = { schemaVersion: 1, ...preferences };
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(stored));
    return true;
  } catch {
    return false;
  }
}

export function effectiveDisplayPreferences(preferences: DisplayPreferences, view: DisplayViewKey) {
  return {
    textSize: preferences.views[view].textSize ?? preferences.global.textSize,
    coverSize: preferences.views[view].coverSize ?? preferences.global.coverSize,
  };
}
