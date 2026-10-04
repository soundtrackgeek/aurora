import { createContext, useContext, useSyncExternalStore } from "react";
import type { AppState, AppStore } from "./appStore";

export const StoreContext = createContext<AppStore | null>(null);

export function useAppStore() {
  const store = useContext(StoreContext);
  if (!store) throw new Error("Aurora domain must be mounted inside AppStoreProvider");
  return store;
}

/** Subscribe to a stable field, so unrelated shared updates do not render this consumer. */
export function useAppSlice<K extends keyof AppState>(key: K): AppState[K] {
  const store = useAppStore();
  return useSyncExternalStore(store.subscribe, () => store.getSnapshot()[key]);
}
