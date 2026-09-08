import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface TonehavnStatus {
  server: string;
  signedIn: boolean;
  savedSession: boolean;
  expiresAt: number | null;
  message: string;
}

export function TonehavnSettingsPanel() {
  const [status, setStatus] = useState<TonehavnStatus | null>(null);
  const [server, setServer] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [totpCode, setTotpCode] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    let cancelled = false;
    void invoke<TonehavnStatus>("tonehavn_status").then(value => {
      if (!cancelled) { setStatus(value); setServer(value.server); }
    }).catch((e: unknown) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setBusy(false); });
    return () => { cancelled = true; };
  }, []);

  async function action(kind: "login" | "status" | "logout") {
    setBusy(true); setError("");
    const request = kind === "login" ? { server, username, password, totpCode: totpCode.trim() } : undefined;
    setPassword(""); setTotpCode("");
    try {
      const next = await invoke<TonehavnStatus>(`tonehavn_${kind}`, request ? { request } : undefined);
      setStatus(next); setServer(next.server);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  return <section className="tonehavn-settings" aria-labelledby="tonehavn-heading">
    <h3 id="tonehavn-heading">Tonehavn account</h3>
    <p>Connect Aurora as a trusted device using your Tonehavn account. A connected PC can apply whole-star ratings and Love edits. Other tag edits remain on the PC.</p>
    <fieldset disabled={busy}>
      <label>Tonehavn server address<input type="url" value={server} disabled={status?.savedSession} autoCapitalize="none" autoCorrect="off" spellCheck={false} placeholder="https://your-pc.ts.net" onChange={e => setServer(e.target.value)} /></label>
      {!status?.savedSession && <>
        <label>Tonehavn username<input autoComplete="username" autoCapitalize="none" autoCorrect="off" spellCheck={false} value={username} onChange={e => setUsername(e.target.value)} /></label>
        <label>Tonehavn password<input type="password" autoComplete="current-password" value={password} onChange={e => setPassword(e.target.value)} /></label>
        <label>Authenticator code<input inputMode="numeric" autoComplete="one-time-code" maxLength={6} value={totpCode} onChange={e => setTotpCode(e.target.value)} /></label>
        <button type="button" className="button button--primary" disabled={!server.trim() || !username.trim() || !password || !/^\d{6}$/.test(totpCode.trim())} onClick={() => void action("login")}>Sign in to Tonehavn</button>
      </>}
      <button type="button" onClick={() => void action("status")}>{status?.savedSession ? "Verify saved session" : "Check connection status"}</button>
      {status?.savedSession && <button type="button" onClick={() => void action("logout")}>Sign out of Tonehavn</button>}
    </fieldset>
    <p role="status">{busy ? "Contacting Tonehavn…" : status?.message || "Not signed in."}</p>
    {status?.signedIn && status.expiresAt && <p>Session expires {new Date(status.expiresAt).toLocaleString()}.</p>}
    <p>The session is kept in your system credential store. Aurora does not save your password or authenticator code.</p>
    {error && <p className="settings-error" role="alert">{error}</p>}
  </section>;
}
