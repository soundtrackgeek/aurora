import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCircle2, FolderOpen, RefreshCw, TriangleAlert, X } from "lucide-react";
import { formatCount } from "../library";
import {
  loadLibrarySyncFolders,
  retryLibrarySyncFolder,
  revealLibrarySyncFolder,
  type CatalogSync,
  type LibrarySyncFolder,
} from "../tags";
import { describeLibrarySyncFailure, describeNextRetry } from "../app/catalog/librarySyncFailure";
import "./LibrarySyncDialog.css";

export interface LibrarySyncDialogProps {
  onClose: () => void;
  /** Receives the sync status after a retry so the topbar notice stays current. */
  onCatalogSync: (sync: CatalogSync) => void;
  /** Injected in tests so retry times read the same on every run. */
  now?: () => number;
}

type RowState = { status: "retrying" } | { status: "synced" } | { status: "error"; message: string };

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function folderName(directory: string): string {
  return directory.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || directory;
}

export function LibrarySyncDialog({ onClose, onCatalogSync, now = Date.now }: LibrarySyncDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const mountedRef = useRef(true);
  const [folders, setFolders] = useState<LibrarySyncFolder[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [rows, setRows] = useState<Record<string, RowState>>({});

  const reload = useCallback(() => loadLibrarySyncFolders()
    .then((next) => {
      if (!mountedRef.current) return;
      setFolders(next);
      setLoadError(null);
    })
    .catch((error: unknown) => {
      if (mountedRef.current) setLoadError(errorText(error));
    }), []);

  useEffect(() => {
    mountedRef.current = true;
    dialogRef.current?.showModal();
    void reload();
    return () => { mountedRef.current = false; };
  }, [reload]);

  const setRow = (directory: string, state: RowState | null) => setRows((current) => {
    const next = { ...current };
    if (state) next[directory] = state;
    else delete next[directory];
    return next;
  });

  const retry = async (directory: string) => {
    setRow(directory, { status: "retrying" });
    try {
      const result = await retryLibrarySyncFolder(directory);
      if (!mountedRef.current) return;
      onCatalogSync(result.catalogSync);
      // A failed retry stores a fresh reason, which the reloaded row shows.
      setRow(directory, result.completed ? { status: "synced" } : null);
    } catch (error) {
      if (mountedRef.current) setRow(directory, { status: "error", message: errorText(error) });
    }
    await reload();
  };

  const reveal = async (directory: string) => {
    try {
      await revealLibrarySyncFolder(directory);
    } catch (error) {
      setRow(directory, { status: "error", message: errorText(error) });
    }
  };

  const retrying = Object.values(rows).some((row) => row.status === "retrying");
  const blocked = folders?.filter((folder) => folder.blocked) ?? [];
  const waiting = folders?.filter((folder) => !folder.blocked) ?? [];
  const editCount = folders?.reduce((total, folder) => total + folder.pendingEditCount, 0) ?? 0;
  const nowMs = now();

  const renderFolder = (folder: LibrarySyncFolder) => {
    const failure = describeLibrarySyncFailure(folder.lastError, folder.directory);
    const row = rows[folder.directory];
    return (
      <li key={folder.directory} className="library-sync-folder" data-blocked={folder.blocked ? "true" : undefined}>
        <div className="library-sync-folder__title">
          <strong title={folder.directory}>{folderName(folder.directory)}</strong>
          <small>{folder.directory}</small>
        </div>
        <span className="library-sync-folder__edits">
          {folder.pendingEditCount > 0
            ? `${formatCount(folder.pendingEditCount)} ${folder.pendingEditCount === 1 ? "edit" : "edits"} waiting`
            : "Folder change waiting"}
        </span>
        <p className="library-sync-folder__reason">{failure.reason}</p>
        {failure.file && <p className="library-sync-folder__file" title={failure.file}>{failure.file}</p>}
        <p className="library-sync-folder__fix">{failure.fix}</p>
        <p className="library-sync-folder__next">{describeNextRetry(folder.nextRetryAtMs, nowMs)}</p>
        {row?.status === "error" && <p className="library-sync-folder__error" role="alert">{row.message}</p>}
        <div className="library-sync-folder__actions">
          <button type="button" onClick={() => void reveal(folder.directory)} aria-label={`Open folder ${folderName(folder.directory)}`}>
            <FolderOpen aria-hidden="true" /> Open folder
          </button>
          <button type="button" className="library-sync-folder__retry" disabled={retrying}
            onClick={() => void retry(folder.directory)} aria-label={`Retry ${folderName(folder.directory)}`}>
            <RefreshCw aria-hidden="true" className={row?.status === "retrying" ? "is-spinning" : undefined} />
            {row?.status === "retrying" ? "Retrying…" : "Retry"}
          </button>
        </div>
      </li>
    );
  };

  const synced = Object.entries(rows).filter(([directory, row]) => (
    row.status === "synced" && !folders?.some((folder) => folder.directory === directory)
  ));

  return (
    <dialog ref={dialogRef} className="library-sync-dialog" aria-labelledby="library-sync-dialog-title"
      onCancel={(event) => { event.preventDefault(); onClose(); }}>
      <header>
        <div>
          <p className="eyebrow">Music Library</p>
          <h2 id="library-sync-dialog-title">Edits waiting for Music Library</h2>
          <p>
            {folders === null
              ? "Loading folders…"
              : folders.length === 0
                ? "Every saved edit is in Music Library."
                : `${formatCount(folders.length)} ${folders.length === 1 ? "folder" : "folders"} · ${formatCount(editCount)} ${editCount === 1 ? "edit" : "edits"} waiting. Your MP3 files already have every change.`}
          </p>
        </div>
        <button type="button" aria-label="Close" onClick={onClose}><X aria-hidden="true" /></button>
      </header>
      <div className="library-sync-dialog__body">
        {loadError && <p className="library-sync-folder__error" role="alert">{loadError}</p>}
        {synced.length > 0 && (
          <p className="library-sync-dialog__synced" role="status">
            <CheckCircle2 aria-hidden="true" />
            {synced.length === 1 ? `${folderName(synced[0][0])} is synced.` : `${formatCount(synced.length)} folders synced.`}
          </p>
        )}
        {blocked.length > 0 && (
          <section aria-label="Needs your attention">
            <h3><TriangleAlert aria-hidden="true" /> Needs your attention</h3>
            <ul>{blocked.map(renderFolder)}</ul>
          </section>
        )}
        {waiting.length > 0 && (
          <section aria-label="Retrying automatically">
            <h3><RefreshCw aria-hidden="true" /> Retrying automatically</h3>
            <ul>{waiting.map(renderFolder)}</ul>
          </section>
        )}
      </div>
    </dialog>
  );
}
