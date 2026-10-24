import { useCallback, useEffect, useRef, useState } from "react";

export interface InboxMessage {
  id: number;
  text: string;
  kind: "success" | "error";
  createdAt: number;
  visible: boolean;
}

const messageLifetime = 5_000;
const historyLimit = 100;

export function useInboxMessages() {
  const [messages, setMessages] = useState<InboxMessage[]>([]);
  const nextId = useRef(0);

  const notify = useCallback((text: string, kind: InboxMessage["kind"] = "success") => {
    const message: InboxMessage = { id: ++nextId.current, text, kind, createdAt: Date.now(), visible: true };
    setMessages((current) => [message, ...current].slice(0, historyLimit));
  }, []);

  const dismiss = useCallback((id: number) => {
    setMessages((current) => current.map((message) => message.id === id ? { ...message, visible: false } : message));
  }, []);

  const nextExpiry = messages.reduce((earliest, message) => message.visible
    ? Math.min(earliest, message.createdAt + messageLifetime)
    : earliest, Infinity);

  useEffect(() => {
    if (!Number.isFinite(nextExpiry)) return;
    const timer = window.setTimeout(() => {
      const now = Date.now();
      setMessages((current) => current.map((message) => message.visible && message.createdAt + messageLifetime <= now
        ? { ...message, visible: false }
        : message));
    }, Math.max(0, nextExpiry - Date.now()));
    return () => window.clearTimeout(timer);
  }, [nextExpiry]);

  return { messages, notify, dismiss };
}
