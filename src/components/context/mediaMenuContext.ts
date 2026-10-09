import { createContext, type KeyboardEvent, type MouseEvent } from "react";
import type { Track } from "../../library";
import type { PlaylistSelection } from "../playlists/playlistAuthoringContext";

export type MediaSelection = PlaylistSelection & {
  artistName?: string;
  artistOnly?: boolean;
  loadTrack?: () => Promise<Track>;
};
export type MediaMenuRequest = { selection: MediaSelection; x: number; y: number; anchor: HTMLElement };
export const MediaMenuContext = createContext<((request: MediaMenuRequest) => void) | null>(null);

export function mediaMenuHandlers(open: ((request: MediaMenuRequest) => void) | null, selection: MediaSelection) {
  function show(event: MouseEvent<HTMLElement> | KeyboardEvent<HTMLElement>, pointer: boolean) {
    if (!open) return;
    event.preventDefault();
    event.stopPropagation();
    const anchor = event.currentTarget;
    const bounds = anchor.getBoundingClientRect();
    open({ selection, anchor, x: pointer ? (event as MouseEvent).clientX : bounds.left + 12, y: pointer ? (event as MouseEvent).clientY : bounds.top + Math.min(32, bounds.height) });
  }
  return {
    onContextMenu: (event: MouseEvent<HTMLElement>) => show(event, true),
    onKeyDown: (event: KeyboardEvent<HTMLElement>) => {
      if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) show(event, false);
    },
  };
}
