import { useState, type ReactNode } from "react";
import { loadViewPreferences } from "../viewPreferences";
import { createAppStore, type AppStore } from "./appStore";
import { StoreContext } from "./useAppStore";

export function AppStoreProvider({ children, store }: { children: ReactNode; store?: AppStore; }) {
  const [ownedStore] = useState(() => store ?? createAppStore(loadViewPreferences().activeNav));
  return <StoreContext value={ownedStore}>{children}</StoreContext>;
}
