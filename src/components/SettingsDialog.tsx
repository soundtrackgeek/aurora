import {
  AlertTriangle,
  Check,
  Headphones,
  Image as ImageIcon,
  Keyboard,
  MonitorCog,
  RotateCcw,
  ShieldCheck,
  Tags,
  Type,
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
import {
  coverSizeOptions,
  createDefaultDisplayPreferences,
  displayViews,
  effectiveDisplayPreferences,
  textSizeOptions,
  type CoverSize,
  type DisplayPreferences,
  type DisplayViewKey,
  type TextSize,
} from "../displayPreferences";
import { loadInboxSettings, updateDiscogsCredentials, type InboxSettingsStatus } from "../inbox";

export type SettingsTab = "display" | "audio" | "shortcuts" | "metadata";

interface SettingsDialogProps {
  shortcutStatus: GlobalShortcutStatus;
  audioStatus: AudioSettingsStatus;
  shortcutSaving: boolean;
  audioSaving: boolean;
  shortcutError: string | null;
  audioError: string | null;
  displayPreferences: DisplayPreferences;
  activeDisplayView: DisplayViewKey;
  initialTab?: SettingsTab;
  onSaveDisplay: (preferences: DisplayPreferences) => void;
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
  displayPreferences,
  activeDisplayView,
  initialTab = "audio",
  onSaveDisplay,
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
  const [displayDraft, setDisplayDraft] = useState<DisplayPreferences>(() => copyDisplayPreferences(displayPreferences));
  const [selectedDisplayView, setSelectedDisplayView] = useState<DisplayViewKey>(activeDisplayView);
  const [metadataStatus, setMetadataStatus] = useState<InboxSettingsStatus | null>(null);
  const [discogsToken, setDiscogsToken] = useState("");
  const [discogsCredentialMode, setDiscogsCredentialMode] = useState<"token" | "consumer">("token");
  const [discogsConsumerKey, setDiscogsConsumerKey] = useState("");
  const [discogsConsumerSecret, setDiscogsConsumerSecret] = useState("");
  const [removeDiscogsToken, setRemoveDiscogsToken] = useState(false);
  const [metadataSaving, setMetadataSaving] = useState(false);
  const [metadataError, setMetadataError] = useState<string | null>(null);

  useEffect(() => {
    if (tab !== "metadata" || metadataStatus) return;
    void loadInboxSettings().then(setMetadataStatus).catch((error: unknown) => setMetadataError(error instanceof Error ? error.message : String(error)));
  }, [metadataStatus, tab]);

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
  const displayDirty = JSON.stringify(displayDraft) !== JSON.stringify(displayPreferences);
  const metadataDirty = removeDiscogsToken
    || (discogsCredentialMode === "token" ? Boolean(discogsToken.trim()) : Boolean(discogsConsumerKey.trim() && discogsConsumerSecret.trim()));
  const activeTitle = tab === "display" ? "Display" : tab === "audio" ? "Audio" : tab === "metadata" ? "Metadata" : "Global shortcuts";
  const activeSaving = tab === "display" ? false : tab === "audio" ? audioSaving : tab === "metadata" ? metadataSaving : shortcutSaving;
  const activeDirty = tab === "display" ? displayDirty : tab === "audio" ? audioDirty : tab === "metadata" ? metadataDirty : shortcutsDirty;
  const activeError = tab === "display" ? null : tab === "audio" ? audioError : tab === "metadata" ? metadataError : shortcutError;

  function saveActiveTab() {
    if (tab === "display") {
      onSaveDisplay(displayDraft);
    } else if (tab === "audio") {
      onSaveAudio({ outputDeviceId, replayGainMode });
    } else if (tab === "shortcuts") {
      onSaveShortcuts({
        enabled,
        bindings: bindings.map(({ action, accelerator }) => ({ action, accelerator })),
      });
    } else {
      setMetadataSaving(true);
      setMetadataError(null);
      void updateDiscogsCredentials(removeDiscogsToken
        ? { mode: "clear" }
        : discogsCredentialMode === "token"
          ? { mode: "token", token: discogsToken }
          : { mode: "consumer", consumerKey: discogsConsumerKey, consumerSecret: discogsConsumerSecret })
        .then((status) => {
          setMetadataStatus(status);
          setDiscogsToken("");
          setDiscogsConsumerKey("");
          setDiscogsConsumerSecret("");
          setRemoveDiscogsToken(false);
        })
        .catch((error: unknown) => setMetadataError(error instanceof Error ? error.message : String(error)))
        .finally(() => setMetadataSaving(false));
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="settings-dialog__header">
          <div className="settings-dialog__mark">{tab === "display" ? <MonitorCog aria-hidden="true" /> : tab === "audio" ? <Volume2 aria-hidden="true" /> : tab === "metadata" ? <Tags aria-hidden="true" /> : <Keyboard aria-hidden="true" />}</div>
          <div><p className="eyebrow">Aurora settings</p><h2 id="settings-title">{activeTitle}</h2></div>
          <button type="button" className="settings-dialog__close" aria-label="Close settings" onClick={onClose}><X aria-hidden="true" /></button>
        </header>

        <nav className="settings-tabs" aria-label="Settings sections" role="tablist">
          <button type="button" role="tab" aria-selected={tab === "display"} onClick={() => setTab("display")}><MonitorCog aria-hidden="true" /> Display</button>
          <button type="button" role="tab" aria-selected={tab === "audio"} onClick={() => setTab("audio")}><Volume2 aria-hidden="true" /> Audio</button>
          <button type="button" role="tab" aria-selected={tab === "shortcuts"} onClick={() => setTab("shortcuts")}><Keyboard aria-hidden="true" /> Shortcuts</button>
          <button type="button" role="tab" aria-selected={tab === "metadata"} onClick={() => setTab("metadata")}><Tags aria-hidden="true" /> Metadata</button>
        </nav>

        <div className="settings-dialog__body">
          {tab === "display" ? (
            <DisplaySettingsPanel
              preferences={displayDraft}
              selectedView={selectedDisplayView}
              onPreferencesChange={setDisplayDraft}
              onSelectedViewChange={setSelectedDisplayView}
            />
          ) : tab === "audio" ? (
            <AudioSettingsPanel
              status={audioStatus}
              outputDeviceId={outputDeviceId}
              replayGainMode={replayGainMode}
              onOutputDeviceChange={setOutputDeviceId}
              onReplayGainModeChange={setReplayGainMode}
            />
          ) : tab === "shortcuts" ? (
            <ShortcutSettingsPanel
              status={shortcutStatus}
              enabled={enabled}
              bindings={bindings}
              recordingAction={recordingAction}
              onEnabledChange={setEnabled}
              onBindingsChange={setBindings}
              onRecordingActionChange={setRecordingAction}
            />
          ) : (
            <MetadataSettingsPanel
              status={metadataStatus}
              token={discogsToken}
              credentialMode={discogsCredentialMode}
              consumerKey={discogsConsumerKey}
              consumerSecret={discogsConsumerSecret}
              removeToken={removeDiscogsToken}
              onCredentialModeChange={setDiscogsCredentialMode}
              onTokenChange={(value) => { setDiscogsToken(value); setRemoveDiscogsToken(false); }}
              onConsumerKeyChange={(value) => { setDiscogsConsumerKey(value); setRemoveDiscogsToken(false); }}
              onConsumerSecretChange={(value) => { setDiscogsConsumerSecret(value); setRemoveDiscogsToken(false); }}
              onRemoveTokenChange={setRemoveDiscogsToken}
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

function MetadataSettingsPanel({
  status,
  token,
  credentialMode,
  consumerKey,
  consumerSecret,
  removeToken,
  onCredentialModeChange,
  onTokenChange,
  onConsumerKeyChange,
  onConsumerSecretChange,
  onRemoveTokenChange,
}: {
  status: InboxSettingsStatus | null;
  token: string;
  credentialMode: "token" | "consumer";
  consumerKey: string;
  consumerSecret: string;
  removeToken: boolean;
  onCredentialModeChange: (value: "token" | "consumer") => void;
  onTokenChange: (value: string) => void;
  onConsumerKeyChange: (value: string) => void;
  onConsumerSecretChange: (value: string) => void;
  onRemoveTokenChange: (value: boolean) => void;
}) {
  return (
    <div className="metadata-settings">
      <section className="display-settings__section" aria-labelledby="discogs-settings-heading">
        <header>
          <span><Tags aria-hidden="true" /><span><strong id="discogs-settings-heading">Discogs</strong><small>Used by Inbox Album Auto-Tagger alongside MusicBrainz.</small></span></span>
          <span className={`metadata-connection${status?.discogsConfigured ? " is-connected" : ""}`}>{status ? status.discogsConfigured ? <><ShieldCheck aria-hidden="true" /> Connected · {status.discogsAuthMode === "consumer" ? "consumer app" : "personal token"}</> : status.discogsIncompleteConsumerKey ? "Consumer secret needed" : "Not connected" : "Checking…"}</span>
        </header>
        <label className="metadata-auth-mode"><span>Authentication method</span><select value={credentialMode} disabled={removeToken} onChange={(event) => onCredentialModeChange(event.target.value as "token" | "consumer")}><option value="token">Personal access token</option><option value="consumer">Consumer key + secret</option></select></label>
        {credentialMode === "token" ? <label className="metadata-token-field">
          <span>Personal access token</span><input type="password" autoComplete="off" value={token} disabled={removeToken} placeholder={status?.discogsAuthMode === "token" ? "Saved securely · enter a replacement" : "Enter your Discogs token"} onChange={(event) => onTokenChange(event.target.value)} />
          <small>One token is enough for personal use.</small>
        </label> : <div className="metadata-consumer-fields"><label className="metadata-token-field"><span>Consumer key</span><input type="password" autoComplete="off" value={consumerKey} disabled={removeToken} placeholder={status?.discogsAuthMode === "consumer" || status?.discogsIncompleteConsumerKey ? "Saved or detected · enter key to replace" : "Enter consumer key"} onChange={(event) => onConsumerKeyChange(event.target.value)} /></label><label className="metadata-token-field"><span>Consumer secret</span><input type="password" autoComplete="off" value={consumerSecret} disabled={removeToken} placeholder={status?.discogsAuthMode === "consumer" ? "Saved securely · enter secret to replace" : "Enter matching consumer secret"} onChange={(event) => onConsumerSecretChange(event.target.value)} /></label></div>}
        <p className="metadata-vault-note">Aurora stores production credentials in your operating system credential vault. Saved values are never displayed or written to Aurora settings files.</p>
        {status?.discogsConfigured ? <label className="metadata-remove"><input type="checkbox" checked={removeToken} onChange={(event) => onRemoveTokenChange(event.target.checked)} /> Remove the saved Discogs credentials</label> : null}
      </section>
      <div className="metadata-settings__note"><ShieldCheck aria-hidden="true" /><span><strong>MusicBrainz needs no key.</strong><small>Aurora identifies itself and observes MusicBrainz's one-request-per-second limit.</small></span></div>
    </div>
  );
}

function DisplaySettingsPanel({
  preferences,
  selectedView,
  onPreferencesChange,
  onSelectedViewChange,
}: {
  preferences: DisplayPreferences;
  selectedView: DisplayViewKey;
  onPreferencesChange: (preferences: DisplayPreferences) => void;
  onSelectedViewChange: (view: DisplayViewKey) => void;
}) {
  const view = displayViews.find(({ id }) => id === selectedView) ?? displayViews[0];
  const override = preferences.views[selectedView];
  const effective = effectiveDisplayPreferences(preferences, selectedView);
  const globalTextLabel = textSizeOptions.find(({ value }) => value === preferences.global.textSize)?.label ?? preferences.global.textSize;
  const globalCoverLabel = coverSizeOptions.find(({ value }) => value === preferences.global.coverSize)?.label ?? preferences.global.coverSize;

  function updateGlobal(next: Partial<DisplayPreferences["global"]>) {
    onPreferencesChange({ ...preferences, global: { ...preferences.global, ...next } });
  }

  function updateOverride(next: Partial<DisplayPreferences["views"][DisplayViewKey]>) {
    onPreferencesChange({
      ...preferences,
      views: {
        ...preferences.views,
        [selectedView]: { ...override, ...next },
      },
    });
  }

  return (
    <div className="display-settings">
      <section className="display-settings__section" aria-labelledby="global-display-heading">
        <header><span><MonitorCog aria-hidden="true" /><span><strong id="global-display-heading">Global defaults</strong><small>Applied to Aurora chrome and every view without an override.</small></span></span></header>
        <div className="display-settings__fields">
          <label><span><Type aria-hidden="true" /> Text size</span><select aria-label="Global text size" value={preferences.global.textSize} onChange={(event) => updateGlobal({ textSize: event.target.value as TextSize })}>{textSizeOptions.map((option) => <option value={option.value} key={option.value}>{option.label} · {option.detail}</option>)}</select></label>
          <label><span><ImageIcon aria-hidden="true" /> Cover size</span><select aria-label="Global cover size" value={preferences.global.coverSize} onChange={(event) => updateGlobal({ coverSize: event.target.value as CoverSize })}>{coverSizeOptions.map((option) => <option value={option.value} key={option.value}>{option.label} · {option.detail}</option>)}</select></label>
        </div>
      </section>

      <section className="display-settings__section" aria-labelledby="view-display-heading">
        <header>
          <span><RotateCcw aria-hidden="true" /><span><strong id="view-display-heading">Per-view override</strong><small>Tune dense surfaces independently. Charts starts larger for readability.</small></span></span>
          <button type="button" disabled={override.textSize === null && override.coverSize === null} onClick={() => updateOverride({ textSize: null, coverSize: null })}><RotateCcw aria-hidden="true" /> Use globals</button>
        </header>
        <label className="display-settings__view"><span>View</span><select aria-label="View to customize" value={selectedView} onChange={(event) => onSelectedViewChange(event.target.value as DisplayViewKey)}>{displayViews.map((option) => <option value={option.id} key={option.id}>{option.label}</option>)}</select></label>
        <div className="display-settings__fields">
          <label><span><Type aria-hidden="true" /> Text size</span><select aria-label={`${view.label} text size`} value={override.textSize ?? ""} onChange={(event) => updateOverride({ textSize: event.target.value ? event.target.value as TextSize : null })}><option value="">Use global · {globalTextLabel}</option>{textSizeOptions.map((option) => <option value={option.value} key={option.value}>{option.label} · {option.detail}</option>)}</select></label>
          <label><span><ImageIcon aria-hidden="true" /> Cover size</span><select aria-label={`${view.label} cover size`} value={override.coverSize ?? ""} disabled={!view.supportsCovers} onChange={(event) => updateOverride({ coverSize: event.target.value ? event.target.value as CoverSize : null })}><option value="">{view.supportsCovers ? `Use global · ${globalCoverLabel}` : "No adjustable covers"}</option>{view.supportsCovers && coverSizeOptions.map((option) => <option value={option.value} key={option.value}>{option.label} · {option.detail}</option>)}</select></label>
        </div>
      </section>

      <div className="display-preview" data-text-size={effective.textSize} data-cover-size={effective.coverSize} aria-label={`${view.label} preview`}>
        <span className="display-preview__cover" aria-hidden="true"><ImageIcon /></span>
        <span><strong>{view.label} preview</strong><small>Readable metadata stays secondary without becoming microscopic.</small></span>
      </div>

      <button type="button" className="display-settings__restore" onClick={() => onPreferencesChange(createDefaultDisplayPreferences())}><RotateCcw aria-hidden="true" /> Restore readable defaults</button>
    </div>
  );
}

function copyDisplayPreferences(preferences: DisplayPreferences): DisplayPreferences {
  return {
    global: { ...preferences.global },
    views: Object.fromEntries(Object.entries(preferences.views).map(([view, override]) => [view, { ...override }])) as DisplayPreferences["views"],
  };
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
      <p className="shortcut-atomic">Aurora registers the entire set together. Rating defaults use the numeric keypad so AltGr characters stay available. If MusicBee or another app owns one binding, change that shortcut and save again.</p>
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
