import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { loadAudioSettings, updateAudioSettings, type AudioSettingsRequest, type AudioSettingsStatus } from '../../audio';
import type { SettingsTab } from '../../components/SettingsDialog';
import { loadLaptopModeStatus, updateLaptopMode, type LaptopModeStatus } from '../../laptopMode';
import {
  defaultShortcutBindings,
  loadGlobalShortcutSettings,
  updateGlobalShortcutSettings,
  type GlobalShortcutSettingsRequest,
  type GlobalShortcutStatus
} from '../../shortcuts';
export function useAppSettings(onAlbumOrderChanged: () => void) {
  const onAlbumOrderChangedRef = useRef(onAlbumOrderChanged);
  useLayoutEffect(() => { onAlbumOrderChangedRef.current = onAlbumOrderChanged; }, [onAlbumOrderChanged]);
  const [laptopModeStatus, setLaptopModeStatus] = useState<LaptopModeStatus | null>(null);

  const [laptopModeBusy, setLaptopModeBusy] = useState(false);

  const [laptopModeError, setLaptopModeError] = useState<string | null>(null);

  const albumOrderRevisionRef = useRef(0);

  const [settingsOpen, setSettingsOpen] = useState(false);

  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>("audio");

  const [shortcutStatus, setShortcutStatus] = useState<GlobalShortcutStatus | null>(null);

  const [shortcutSaving, setShortcutSaving] = useState(false);

  const [shortcutError, setShortcutError] = useState<string | null>(null);

  const [audioStatus, setAudioStatus] = useState<AudioSettingsStatus | null>(null);

  const [audioSaving, setAudioSaving] = useState(false);

  const [audioError, setAudioError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      void loadLaptopModeStatus()
        .then((status) => {
          if (cancelled) return;
          if (status.albumOrderRevision > albumOrderRevisionRef.current) {
            albumOrderRevisionRef.current = status.albumOrderRevision;
            onAlbumOrderChangedRef.current();
          }
          setLaptopModeStatus(status);
          setLaptopModeError(null);
        })
        .catch((error: unknown) => {
          if (!cancelled) setLaptopModeError(error instanceof Error ? error.message : String(error));
        });
    };
    refresh();
    const interval = window.setInterval(refresh, 5_000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    if (!settingsOpen) return;
    let cancelled = false;
    void Promise.allSettled([loadGlobalShortcutSettings(), loadAudioSettings()]).then(([shortcuts, audio]) => {
      if (cancelled) return;
      if (shortcuts.status === "fulfilled") setShortcutStatus(shortcuts.value);
      else {
        const message = shortcuts.reason instanceof Error ? shortcuts.reason.message : String(shortcuts.reason);
        setShortcutStatus({
          platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
          enabled: true,
          registered: false,
          platformAvailable: true,
          error: message,
          warning: null,
          bindings: defaultShortcutBindings,
        });
      }
      if (audio.status === "fulfilled") setAudioStatus(audio.value);
      else {
        const message = audio.reason instanceof Error ? audio.reason.message : String(audio.reason);
        setAudioStatus({
          settings: { outputDeviceId: "system-default", replayGainMode: "off" },
          devices: [],
          activeDeviceId: null,
          activeDeviceLabel: null,
          usingFallback: false,
          message: null,
          error: message,
        });
      }
    });
    return () => { cancelled = true; };
  }, [settingsOpen]);

  async function toggleLaptopMode() {
    if (!laptopModeStatus || laptopModeBusy) return;
    setLaptopModeBusy(true);
    setLaptopModeError(null);
    try {
      setLaptopModeStatus(await updateLaptopMode(!laptopModeStatus.laptopMode));
    } catch (error) {
      setLaptopModeError(error instanceof Error ? error.message : String(error));
    } finally {
      setLaptopModeBusy(false);
    }
  }

  async function saveGlobalShortcuts(request: GlobalShortcutSettingsRequest) {
    setShortcutSaving(true);
    setShortcutError(null);
    try {
      setShortcutStatus(await updateGlobalShortcutSettings(request));
    } catch (error) {
      setShortcutError(error instanceof Error ? error.message : String(error));
    } finally {
      setShortcutSaving(false);
    }
  }

  async function saveAudioSettings(request: AudioSettingsRequest) {
    setAudioSaving(true);
    setAudioError(null);
    try {
      setAudioStatus(await updateAudioSettings(request));
    } catch (error) {
      setAudioError(error instanceof Error ? error.message : String(error));
    } finally {
      setAudioSaving(false);
    }
  }

  function openSettings(tab: SettingsTab = "audio") {
    setSettingsInitialTab(tab);
    setShortcutError(null);
    setAudioError(null);
    setSettingsOpen(true);
  }
  return { settingsOpen, setSettingsOpen, settingsInitialTab, shortcutStatus, shortcutSaving, shortcutError, audioStatus, audioSaving, audioError, laptopModeStatus, laptopModeBusy, laptopModeError, toggleLaptopMode, saveGlobalShortcuts, saveAudioSettings, openSettings };
}
