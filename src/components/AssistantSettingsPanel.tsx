import { useEffect, useState } from "react";
import { BotMessageSquare, Copy } from "lucide-react";
import {
  backendLabels,
  claudeAddCommand,
  codexAddCommand,
  loadAssistantStatus,
  saveAssistantSettings,
  testAssistantBackend,
  type AssistantBackend,
  type AssistantSettings,
  type AssistantStatus,
} from "../assistant";
import { isTauriRuntime } from "../library";
import "./AssistantPanel.css";

function editableFields(settings: AssistantSettings) {
  return { claudePath: settings.claudePath, codexPath: settings.codexPath, claudeModel: settings.claudeModel, codexModel: settings.codexModel };
}

export function AssistantSettingsPanel() {
  const [status, setStatus] = useState<AssistantStatus | null>(null);
  const [paths, setPaths] = useState({ claudePath: "", codexPath: "", claudeModel: "", codexModel: "" });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    let active = true;
    void loadAssistantStatus()
      .then((value) => {
        if (!active) return;
        setStatus(value);
        setPaths(editableFields(value.settings));
      })
      .catch(() => { if (active) setMessage("Could not read the assistant settings."); });
    return () => { active = false; };
  }, []);

  async function save(next: Partial<AssistantSettings>, confirmation?: string) {
    if (!status) return;
    setBusy(true); setMessage("");
    try {
      const saved = await saveAssistantSettings({ ...status.settings, ...next });
      setStatus(saved);
      setPaths(editableFields(saved.settings));
      if (confirmation) setMessage(confirmation);
    } catch (error) { setMessage(String(error instanceof Error ? error.message : error)); }
    finally { setBusy(false); }
  }

  async function test(backend: AssistantBackend) {
    setBusy(true); setMessage("");
    try { setMessage(`${await testAssistantBackend(backend)} is ready.`); }
    catch (error) { setMessage(String(error instanceof Error ? error.message : error)); }
    finally { setBusy(false); }
  }

  async function copy(text: string) {
    try { await navigator.clipboard.writeText(text); setMessage("Command copied."); }
    catch { setMessage("Could not copy. Select the command and copy it instead."); }
  }

  const preview = !isTauriRuntime();
  const pathsChanged = status !== null && (Object.keys(paths) as (keyof typeof paths)[]).some((key) => paths[key].trim() !== status.settings[key]);

  return <section className="display-settings__section assistant-settings" aria-labelledby="assistant-settings-title">
    <header>
      <span><BotMessageSquare aria-hidden="true" /><span><strong id="assistant-settings-title">AI assistant</strong><small>Ask questions about your library with Claude Code or OpenAI Codex, in Aurora or in your terminal.</small></span></span>
      <span className={`metadata-connection${status?.settings.libraryAccess ? " is-connected" : ""}`}>{status?.settings.libraryAccess ? "Library access on" : "Off"}</span>
    </header>

    <label className="settings-switch">
      <span><strong>Let AI assistants read my library</strong><small>Turns on Aurora’s read-only MCP server and the Assistant. Assistants can search the catalog, genres, ratings and your listening history, but cannot change tags, files, ratings or playlists.</small></span>
      <input type="checkbox" checked={status?.settings.libraryAccess ?? false} disabled={busy || !status || preview} onChange={(event) => void save({ libraryAccess: event.target.checked })} />
      <i aria-hidden="true" />
    </label>

    <label className="settings-switch">
      <span><strong>Let AI assistants control Aurora</strong><small>Assistants in Aurora and in your terminal can open searches here, for example the albums that have more than one genre, and propose genre changes. You review every change and apply it yourself; nothing is written until you do. Aurora must be running.</small></span>
      <input type="checkbox" checked={status?.settings.appControl ?? false} disabled={busy || !status || preview || !status.settings.libraryAccess} onChange={(event) => void save({ appControl: event.target.checked })} />
      <i aria-hidden="true" />
    </label>

    <fieldset className="assistant-settings__backends" disabled={busy || !status || preview}>
      <legend>Assistant in Aurora uses</legend>
      {(["claudeCode", "codex"] as const).map((backend) => {
        const cli = backend === "claudeCode" ? status?.claude : status?.codex;
        return <label key={backend} className="assistant-settings__backend">
          <input type="radio" name="assistant-backend" checked={status?.settings.backend === backend} onChange={() => void save({ backend })} />
          <span><strong>{backendLabels[backend]}</strong><small>{cli?.path ? `${cli.source === "settings" ? "Chosen" : "Found"}: ${cli.path}` : "Not found. Install it, or enter its path below."}</small></span>
          <button type="button" className="button" disabled={!cli?.path} onClick={() => void test(backend)}>Test</button>
        </label>;
      })}
    </fieldset>

    <div className="assistant-settings__paths">
      <label className="metadata-token-field"><span>Claude Code executable (optional)</span><input type="text" spellCheck={false} value={paths.claudePath} disabled={busy || preview} placeholder={status?.claude.source === "detected" ? status.claude.path ?? "" : "Detected automatically"} onChange={(event) => setPaths((current) => ({ ...current, claudePath: event.target.value }))} /></label>
      <label className="metadata-token-field"><span>Codex executable (optional)</span><input type="text" spellCheck={false} value={paths.codexPath} disabled={busy || preview} placeholder={status?.codex.source === "detected" ? status.codex.path ?? "" : "Detected automatically · codex.exe"} onChange={(event) => setPaths((current) => ({ ...current, codexPath: event.target.value }))} /></label>
      <label className="metadata-token-field"><span>Claude Code model</span><input type="text" spellCheck={false} value={paths.claudeModel} disabled={busy || preview} placeholder="Claude Code’s default model" onChange={(event) => setPaths((current) => ({ ...current, claudeModel: event.target.value }))} /></label>
      <label className="metadata-token-field"><span>Codex model</span><input type="text" spellCheck={false} value={paths.codexModel} disabled={busy || preview} placeholder="Codex’s default model" onChange={(event) => setPaths((current) => ({ ...current, codexModel: event.target.value }))} /></label>
      <small className="assistant-settings__hint">Leave a model empty to use the CLI’s own default. Aurora starts with Claude Haiku 5.5 (claude-haiku-5-5) and GPT-6-Luna (gpt-6-luna), which are fast and inexpensive.</small>
      <div className="artist-settings-actions"><button type="button" className="button button--primary" disabled={busy || !pathsChanged} onClick={() => void save({ claudePath: paths.claudePath, codexPath: paths.codexPath, claudeModel: paths.claudeModel, codexModel: paths.codexModel }, "Assistant settings saved.")}>Save</button></div>
    </div>

    {status && <div className="assistant-settings__terminal">
      <strong>Use Aurora from your terminal</strong>
      <small>Register Aurora once, then ask Claude Code or Codex about your library from any folder. The library-access switch above still applies.</small>
      {[claudeAddCommand(status.mcpCommand), codexAddCommand(status.mcpCommand)].map((command) => <div key={command} className="assistant-settings__command">
        <code>{command}</code>
        <button type="button" aria-label="Copy command" title="Copy command" onClick={() => void copy(command)}><Copy aria-hidden="true" /></button>
      </div>)}
    </div>}

    <p className="metadata-vault-note">The assistant uses the CLI’s own sign-in, so requests count against that Claude or ChatGPT plan; Aurora stores no AI keys. Tool results that the CLI sends to its provider contain artist, album and track names, genres, ratings and play counts. File paths are never included.</p>
    {preview && <p>Set up the assistant in the desktop app.</p>}
    {message && <p role="status">{message}</p>}
  </section>;
}
