export type TrackColumnId =
  | "title"
  | "artist"
  | "album"
  | "year"
  | "genre"
  | "publisher"
  | "charts"
  | "time"
  | "plays"
  | "rating"
  | "love";

export interface TrackColumnDefinition {
  id: TrackColumnId;
  label: string;
  /** Default width in pixels. `null` means the column takes the remaining space. */
  defaultWidth: number | null;
  minWidth: number;
  numeric?: boolean;
  defaultVisible: boolean;
}

export const trackColumnDefinitions: readonly TrackColumnDefinition[] = [
  { id: "title", label: "Title", defaultWidth: null, minWidth: 120, defaultVisible: true },
  { id: "artist", label: "Artist", defaultWidth: 170, minWidth: 60, defaultVisible: true },
  { id: "album", label: "Album", defaultWidth: 190, minWidth: 60, defaultVisible: true },
  { id: "year", label: "Year", defaultWidth: 62, minWidth: 48, numeric: true, defaultVisible: true },
  { id: "genre", label: "Genre", defaultWidth: 96, minWidth: 48, defaultVisible: true },
  { id: "publisher", label: "Publisher", defaultWidth: 120, minWidth: 48, defaultVisible: false },
  { id: "charts", label: "Charts", defaultWidth: 190, minWidth: 60, defaultVisible: false },
  { id: "time", label: "Time", defaultWidth: 66, minWidth: 48, numeric: true, defaultVisible: true },
  { id: "plays", label: "Plays", defaultWidth: 64, minWidth: 48, numeric: true, defaultVisible: true },
  { id: "rating", label: "Rating", defaultWidth: 126, minWidth: 90, defaultVisible: true },
  { id: "love", label: "Love", defaultWidth: 58, minWidth: 44, defaultVisible: true },
];

const definitionById = new Map(trackColumnDefinitions.map((column) => [column.id, column]));

export function trackColumnDefinition(id: TrackColumnId): TrackColumnDefinition {
  return definitionById.get(id)!;
}

export interface TrackColumnLayout {
  /** Every column id, in display order (hidden ones included so they keep their place). */
  order: TrackColumnId[];
  hidden: TrackColumnId[];
  widths: Partial<Record<TrackColumnId, number>>;
}

export const MAX_TRACK_COLUMN_WIDTH = 900;

export function defaultTrackColumnLayout(): TrackColumnLayout {
  return {
    order: trackColumnDefinitions.map((column) => column.id),
    hidden: trackColumnDefinitions.filter((column) => !column.defaultVisible).map((column) => column.id),
    widths: {},
  };
}

export function visibleTrackColumns(layout: TrackColumnLayout): TrackColumnId[] {
  const hidden = new Set(layout.hidden);
  return layout.order.filter((id) => !hidden.has(id));
}

export function clampTrackColumnWidth(id: TrackColumnId, width: number): number {
  return Math.round(Math.min(MAX_TRACK_COLUMN_WIDTH, Math.max(trackColumnDefinition(id).minWidth, width)));
}

export function setTrackColumnWidth(layout: TrackColumnLayout, id: TrackColumnId, width: number): TrackColumnLayout {
  return { ...layout, widths: { ...layout.widths, [id]: clampTrackColumnWidth(id, width) } };
}

/** Shows or hides a column. The last visible column cannot be hidden. */
export function toggleTrackColumn(layout: TrackColumnLayout, id: TrackColumnId): TrackColumnLayout {
  if (layout.hidden.includes(id)) {
    return { ...layout, hidden: layout.hidden.filter((candidate) => candidate !== id) };
  }
  if (visibleTrackColumns(layout).length <= 1) return layout;
  return { ...layout, hidden: [...layout.hidden, id] };
}

/**
 * Moves `id` so it lands before the visible column at `targetVisibleIndex`
 * (or after the last visible column when the index equals the visible count).
 */
export function moveTrackColumn(layout: TrackColumnLayout, id: TrackColumnId, targetVisibleIndex: number): TrackColumnLayout {
  const visible = visibleTrackColumns(layout);
  if (!visible.includes(id)) return layout;
  const anchor = visible[targetVisibleIndex];
  if (anchor === id) return layout;
  const order = layout.order.filter((candidate) => candidate !== id);
  if (anchor === undefined) {
    const others = visible.filter((candidate) => candidate !== id);
    const lastVisible = others[others.length - 1];
    const insertAt = lastVisible === undefined ? order.length : order.indexOf(lastVisible) + 1;
    order.splice(insertAt, 0, id);
  } else {
    order.splice(order.indexOf(anchor), 0, id);
  }
  if (order.every((candidate, index) => candidate === layout.order[index])) return layout;
  return { ...layout, order };
}

const STORAGE_KEY = "aurora:songs-columns:v1";
const knownIds = new Set<TrackColumnId>(trackColumnDefinitions.map((column) => column.id));

function isColumnId(value: unknown): value is TrackColumnId {
  return typeof value === "string" && knownIds.has(value as TrackColumnId);
}

/** Accepts any stored value and returns a complete, valid layout. */
export function sanitizeTrackColumnLayout(value: unknown): TrackColumnLayout {
  const fallback = defaultTrackColumnLayout();
  if (!value || typeof value !== "object") return fallback;
  const raw = value as Partial<Record<keyof TrackColumnLayout, unknown>>;
  const order: TrackColumnId[] = [];
  if (Array.isArray(raw.order)) {
    for (const id of raw.order) if (isColumnId(id) && !order.includes(id)) order.push(id);
  }
  // Columns added in later versions keep their default position relative to their neighbours.
  for (const column of trackColumnDefinitions) {
    if (order.includes(column.id)) continue;
    const defaultIndex = fallback.order.indexOf(column.id);
    const previous = fallback.order.slice(0, defaultIndex).reverse().find((id) => order.includes(id));
    order.splice(previous === undefined ? 0 : order.indexOf(previous) + 1, 0, column.id);
  }
  const hidden = Array.isArray(raw.hidden)
    ? [...new Set(raw.hidden.filter(isColumnId))]
    : fallback.hidden;
  const widths: Partial<Record<TrackColumnId, number>> = {};
  if (raw.widths && typeof raw.widths === "object") {
    for (const [id, width] of Object.entries(raw.widths as Record<string, unknown>)) {
      if (isColumnId(id) && typeof width === "number" && Number.isFinite(width)) widths[id] = clampTrackColumnWidth(id, width);
    }
  }
  const layout = { order, hidden, widths };
  return visibleTrackColumns(layout).length === 0 ? { ...layout, hidden: [] } : layout;
}

type ColumnStorage = Pick<Storage, "getItem" | "setItem">;

function browserStorage(): ColumnStorage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

export function loadTrackColumnLayout(storage: ColumnStorage | null = browserStorage()): TrackColumnLayout {
  if (!storage) return defaultTrackColumnLayout();
  try {
    const raw = storage.getItem(STORAGE_KEY);
    return raw ? sanitizeTrackColumnLayout(JSON.parse(raw)) : defaultTrackColumnLayout();
  } catch {
    return defaultTrackColumnLayout();
  }
}

export function saveTrackColumnLayout(layout: TrackColumnLayout, storage: ColumnStorage | null = browserStorage()): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    // Column layout is a convenience; a full or blocked store just means it is not remembered.
  }
}
