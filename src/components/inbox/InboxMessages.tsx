import { AlertTriangle, CheckCircle2, History, X } from "lucide-react";
import type { InboxMessage } from "./useInboxMessages";

function MessageIcon({ kind }: { kind: InboxMessage["kind"] }) {
  return kind === "error" ? <AlertTriangle aria-label="Error" /> : <CheckCircle2 aria-label="Success" />;
}

export function InboxMessageHistoryButton({ count, open, onToggle }: { count: number; open: boolean; onToggle: () => void }) {
  return <button type="button" aria-label={`Message history (${count})`} aria-expanded={open} aria-controls="inbox-message-history" title="Inbox message history for this session" onClick={onToggle}>
    <History aria-hidden="true" /> History{count > 0 ? <span className="inbox-history-count">{count}</span> : null}
  </button>;
}

export function InboxMessages({ messages, historyOpen, onCloseHistory, onDismiss }: {
  messages: InboxMessage[];
  historyOpen: boolean;
  onCloseHistory: () => void;
  onDismiss: (id: number) => void;
}) {
  return <>
    {messages.filter((message) => message.visible).slice(0, 3).map((message) => <div key={message.id} className={`inbox-banner${message.kind === "success" ? " inbox-banner--success" : ""}`}>
      <span className="inbox-message" role={message.kind === "error" ? "alert" : "status"}><MessageIcon kind={message.kind} /><span>{message.text}</span></span>
      <button type="button" className="inbox-message-close" aria-label="Dismiss message" onClick={() => onDismiss(message.id)}><X aria-hidden="true" /></button>
    </div>)}
    {historyOpen ? <section id="inbox-message-history" className="inbox-message-history" aria-label="Inbox message history">
      <header><div><strong>Message history</strong><small>Latest 100 messages · This session</small></div><button type="button" className="inbox-message-close" aria-label="Close message history" onClick={onCloseHistory}><X aria-hidden="true" /></button></header>
      {messages.length ? <ol>{messages.map((message) => <li key={message.id} className={`inbox-history-entry inbox-history-entry--${message.kind}`}>
        <MessageIcon kind={message.kind} />
        <span>{message.text}</span>
        <time dateTime={new Date(message.createdAt).toISOString()} title={new Date(message.createdAt).toLocaleString()}>{new Date(message.createdAt).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</time>
      </li>)}</ol> : <p>No messages yet. Results from Inbox actions will appear here.</p>}
    </section> : null}
  </>;
}
