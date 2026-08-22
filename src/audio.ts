import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";

export type ReplayGainMode = "off" | "track" | "album";

export interface AudioSettings {
  outputDeviceId: string;
  replayGainMode: ReplayGainMode;
}

export interface AudioOutputDevice {
  id: string;
  label: string;
  isDefault: boolean;
}

export interface AudioSettingsStatus {
  settings: AudioSettings;
  devices: AudioOutputDevice[];
  activeDeviceId: string | null;
  activeDeviceLabel: string | null;
  usingFallback: boolean;
  message: string | null;
  error: string | null;
}

export type AudioSettingsRequest = AudioSettings;

const previewDevices: AudioOutputDevice[] = [
  { id: "preview-speakers", label: "Speakers (Realtek Audio)", isDefault: true },
  { id: "preview-dac", label: "USB DAC", isDefault: false },
];

let previewStatus: AudioSettingsStatus = {
  settings: { outputDeviceId: "system-default", replayGainMode: "off" },
  devices: previewDevices,
  activeDeviceId: "preview-speakers",
  activeDeviceLabel: "Speakers (Realtek Audio)",
  usingFallback: false,
  message: null,
  error: null,
};

function cloneStatus(status: AudioSettingsStatus): AudioSettingsStatus {
  return {
    ...status,
    settings: { ...status.settings },
    devices: status.devices.map((device) => ({ ...device })),
  };
}

export async function loadAudioSettings(): Promise<AudioSettingsStatus> {
  if (!isTauriRuntime()) return cloneStatus(previewStatus);
  return invoke<AudioSettingsStatus>("audio_settings");
}

export async function updateAudioSettings(
  request: AudioSettingsRequest,
): Promise<AudioSettingsStatus> {
  if (!isTauriRuntime()) {
    const selected = request.outputDeviceId === "system-default"
      ? previewDevices.find((device) => device.isDefault)
      : previewDevices.find((device) => device.id === request.outputDeviceId);
    previewStatus = {
      ...previewStatus,
      settings: { ...request },
      activeDeviceId: selected?.id ?? previewDevices[0].id,
      activeDeviceLabel: selected?.label ?? previewDevices[0].label,
      usingFallback: !selected,
      message: selected ? null : "The selected output is unavailable. Aurora continued on the Windows default.",
    };
    return cloneStatus(previewStatus);
  }
  return invoke<AudioSettingsStatus>("update_audio_settings", { request });
}

export function previewAudioSnapshot(): Pick<
  AudioSettingsStatus,
  "settings" | "activeDeviceLabel" | "usingFallback"
> {
  return {
    settings: { ...previewStatus.settings },
    activeDeviceLabel: previewStatus.activeDeviceLabel,
    usingFallback: previewStatus.usingFallback,
  };
}

export function resetAudioPreview(): void {
  previewStatus = {
    settings: { outputDeviceId: "system-default", replayGainMode: "off" },
    devices: previewDevices,
    activeDeviceId: "preview-speakers",
    activeDeviceLabel: "Speakers (Realtek Audio)",
    usingFallback: false,
    message: null,
    error: null,
  };
}
