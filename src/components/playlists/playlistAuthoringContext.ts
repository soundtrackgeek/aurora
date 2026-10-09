import { createContext } from "react";
import type { Track } from "../../library";
export type PlaylistSelection = { tracks?: readonly Track[]; albumIds?: readonly string[]; label: string };
export const PlaylistAuthoringContext = createContext<((selection: PlaylistSelection) => void) | null>(null);
