import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface TonehavnStatus {
  server: string;
  signedIn: boolean;
  savedSession: boolean;
  expiresAt: number | null;
  pendingEmailExpiresAt: number | null;
  message: string;
}

type Action = "request_email_login" | "complete_email_login" | "cancel_email_login" | "status" | "logout";

export function TonehavnSettingsPanel() {
  const [status, setStatus] = useState<TonehavnStatus | null>(null);
  const [server, setServer] = useState("");
  const [email, setEmail] = useState("");
  const [link, setLink] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const pending = Boolean(status?.pendingEmailExpiresAt);

  useEffect(() => {
    let cancelled = false;
    void invoke<TonehavnStatus>("tonehavn_status").then(value => {
      if (!cancelled) { setStatus(value); setServer(value.server); }
    }).catch((e: unknown) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setBusy(false); });
    return () => { cancelled = true; };
  }, []);

  async function action(kind: Action) {
    setBusy(true); setError("");
    const request = kind === "request_email_login" ? { server, email: email.trim() }
      : kind === "complete_email_login" ? { link: link.trim() } : undefined;
    setLink("");
    try {
      const next = await invoke<TonehavnStatus>(`tonehavn_${kind}`, request ? { request } : undefined);
      setStatus(next); setServer(next.server);
      if (next.signedIn || kind === "cancel_email_login" || kind === "logout") setEmail("");
    } catch (e) {
      setError(String(e));
      // Reconcile a rejected/uncertain link request or an expired completion with native storage.
      if (kind === "request_email_login" || kind === "complete_email_login") {
        try {
          const next = await invoke<TonehavnStatus>("tonehavn_status");
          setStatus(next); setServer(next.server);
        } catch { /* Keep the original action error if the status check also fails. */ }
      }
    } finally { setBusy(false); }
  }

  return <section className="tonehavn-settings" aria-labelledby="tonehavn-heading">
    <h3 id="tonehavn-heading">Tonehavn account</h3>
    <p>Sign in with your Tonehavn email address. A connected PC can apply whole-star ratings and Love edits.</p>
    <fieldset disabled={busy}>
      <label>Tonehavn server address<input type="url" value={server} disabled={status?.savedSession || pending} autoCapitalize="none" autoCorrect="off" spellCheck={false} placeholder="https://your-pc.ts.net" onChange={e => setServer(e.target.value)} /></label>
      {!status?.savedSession && <>
        {!pending ? <form onSubmit={e => { e.preventDefault(); void action("request_email_login"); }}>
          <label>Tonehavn email<input type="email" autoComplete="email" autoCapitalize="none" autoCorrect="off" spellCheck={false} maxLength={254} required value={email} onChange={e => setEmail(e.target.value)} /></label>
          <button type="submit" className="button button--primary" disabled={!server.trim() || !email.trim()}>Send sign-in link</button>
        </form> : <form onSubmit={e => { e.preventDefault(); void action("complete_email_login"); }}>
          <p>Copy the sign-in link from your email and paste it here. Request the link in Aurora on this computer so it can complete sign-in.</p>
          <label>Sign-in link<input type="password" autoComplete="off" autoCapitalize="none" autoCorrect="off" spellCheck={false} maxLength={2048} required value={link} onChange={e => setLink(e.target.value)} /></label>
          <button type="submit" className="button button--primary" disabled={!link.trim()}>Complete sign-in</button>
          <button type="button" onClick={() => void action("cancel_email_login")}>Start again</button>
          <p>Link expires {new Date(status!.pendingEmailExpiresAt!).toLocaleString()}.</p>
        </form>}
      </>}
      <button type="button" onClick={() => void action("status")}>{status?.savedSession ? "Verify saved session" : "Check connection status"}</button>
      {status?.savedSession && <button type="button" onClick={() => void action("logout")}>Sign out of Tonehavn</button>}
    </fieldset>
    <p role="status">{busy ? "Contacting Tonehavn…" : status?.message || "Not signed in."}</p>
    {status?.signedIn && status.expiresAt && <p>Session expires {new Date(status.expiresAt).toLocaleString()}.</p>}
    <p>Sign-in links last 15 minutes and work once. Aurora keeps the pending request and session in your system credential store.</p>
    {error && <p className="settings-error" role="alert">{error}</p>}
  </section>;
}
