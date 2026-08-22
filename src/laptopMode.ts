import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";

export type LaptopSyncState =
  | "synced"
  | "pending"
  | "remoteUpdate"
  | "conflict"
  | "unavailable"
  | "preview";

export interface LaptopPathMapping {
  desktopRoot: string;
  laptopRoot: string;
  activeRoot: string;
  available: boolean;
}

export interface LaptopModeStatus {
  laptopMode: boolean;
  modeLabel: string;
  syncState: LaptopSyncState;
  message: string;
  remotePath: string;
  lastSyncedAtMs: number | null;
  mappings: LaptopPathMapping[];
  settingWarning: string | null;
}

let previewLaptopMode = false;

function previewStatus(): LaptopModeStatus {
  const mappings: Array<[string, string]> = [
    ["D:\\MUSIC", "Y:\\MUSIC"],
    ["G:\\_BACKUP\\SCORES", "V:\\_BACKUP\\SCORES"],
    ["H:\\Synthwave", "U:\\Synthwave"],
  ];
  return {
    laptopMode: previewLaptopMode,
    modeLabel: previewLaptopMode ? "Laptop Mode" : "Desktop Mode",
    syncState: "preview",
    message: "Browser preview: verified state snapshot is ready in OneDrive.",
    remotePath: "C:\\Users\\jtill\\OneDrive\\_musicbackup\\aurora-state.sqlite3",
    lastSyncedAtMs: Date.now(),
    mappings: mappings.map(([desktopRoot, laptopRoot]) => ({
      desktopRoot,
      laptopRoot,
      activeRoot: previewLaptopMode ? laptopRoot : desktopRoot,
      available: true,
    })),
    settingWarning: null,
  };
}

export async function loadLaptopModeStatus(): Promise<LaptopModeStatus> {
  if (!isTauriRuntime()) return previewStatus();
  return invoke<LaptopModeStatus>("laptop_mode_status");
}

export async function updateLaptopMode(enabled: boolean): Promise<LaptopModeStatus> {
  if (!isTauriRuntime()) {
    previewLaptopMode = enabled;
    return previewStatus();
  }
  return invoke<LaptopModeStatus>("set_laptop_mode", { enabled });
}

export function resetLaptopModePreview(): void {
  previewLaptopMode = false;
}
