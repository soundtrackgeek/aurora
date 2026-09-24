import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isTauriRuntime, type Track } from "./library";
import type { CatalogSync } from "./tags";
import type { PlaybackSnapshot } from "./playback";

export interface ShortcutBinding {
  action: string;
  label: string;
  accelerator: string;
  defaultAccelerator: string;
}

export interface GlobalShortcutStatus {
  platform: "macos" | "windows" | "other";
  enabled: boolean;
  registered: boolean;
  platformAvailable: boolean;
  error: string | null;
  warning: string | null;
  bindings: ShortcutBinding[];
}

export interface GlobalShortcutSettingsRequest {
  enabled: boolean;
  bindings: Array<Pick<ShortcutBinding, "action" | "accelerator">>;
}

export interface GlobalShortcutResult {
  action: string;
  success: boolean;
  message: string;
  track: Track | null;
  previousTrack: Track | null;
  catalogSync: CatalogSync | null;
  playback: PlaybackSnapshot | null;
}

const previewPlatform: GlobalShortcutStatus["platform"] = navigator.userAgent.includes("Mac") ? "macos" : "windows";
const previewModifiers = previewPlatform === "macos" ? "Super+Alt" : "Ctrl+Alt";

export const defaultShortcutBindings: ShortcutBinding[] = [
  binding("playPause", "Play or pause", `${previewModifiers}+P`),
  binding("next", "Next track", `${previewModifiers}+N`),
  binding("rating0", "Clear rating", `${previewModifiers}+Numpad0`),
  binding("rating1", "Rate 1 star", `${previewModifiers}+Numpad1`),
  binding("rating2", "Rate 2 stars", `${previewModifiers}+Numpad2`),
  binding("rating3", "Rate 3 stars", `${previewModifiers}+Numpad3`),
  binding("rating4", "Rate 4 stars", `${previewModifiers}+Numpad4`),
  binding("rating5", "Rate 5 stars", `${previewModifiers}+Numpad5`),
  binding("love", "Toggle Love", `${previewModifiers}+L`),
];

let previewStatus: GlobalShortcutStatus = {
  platform: previewPlatform,
  enabled: true,
  registered: false,
  platformAvailable: false,
  error: null,
  warning: null,
  bindings: defaultShortcutBindings,
};

function binding(action: string, label: string, accelerator: string): ShortcutBinding {
  return { action, label, accelerator, defaultAccelerator: accelerator };
}

function cloneStatus(status: GlobalShortcutStatus): GlobalShortcutStatus {
  return { ...status, bindings: status.bindings.map((item) => ({ ...item })) };
}

export async function loadGlobalShortcutSettings(): Promise<GlobalShortcutStatus> {
  if (!isTauriRuntime()) return cloneStatus(previewStatus);
  return invoke<GlobalShortcutStatus>("global_shortcut_settings");
}

export async function updateGlobalShortcutSettings(
  request: GlobalShortcutSettingsRequest,
): Promise<GlobalShortcutStatus> {
  if (!isTauriRuntime()) {
    previewStatus = {
      ...previewStatus,
      enabled: request.enabled,
      bindings: defaultShortcutBindings.map((item) => ({
        ...item,
        accelerator: request.bindings.find((candidate) => candidate.action === item.action)?.accelerator
          ?? item.defaultAccelerator,
      })),
    };
    return cloneStatus(previewStatus);
  }
  return invoke<GlobalShortcutStatus>("update_global_shortcut_settings", { request });
}

export async function listenForGlobalShortcutResults(
  handler: (result: GlobalShortcutResult) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<GlobalShortcutResult>("aurora-global-shortcut-result", (event) => handler(event.payload));
}

export function resetGlobalShortcutPreview(): void {
  previewStatus = {
    platform: previewPlatform,
    enabled: true,
    registered: false,
    platformAvailable: false,
    error: null,
    warning: null,
    bindings: defaultShortcutBindings,
  };
}
