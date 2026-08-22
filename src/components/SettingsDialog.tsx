import {
  AlertTriangle,
  Check,
  Headphones,
  Keyboard,
  RotateCcw,
  ShieldCheck,
  Volume2,
  Waves,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  type AudioSettingsRequest,
  type AudioSettingsStatus,
  type ReplayGainMode,
} from "../audio";
import {
  type GlobalShortcutSettingsRequest,
  type GlobalShortcutStatus,
  type ShortcutBinding,
} from "../shortcuts";
import { acceleratorFromEvent } from "../shortcutCapture";

export type SettingsTab = "audio" | "shortcuts";

interface SettingsDialogProps {
  shortcutStatus: GlobalShortcutStatus;
  audioStatus: AudioSettingsStatus;
  shortcutSaving: boolean;
  audioSaving: boolean;
  shortcutError: string | null;
  audioError: string | null;
  initialTab?: SettingsTab;
  onSaveShortcuts: (request: GlobalShortcutSettingsRequest) => void;
  onSaveAudio: (request: AudioSettingsRequest) => void;
  onClose: () => void;
}

export function SettingsDialog({
  shortcutStatus,
  audioStatus,
  shortcutSaving,
  audioSaving,
  shortcutError,
  audioError,
  initialTab = "audio",
  onSaveShortcuts,
  onSaveAudio,
  onClose,
}: SettingsDialogProps) {
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  const [enabled, setEnabled] = useState(shortcutStatus.enabled);
  const [bindings, setBindings] = useState<ShortcutBinding[]>(shortcutStatus.bindings);
  const [recordingAction, setRecordingAction] = useState<string | null>(null);
  const [outputDeviceId, setOutputDeviceId] = useState(audioStatus.settings.outputDeviceId);
  const [replayGainMode, setReplayGainMode] = useState<ReplayGainMode>(audioStatus.settings.replayGainMode);

  useEffect(() => {
    if (!recordingAction) return;
    function capture(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setRecordingAction(null);
        return;
      }
      const accelerator = acceleratorFromEvent(event);
      if (!accelerator) return;
      setBindings((current) => current.map((item) => (
        item.action === recordingAction ? { ...item, accelerator } : item
      )));
      setRecordingAction(null);
    }
    window.addEventListener("keydown", capture, true);
    return () => window.removeEventListener("keydown", capture, true);
  }, [recordingAction]);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && !recordingAction) onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, recordingAction]);

  const validationError = useMemo(() => validateBindings(bindings), [bindings]);
  const shortcutsDirty = enabled !== shortcutStatus.enabled || bindings.some((binding) => (
    shortcutStatus.bindings.find((current) => current.action === binding.action)?.accelerator !== binding.accelerator
  ));
  const audioDirty = outputDeviceId !== audioStatus.settings.outputDeviceId
    || replayGainMode !== audioStatus.settings.replayGainMode;
  const activeTitle = tab === "audio" ? "Audio" : "Global shortcuts";
  const activeSaving = tab === "audio" ? audioSaving : shortcutSaving;
  const activeDirty = tab === "audio" ? audioDirty : shortcutsDirty;
  const activeError = tab === "audio" ? audioError : shortcutError;

  function saveActiveTab() {
    if (tab === "audio") {
      onSaveAudio({ outputDeviceId, replayGainMode });
    } else {
      onSaveShortcuts({
        enabled,
        bindings: bindings.map(({ action, accelerator }) => ({ action, accelerator })),
      });
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="settings-dialog__header">
          <div className="settings-dialog__mark">{tab === "audio" ? <Volume2 aria-hidden="true" /> : <Keyboard aria-hidden="true" />}</div>
          <div><p className="eyebrow">Aurora settings</p><h2 id="settings-title">{activeTitle}</h2></div>
          <button type="button" className="settings-dialog__close" aria-label="Close settings" onClick={onClose}><X aria-hidden="true" /></button>
        </header>

        <nav className="settings-tabs" aria-label="Settings sections" role="tablist">
          <button type="button" role="tab" aria-selected={tab === "audio"} onClick={() => setTab("audio")}><Volume2 aria-hidden="true" /> Audio</button>
          <button type="button" role="tab" aria-selected={tab === "shortcuts"} onClick={() => setTab("shortcuts")}><Keyboard aria-hidden="true" /> Shortcuts</button>
        </nav>

        <div className="settings-dialog__body">
          {tab === "audio" ? (
            <AudioSettingsPanel
              status={audioStatus}
              outputDeviceId={outputDeviceId}
              replayGainMode={replayGainMode}
              onOutputDeviceChange={setOutputDeviceId}
              onReplayGainModeChange={setReplayGainMode}
            />
          ) : (
            <ShortcutSettingsPanel
              status={shortcutStatus}
              enabled={enabled}
              bindings={bindings}
              recordingAction={recordingAction}
              onEnabledChange={setEnabled}
              onBindingsChange={setBindings}
              onRecordingActionChange={setRecordingAction}
            />
          )}
          {((tab === "shortcuts" && validationError) || activeError) && (
            <p className="settings-error" role="alert">{tab === "shortcuts" ? validationError ?? activeError : activeError}</p>
          )}
        </div>

        <footer className="settings-dialog__footer">
          <button type="button" className="button button--quiet" onClick={onClose}>Cancel</button>
          <button
            type="button"
            className="button button--primary"
            disabled={activeSaving || !activeDirty || (tab === "shortcuts" && (Boolean(validationError) || Boolean(recordingAction)))}
            onClick={saveActiveTab}
          >{activeSaving ? "Saving…" : "Save changes"}</button>
        </footer>
      </section>
    </div>
  );
}

