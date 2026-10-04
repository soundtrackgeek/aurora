import type { SetStateAction } from "react";
import type { SidebarDestination } from "../components/navigation/SidebarNavigation";
import type { Track } from "../library";

export type AppState = {
  activeDestination: SidebarDestination;
  selectedTrack: Track | null;
  /** Local invalidation revision, including tag syncs without a catalog import. */
  catalogRevision: number;
};

/** One store per mounted app: no state leaks between windows or test renders. */
export function createAppStore(activeDestination: SidebarDestination) {
  let state: AppState = { activeDestination, selectedTrack: null, catalogRevision: 0 };
  const listeners = new Set<() => void>();
  function setSlice<K extends keyof AppState>(key: K, update: SetStateAction<AppState[K]>) {
    const value = typeof update === "function"
      ? (update as (previous: AppState[K]) => AppState[K])(state[key])
      : update;
    if (Object.is(value, state[key])) return;
    state = { ...state, [key]: value };
    for (const listener of listeners) listener();
  }
  return {
    getSnapshot: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    setActiveDestination: (value: SetStateAction<SidebarDestination>) => setSlice("activeDestination", value),
    setSelectedTrack: (value: SetStateAction<Track | null>) => setSlice("selectedTrack", value),
    setCatalogRevision: (value: SetStateAction<number>) => setSlice("catalogRevision", value),
  };
}

export type AppStore = ReturnType<typeof createAppStore>;
