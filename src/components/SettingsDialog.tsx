import { AlertTriangle, Check, Keyboard, RotateCcw, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  type GlobalShortcutSettingsRequest,
  type GlobalShortcutStatus,
  type ShortcutBinding,
} from "../shortcuts";
import { acceleratorFromEvent } from "../shortcutCapture";

interface SettingsDialogProps {
  status: GlobalShortcutStatus;
  saving: boolean;
  error: string | null;
  onSave: (request: GlobalShortcutSettingsRequest) => void;
  onClose: () => void;
}

export function SettingsDialog({ status, saving, error, onSave, onClose }: SettingsDialogProps) {
  const [enabled, setEnabled] = useState(status.enabled);
  const [bindings, setBindings] = useState<ShortcutBinding[]>(status.bindings);
  const [recordingAction, setRecordingAction] = useState<string | null>(null);

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
  const isDirty = enabled !== status.enabled || bindings.some((binding) => (
    status.bindings.find((current) => current.action === binding.action)?.accelerator !== binding.accelerator
  ));

  const registration = !status.platformAvailable
    ? { tone: "preview", title: "Native app only", copy: "The browser preview can edit this screen, but Windows registers shortcuts only in the installed Aurora app." }
    : status.enabled && status.registered
      ? { tone: "ready", title: "Global shortcuts active", copy: "Windows is listening even when Aurora is behind another app." }
      : status.enabled
        ? { tone: "error", title: "Shortcuts not registered", copy: status.error ?? "One or more shortcuts are unavailable." }
        : { tone: "off", title: "Global shortcuts off", copy: "Aurora will not respond to shortcuts outside the app." };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="settings-dialog__header">
          <div className="settings-dialog__mark"><Keyboard aria-hidden="true" /></div>
          <div><p className="eyebrow">Aurora settings</p><h2 id="settings-title">Global shortcuts</h2></div>
          <button type="button" className="settings-dialog__close" aria-label="Close settings" onClick={onClose}><X aria-hidden="true" /></button>
        </header>

        <div className="settings-dialog__body">
          <label className="settings-switch">
            <span><strong>Enable global shortcuts</strong><small>Use playback controls while Edge or another app has focus.</small></span>
            <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />
            <i aria-hidden="true" />
          </label>

          <div className={`shortcut-status shortcut-status--${registration.tone}`} role="status">
            {registration.tone === "ready" ? <Check aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
            <span><strong>{registration.title}</strong><small>{registration.copy}</small></span>
          </div>
          {status.warning && <p className="shortcut-warning">{status.warning}</p>}

          <div className="shortcut-heading">
            <span><strong>Bindings</strong><small>Click a shortcut, then press a modifier and key.</small></span>
            <button type="button" onClick={() => setBindings((current) => current.map((item) => ({
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
                  onClick={() => setRecordingAction(binding.action)}
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
          {(validationError || error) && <p className="settings-error" role="alert">{validationError ?? error}</p>}
        </div>

        <footer className="settings-dialog__footer">
          <button type="button" className="button button--quiet" onClick={onClose}>Cancel</button>
          <button
            type="button"
            className="button button--primary"
            disabled={saving || !isDirty || Boolean(validationError) || Boolean(recordingAction)}
            onClick={() => onSave({
              enabled,
              bindings: bindings.map(({ action, accelerator }) => ({ action, accelerator })),
            })}
          >{saving ? "Saving…" : "Save changes"}</button>
        </footer>
      </section>
    </div>
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