function AudioSettingsPanel({
  status,
  outputDeviceId,
  replayGainMode,
  onOutputDeviceChange,
  onReplayGainModeChange,
}: {
  status: AudioSettingsStatus;
  outputDeviceId: string;
  replayGainMode: ReplayGainMode;
  onOutputDeviceChange: (deviceId: string) => void;
  onReplayGainModeChange: (mode: ReplayGainMode) => void;
}) {
  const selectedIsMissing = outputDeviceId !== "system-default"
    && !status.devices.some((device) => device.id === outputDeviceId);
  const audioState = status.error
    ? { tone: "error", title: "Windows audio unavailable", copy: status.error }
    : status.usingFallback
      ? { tone: "error", title: "Using Windows default", copy: status.message ?? "The preferred output is not available." }
      : {
          tone: "ready",
          title: status.activeDeviceLabel ? "Audio output active" : "Output ready",
          copy: status.activeDeviceLabel ?? "Aurora will open the selected output when playback starts.",
        };

  return (
    <div className="audio-settings">
      <div className={`shortcut-status shortcut-status--${audioState.tone}`} role="status">
        {audioState.tone === "ready" ? <Check aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
        <span><strong>{audioState.title}</strong><small>{audioState.copy}</small></span>
      </div>

      <label className="audio-field">
        <span><Headphones aria-hidden="true" /><span><strong>Output device</strong><small>Stored only on this computer. Laptop and Desktop choices stay independent.</small></span></span>
        <select value={outputDeviceId} onChange={(event) => onOutputDeviceChange(event.target.value)}>
          <option value="system-default">Windows default (recommended)</option>
          {selectedIsMissing && <option value={outputDeviceId}>Unavailable output (preference kept)</option>}
          {status.devices.map((device) => (
            <option value={device.id} key={device.id}>{device.label}{device.isDefault ? " · current default" : ""}</option>
          ))}
        </select>
      </label>

      <fieldset className="replay-gain-settings">
        <legend><Waves aria-hidden="true" /><span><strong>ReplayGain</strong><small>Normalize loudness from tags without changing the MP3.</small></span></legend>
        <div className="replay-gain-options">
          {([
            ["off", "Off", "Original level"],
            ["track", "Track", "Each song"],
            ["album", "Album", "Album dynamics"],
          ] as const).map(([value, label, copy]) => (
            <label key={value} className={replayGainMode === value ? "is-selected" : undefined}>
              <input type="radio" name="replay-gain" value={value} checked={replayGainMode === value} onChange={() => onReplayGainModeChange(value)} />
              <strong>{label}</strong><small>{copy}</small>
            </label>
          ))}
        </div>
      </fieldset>

      <div className="audio-engine-note">
        <ShieldCheck aria-hidden="true" />
        <span><strong>Clipping protection is always on</strong><small>Tagged peaks cap positive gain. Album mode falls back to Track gain when album frames are missing.</small></span>
      </div>
      <div className="audio-engine-note">
        <Waves aria-hidden="true" />
        <span><strong>Gapless queue transitions</strong><small>Aurora opens and queues the next MP3 before the current one ends. Shuffle and repeat remain authoritative.</small></span>
      </div>
    </div>
  );
}

function ShortcutSettingsPanel({
  status,
  enabled,
  bindings,
  recordingAction,
  onEnabledChange,
  onBindingsChange,
  onRecordingActionChange,
}: {
  status: GlobalShortcutStatus;
  enabled: boolean;
  bindings: ShortcutBinding[];
  recordingAction: string | null;
  onEnabledChange: (enabled: boolean) => void;
  onBindingsChange: (bindings: ShortcutBinding[]) => void;
  onRecordingActionChange: (action: string | null) => void;
}) {
  const registration = !status.platformAvailable
    ? { tone: "preview", title: "Native app only", copy: "The browser preview can edit this screen, but Windows registers shortcuts only in the installed Aurora app." }
    : status.enabled && status.registered
      ? { tone: "ready", title: "Global shortcuts active", copy: "Windows is listening even when Aurora is behind another app." }
      : status.enabled
        ? { tone: "error", title: "Shortcuts not registered", copy: status.error ?? "One or more shortcuts are unavailable." }
        : { tone: "off", title: "Global shortcuts off", copy: "Aurora will not respond to shortcuts outside the app." };

  return (
    <>
      <label className="settings-switch">
        <span><strong>Enable global shortcuts</strong><small>Use playback controls while Edge or another app has focus.</small></span>
        <input type="checkbox" checked={enabled} onChange={(event) => onEnabledChange(event.target.checked)} />
        <i aria-hidden="true" />
      </label>

      <div className={`shortcut-status shortcut-status--${registration.tone}`} role="status">
        {registration.tone === "ready" ? <Check aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
        <span><strong>{registration.title}</strong><small>{registration.copy}</small></span>
      </div>
      {status.warning && <p className="shortcut-warning">{status.warning}</p>}

      <div className="shortcut-heading">
        <span><strong>Bindings</strong><small>Click a shortcut, then press a modifier and key.</small></span>
        <button type="button" onClick={() => onBindingsChange(bindings.map((item) => ({
          ...item,
          accelerator: item.defaultAccelerator,
        })))}><RotateCcw aria-hidden="true" /> Restore defaults</button>
      </div>

      <div className="shortcut-list">
        {bindings.map((binding) => (
          <div className="shortcut-row" key={binding.action}>
            <span>{binding.label}</span>
            <button
              type="button"
              className={recordingAction === binding.action ? "is-recording" : undefined}
              aria-label={`Change ${binding.label} shortcut. Current shortcut ${binding.accelerator}.`}
              onClick={() => onRecordingActionChange(binding.action)}
            >
              {recordingAction === binding.action
                ? <em>Press keys…</em>
                : binding.accelerator.split("+").map((part) => <kbd key={part}>{displayKey(part)}</kbd>)}
            </button>
          </div>
        ))}
      </div>

      <p className="shortcut-scope"><strong>Now playing is the only target.</strong> Rating and Love shortcuts write instantly to the MP3 and Aurora state for the track currently playing. Selecting another song in Explore never changes the shortcut target.</p>
      <p className="shortcut-atomic">Aurora registers the entire set together. If MusicBee or another app owns one binding, change that shortcut and save again.</p>
    </>
  );
}

function validateBindings(bindings: ShortcutBinding[]): string | null {
  const seen = new Map<string, string>();
  for (const binding of bindings) {
    const duplicate = seen.get(binding.accelerator.toLocaleLowerCase());
    if (duplicate) return `${binding.accelerator} is assigned to both ${duplicate} and ${binding.label}.`;
    seen.set(binding.accelerator.toLocaleLowerCase(), binding.label);
  }
  return null;
}

function displayKey(key: string): string {
  if (key === "Ctrl") return "CTRL";
  if (key === "Super") return "WIN";
  if (key.startsWith("Numpad")) return `NUM ${key.slice(6)}`;
  return key.toLocaleUpperCase();
}
