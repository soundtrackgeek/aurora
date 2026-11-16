import { useCallback, useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { AlertTriangle, BotMessageSquare, Check, Loader2, RotateCcw, Send, Settings, Square, X } from "lucide-react";
import {
  applyAssistantEvent,
  backendLabels,
  cancelAssistantRun,
  emptyConversation,
  listenToAssistant,
  loadAssistantStatus,
  saveAssistantSettings,
  sendAssistantMessage,
  toolLabels,
  type AssistantBackend,
  type AssistantConversation,
  type AssistantEventEnvelope,
  type AssistantStatus,
  type AssistantTurn,
} from "../assistant";
import { AssistantMarkdown } from "./AssistantMarkdown";
import "./AssistantPanel.css";

const suggestions = [
  "Which genres have inconsistent genre tag spellings?",
  "What did I listen to most last month?",
  "Find albums whose tracks have mixed genres, starting with Synthpop.",
  "Which 1980s albums have I loved the most tracks on?",
  "Find unrated albums that sound like Vangelis in the 1980s.",
  "Look up the genre of the album I have open on Discogs or MusicBrainz.",
];

type AssistantPanelProps = {
  /** What the user has open in Aurora, sent so "these albums" makes sense. */
  context: string;
  onClose: () => void;
  onOpenSettings: () => void;
  /** Runs a search query the assistant suggested. */
  onUseQuery: (query: string) => void;
  /** Genre changes from assistants that wait for review. */
  reviewCount?: number;
  onReviewChanges?: () => void;
};

let turnCounter = 0;
const nextTurnId = () => `turn-${Date.now()}-${(turnCounter += 1)}`;

export function AssistantPanel({ context, onClose, onOpenSettings, onUseQuery, reviewCount = 0, onReviewChanges }: AssistantPanelProps) {
  const [status, setStatus] = useState<AssistantStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [backend, setBackend] = useState<AssistantBackend>("claudeCode");
  const [conversation, setConversation] = useState<AssistantConversation>(emptyConversation);
  const [draft, setDraft] = useState("");
  const [runId, setRunId] = useState<string | null>(null);
  const runRef = useRef<{ runId: string; turnId: string; backend: AssistantBackend } | null>(null);
  const pendingEvents = useRef<AssistantEventEnvelope[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const refreshStatus = useCallback(() => {
    void loadAssistantStatus()
      .then((value) => { setStatus(value); setBackend(value.settings.backend); setStatusError(null); })
      .catch((error) => setStatusError(String(error instanceof Error ? error.message : error)));
  }, []);

  useEffect(() => { refreshStatus(); inputRef.current?.focus(); }, [refreshStatus]);

  const applyEvent = useCallback((event: AssistantEventEnvelope) => {
    const run = runRef.current;
    if (!run || run.runId !== event.runId) return;
    setConversation((current) => {
      const sessions = { ...current.sessions };
      if (event.kind === "session") sessions[run.backend] = event.sessionId;
      if (event.kind === "done" && event.sessionId) sessions[run.backend] = event.sessionId;
      return {
        sessions,
        turns: current.turns.map((turn) => turn.id === run.turnId ? applyAssistantEvent(turn, event) : turn),
      };
    });
    if (event.kind === "done" || event.kind === "error") {
      runRef.current = null;
      setRunId(null);
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let active = true;
    void listenToAssistant((event) => {
      // Events can arrive before `assistant_send` has returned the run id.
      if (!runRef.current?.runId) { pendingEvents.current.push(event); return; }
      applyEvent(event);
    }).then((stop) => { if (active) unlisten = stop; else stop(); });
    return () => { active = false; unlisten?.(); };
  }, [applyEvent]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [conversation]);

  async function send(text: string) {
    const message = text.trim();
    if (!message || runRef.current) return;
    const userTurn: AssistantTurn = { id: nextTurnId(), role: "user", backend, parts: [{ type: "text", text: message }], status: "done" };
    const answerTurn: AssistantTurn = { id: nextTurnId(), role: "assistant", backend, parts: [], status: "running" };
    setConversation((current) => ({ ...current, turns: [...current.turns, userTurn, answerTurn] }));
    setDraft("");
    runRef.current = { runId: "", turnId: answerTurn.id, backend };
    pendingEvents.current = [];
    try {
      const id = await sendAssistantMessage({ message, backend, sessionId: conversation.sessions[backend] ?? null, context });
      runRef.current = { runId: id, turnId: answerTurn.id, backend };
      setRunId(id);
      const early = pendingEvents.current.filter((event) => event.runId === id);
      pendingEvents.current = [];
      early.forEach(applyEvent);
    } catch (error) {
      runRef.current = null;
      setConversation((current) => ({
        ...current,
        turns: current.turns.map((turn) => turn.id === answerTurn.id ? { ...turn, status: "error", error: String(error instanceof Error ? error.message : error) } : turn),
      }));
    }
  }

  function submit(event: FormEvent) { event.preventDefault(); void send(draft); }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void send(draft); }
  }

  async function chooseBackend(next: AssistantBackend) {
    setBackend(next);
    if (status) {
      try { setStatus(await saveAssistantSettings({ ...status.settings, backend: next })); } catch { /* The choice still applies to this panel. */ }
    }
  }

  function newConversation() {
    if (runRef.current?.runId) void cancelAssistantRun(runRef.current.runId);
    runRef.current = null;
    setRunId(null);
    setConversation(emptyConversation);
    inputRef.current?.focus();
  }

  const accessOff = status !== null && !status.settings.libraryAccess;
  const cli = backend === "claudeCode" ? status?.claude : status?.codex;
  const busy = runId !== null || runRef.current !== null;

  return <aside className="assistant-panel" aria-label="Assistant">
    <header className="assistant-panel__header">
      <BotMessageSquare aria-hidden="true" />
      <div><strong>Assistant</strong><small>{status?.settings.appControl ? "Controls Aurora" : "Read-only"}{status?.settings.webAccess ? " · Web" : ""} · {(backend === "claudeCode" ? status?.settings.claudeModel : status?.settings.codexModel) || "CLI default model"}</small></div>
      <select aria-label="Assistant backend" value={backend} disabled={busy} onChange={(event) => void chooseBackend(event.target.value as AssistantBackend)}>
        {(["claudeCode", "codex"] as const).map((value) => <option key={value} value={value}>{backendLabels[value]}</option>)}
      </select>
      <button type="button" aria-label="New conversation" title="New conversation" disabled={!conversation.turns.length} onClick={newConversation}><RotateCcw aria-hidden="true" /></button>
      <button type="button" aria-label="Assistant settings" title="Assistant settings" onClick={onOpenSettings}><Settings aria-hidden="true" /></button>
      <button type="button" aria-label="Close assistant" title="Close assistant" onClick={onClose}><X aria-hidden="true" /></button>
    </header>

    <div className="assistant-panel__scroll" ref={scrollRef}>
      {statusError && <p className="assistant-panel__notice" role="alert"><AlertTriangle aria-hidden="true" /> {statusError}</p>}
      {accessOff && <div className="assistant-panel__notice">
        <AlertTriangle aria-hidden="true" />
        <span>Turn on <strong>Let AI assistants read my library</strong> to use the Assistant.</span>
        <button type="button" className="button" onClick={onOpenSettings}>Open settings</button>
      </div>}
      {status && !accessOff && !cli?.path && <div className="assistant-panel__notice">
        <AlertTriangle aria-hidden="true" />
        <span>Aurora could not find {backendLabels[backend]}. Install it, or choose its executable in settings.</span>
        <button type="button" className="button" onClick={onOpenSettings}>Open settings</button>
      </div>}
      {reviewCount > 0 && onReviewChanges && <div className="assistant-panel__notice">
        <span>{reviewCount === 1 ? "1 album has" : `${reviewCount} albums have`} genre changes waiting for your review.</span>
        <button type="button" className="button" onClick={onReviewChanges}>Review changes</button>
      </div>}
      {!conversation.turns.length && <div className="assistant-panel__empty">
        <p>Ask about your genres, ratings, listening history, albums and artists. {backendLabels[backend]} answers with your own sign-in.</p>
        <ul>{suggestions.map((suggestion) => <li key={suggestion}><button type="button" disabled={accessOff || busy} onClick={() => void send(suggestion)}>{suggestion}</button></li>)}</ul>
      </div>}
      {conversation.turns.map((turn) => <article key={turn.id} className={`assistant-turn assistant-turn--${turn.role}`} aria-busy={turn.status === "running"}>
        {turn.parts.map((part, index) => part.type === "text"
          ? <div key={index} className="assistant-md">{turn.role === "user" ? <p>{part.text}</p> : <AssistantMarkdown text={part.text} onCode={onUseQuery} />}</div>
          : <p key={part.id || index} className={`assistant-tool assistant-tool--${part.status}`}>
            {part.status === "running" ? <Loader2 aria-hidden="true" /> : part.status === "done" ? <Check aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
            {toolLabels[part.name] ?? part.name}
          </p>)}
        {turn.role === "assistant" && turn.status === "running" && !turn.parts.length && <p className="assistant-tool assistant-tool--running"><Loader2 aria-hidden="true" /> Starting {backendLabels[turn.backend]}…</p>}
        {turn.status === "error" && <p className="assistant-turn__error" role="alert">{turn.error}</p>}
      </article>)}
    </div>

    <form className="assistant-panel__composer" onSubmit={submit}>
      <textarea ref={inputRef} rows={2} value={draft} placeholder={accessOff ? "Library access is off" : "Ask about your library…"} disabled={accessOff} onChange={(event) => setDraft(event.target.value)} onKeyDown={onKeyDown} aria-label="Message the assistant" />
      {busy
        ? <button type="button" aria-label="Stop" title="Stop" onClick={() => { if (runRef.current?.runId) void cancelAssistantRun(runRef.current.runId); }}><Square aria-hidden="true" /></button>
        : <button type="submit" aria-label="Send" title="Send (Enter)" disabled={accessOff || !draft.trim()}><Send aria-hidden="true" /></button>}
    </form>
  </aside>;
}
