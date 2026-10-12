import { createContext } from "react";
import type { ShortcutRatingSave } from "../../shortcuts";

export const ShortcutRatingSaveContext = createContext<Readonly<Record<string, ShortcutRatingSave>>>({});
