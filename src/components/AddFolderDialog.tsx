import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  Film,
  FolderOpen,
  FolderPlus,
  LoaderCircle,
  Music2,
  ShieldCheck,
  Waves,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  libraryIntakeAdapter,
  libraryIntakeCategories,
  type LibraryBridgeCapabilities,
  type LibraryIntakeAdapter,
  type LibraryIntakeApplyResult,
  type LibraryIntakeCategoryId,
  type LibraryIntakePreview,
} from "../ingest";
import "./AddFolderDialog.css";

type BusyAction = "capabilities" | "selecting" | "previewing" | "applying" | null;

interface AddFolderDialogProps {
  onClose: () => void;
  onCatalogChanged: () => boolean | void | Promise<boolean | void>;
  adapter?: LibraryIntakeAdapter;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function leafName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function CategoryIcon({ category }: { category: LibraryIntakeCategoryId }) {
  if (category === "scores") return <Film aria-hidden="true" />;
  if (category === "synthwave") return <Waves aria-hidden="true" />;
  return <Music2 aria-hidden="true" />;
}

export function AddFolderDialog({
  onClose,
  onCatalogChanged,
  adapter = libraryIntakeAdapter,
}: AddFolderDialogProps) {
  const [capabilities, setCapabilities] = useState<LibraryBridgeCapabilities | null>(null);
  const [sourcePath, setSourcePath] = useState<string | null>(null);
  const [category, setCategory] = useState<LibraryIntakeCategoryId | null>(null);
  const [preview, setPreview] = useState<LibraryIntakePreview | null>(null);
  const [result, setResult] = useState<LibraryIntakeApplyResult | null>(null);
  const [busyAction, setBusyAction] = useState<BusyAction>("capabilities");
  const [error, setError] = useState<string | null>(null);
  const [refreshWarning, setRefreshWarning] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const requestGenerationRef = useRef(0);
  const dialogRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const isBusy = busyAction === "selecting" || busyAction === "previewing" || busyAction === "applying";

  useEffect(() => {
    let current = true;
    void adapter.capabilities()
      .then((value) => {
        if (!current) return;
        setCapabilities(value);
        setError(null);
      })
      .catch((nextError: unknown) => {
        if (!current) return;
        setError(errorMessage(nextError));
      })
      .finally(() => {
        if (current) setBusyAction(null);
      });
    return () => { current = false; };
  }, [adapter]);

  useEffect(() => {
    closeButtonRef.current?.focus();
  }, []);

  useEffect(() => {
    function handleDialogKeyboard(event: KeyboardEvent) {
      if (event.key === "Escape") {
        if (isBusy) return;
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== "Tab") return;
      const dialog = dialogRef.current;
      if (!dialog) return;
      const focusable = [...dialog.querySelectorAll<HTMLElement>(
        "button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex='-1'])",
      )].filter((element) => !element.hasAttribute("hidden"));
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) {
        event.preventDefault();
        first.focus();
      }
    }
    window.addEventListener("keydown", handleDialogKeyboard);
    return () => window.removeEventListener("keydown", handleDialogKeyboard);
  }, [isBusy, onClose]);

  const invalidatePreview = useCallback(() => {
    requestGenerationRef.current += 1;
    setPreview(null);
    setResult(null);
    setConfirming(false);
    setRefreshWarning(null);
  }, []);

  const selectedCapability = useMemo(
    () => capabilities?.categories.find((item) => item.id === category) ?? null,
    [capabilities, category],
  );
  const companionReady = Boolean(
    capabilities
    && capabilities.bridgeVersion === 1
    && capabilities.supports.previewRequired
    && capabilities.supports.batchFolders,
  );
  const previewProblems = [
    ...(preview?.errors ?? []),
    ...(preview?.conflicts ?? []),
  ];
  const canPreview = Boolean(
    companionReady
    && sourcePath
    && selectedCapability?.available
    && !isBusy,
  );
  const canApply = Boolean(
    preview?.canApply
    && previewProblems.length === 0
    && !isBusy,
  );

  async function chooseSourceFolder() {
    if (isBusy) return;
    setBusyAction("selecting");
    setError(null);
    try {
      const selected = await adapter.selectFolder();
      if (selected) {
        setSourcePath(selected);
        invalidatePreview();
      }
    } catch (nextError) {
      setError(errorMessage(nextError));
    } finally {
      setBusyAction(null);
    }
  }

  function selectCategory(nextCategory: LibraryIntakeCategoryId) {
    if (nextCategory === category || isBusy) return;
    setCategory(nextCategory);
    setError(null);
    invalidatePreview();
  }

  async function createPreview() {
    if (!sourcePath || !category || !canPreview) return;
    const generation = ++requestGenerationRef.current;
    setPreview(null);
    setResult(null);
    setBusyAction("previewing");
    setError(null);
    setConfirming(false);
    try {
      const nextPreview = await adapter.preview({ sourcePath, category });
      if (generation !== requestGenerationRef.current) return;
      setPreview(nextPreview);
    } catch (nextError) {
      if (generation === requestGenerationRef.current) setError(errorMessage(nextError));
    } finally {
      if (generation === requestGenerationRef.current) setBusyAction(null);
    }
  }

  async function applyPreview() {
    if (!preview || !canApply) return;
    setBusyAction("applying");
    setError(null);
    setRefreshWarning(null);
    try {
      const nextResult = await adapter.apply({
        planId: preview.planId,
        sessionId: preview.sessionId,
      });
      setConfirming(false);
      try {
        const refreshed = await onCatalogChanged();
        if (refreshed === false) {
          setRefreshWarning("The batch completed, but Aurora has not detected the new catalog revision yet. Automatic refresh will keep trying.");
        }
      } catch (refreshError) {
        setRefreshWarning(`The batch completed, but Aurora could not refresh immediately: ${errorMessage(refreshError)}`);
      }
      setResult(nextResult);
    } catch (nextError) {
      setError(errorMessage(nextError));
    } finally {
      setBusyAction(null);
    }
  }

  const retainedAlbumCount = result?.albums.filter((album) => album.cleanupStatus === "retained").length ?? 0;
  const removedSourceAlbumCount = result?.albums.filter((album) => album.cleanupStatus === "removed").length ?? 0;
  const fullyMovedAlbumCount = result ? Math.min(result.movedAlbumCount, removedSourceAlbumCount) : 0;
  const cleanupWarnings = result
    ? [
        ...(retainedAlbumCount > 0
          ? [`${retainedAlbumCount} source album ${retainedAlbumCount === 1 ? "folder was" : "folders were"} retained and must be cleaned up manually.`]
          : []),
        ...result.cleanupWarnings,
      ]
    : [];

  return (
    <div
      className="modal-backdrop add-folder-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !isBusy) onClose();
      }}
    >
      <section
        ref={dialogRef}
        className="add-folder-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-folder-title"
        aria-describedby="add-folder-description"
        aria-busy={isBusy}
      >
        <header className="add-folder-dialog__header">
          <div className="add-folder-dialog__mark"><FolderPlus aria-hidden="true" /></div>
          <div>
            <p className="eyebrow">Library intake</p>
            <h2 id="add-folder-title">Add music folders</h2>
            <p id="add-folder-description">Move already-tagged albums into your library and catalog them in one reviewed batch.</p>
          </div>
          <button ref={closeButtonRef} type="button" aria-label="Close add music" disabled={isBusy} onClick={onClose}><X aria-hidden="true" /></button>
        </header>

        <div className="add-folder-dialog__body">
          <section className={`bridge-status${companionReady ? " is-ready" : " is-unavailable"}`} aria-live="polite">
            {busyAction === "capabilities" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : companionReady ? <ShieldCheck aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
            <span>
              <strong>{busyAction === "capabilities" ? "Checking Music Library companion…" : companionReady ? "Music Library companion ready" : "Music Library companion unavailable"}</strong>
              <small>{companionReady ? `Bridge v${capabilities?.bridgeVersion} · preview and rollback protection enabled` : error ?? "Open native Aurora and make sure the Music Library companion is installed."}</small>
            </span>
          </section>

          <section className="intake-step" aria-labelledby="intake-source-heading">
            <div className="intake-step__heading"><span>1</span><div><h3 id="intake-source-heading">Choose your intake folder</h3><p>Select one tagged album folder, or a parent folder containing many tagged albums.</p></div></div>
            <div className="intake-source">
              <FolderOpen aria-hidden="true" />
              <span title={sourcePath ?? undefined}>
                <strong>{sourcePath ? leafName(sourcePath) : "No folder selected"}</strong>
                <small>{sourcePath ?? "Your source is never changed during preview."}</small>
              </span>
              <button type="button" disabled={!companionReady || isBusy} onClick={() => void chooseSourceFolder()}>
                {busyAction === "selecting" ? "Choosing…" : sourcePath ? "Change" : "Choose folder"}
              </button>
            </div>
          </section>

          <section className="intake-step" aria-labelledby="intake-category-heading">
            <div className="intake-step__heading"><span>2</span><div><h3 id="intake-category-heading">Choose one library destination</h3><p>This changes the destination only. Aurora keeps your tags, filenames, and album folder names unchanged.</p></div></div>
            <fieldset className="intake-categories">
              <legend className="sr-only">Library destination</legend>
              {libraryIntakeCategories.map((option) => {
                const capability = capabilities?.categories.find((item) => item.id === option.id);
                const unavailable = Boolean(capabilities && !capability?.available);
                return (
                  <label key={option.id} className={category === option.id ? "is-selected" : ""}>
                    <input
                      type="radio"
                      name="library-intake-category"
                      value={option.id}
                      checked={category === option.id}
                      disabled={!companionReady || unavailable || isBusy}
                      onChange={() => selectCategory(option.id)}
                    />
                    <span className="intake-category__icon"><CategoryIcon category={option.id} /></span>
                    <span>
                      <strong>{option.label}</strong>
                      <small>{option.description}</small>
                      <code title={capability?.destinationRoot}>{capability?.destinationRoot ?? "Resolving destination…"}</code>
                    </span>
                  </label>
                );
              })}
            </fieldset>
          </section>

          {error && companionReady ? <p className="intake-alert intake-alert--error" role="alert"><AlertTriangle aria-hidden="true" /> {error}</p> : null}

          {preview ? (
            <section className="intake-preview" aria-labelledby="intake-preview-heading">
              <header>
                <div><p className="eyebrow">Verified plan</p><h3 id="intake-preview-heading">{preview.albumCount} {preview.albumCount === 1 ? "album" : "albums"} · {preview.trackCount} tracks</h3></div>
                <code title={preview.category.destinationRoot}>{preview.category.destinationRoot}</code>
              </header>
              <div className="intake-delta" aria-label="Exact catalog changes">
                <span><strong>+{preview.delta.addedTracks}</strong> tracks</span>
                <span><strong>{preview.delta.changedTracks}</strong> changed</span>
                <span><strong>−{preview.delta.removedTracks}</strong> removed</span>
                <span><strong>+{preview.delta.addedAlbums}</strong> albums</span>
                <span><strong>{preview.delta.changedAlbums}</strong> changed</span>
                <span><strong>−{preview.delta.removedAlbums}</strong> removed</span>
              </div>
              <ol className="intake-album-list" aria-label="Album moves">
                {preview.albums.map((album) => (
                  <li key={`${album.sourcePath}\n${album.destinationPath}`}>
                    <span><strong>{album.album}</strong><small>{album.artist} · {album.year || "Year unknown"} · {album.trackCount} tracks</small></span>
                    <span className="intake-album-paths"><code title={album.sourcePath}>{album.sourcePath}</code><ArrowRight aria-label="moves to" /><code title={album.destinationPath}>{album.destinationPath}</code></span>
                  </li>
                ))}
              </ol>
              {(preview.suspiciousFlags?.length ?? 0) > 0 ? (
                <div className="intake-alert intake-alert--warning" role="status"><AlertTriangle aria-hidden="true" /><span><strong>Review these suspicious details</strong>{preview.suspiciousFlags?.map((flag) => <small key={flag}>{flag}</small>)}</span></div>
              ) : null}
              {previewProblems.length > 0 ? (
                <div className="intake-alert intake-alert--error" role="alert"><AlertTriangle aria-hidden="true" /><span><strong>This plan cannot be applied</strong>{previewProblems.map((problem) => <small key={problem}>{problem}</small>)}</span></div>
              ) : null}
            </section>
          ) : null}

          {confirming && preview ? (
            <section className="intake-confirmation" role="alertdialog" aria-label="Confirm album move">
              <ShieldCheck aria-hidden="true" />
              <span><strong>Move and catalog {preview.albumCount} {preview.albumCount === 1 ? "album" : "albums"}?</strong><small>Destination root: {preview.category.destinationRoot}</small></span>
              <button type="button" className="button button--primary" onClick={() => void applyPreview()}>Move and catalog {preview.albumCount}</button>
              <button type="button" className="button button--quiet" onClick={() => setConfirming(false)}>Not yet</button>
            </section>
          ) : null}

          {busyAction === "applying" ? <p className="intake-progress" role="status"><LoaderCircle className="is-spinning" aria-hidden="true" /> Moving albums, verifying copies, and updating the catalog… Keep Aurora open.</p> : null}

          {result ? (
            <section className={`intake-result${cleanupWarnings.length > 0 || refreshWarning ? " has-warnings" : ""}`} role="status">
              {cleanupWarnings.length > 0 || refreshWarning ? <AlertTriangle aria-hidden="true" /> : <CheckCircle2 aria-hidden="true" />}
              <span>
                <strong>{fullyMovedAlbumCount} of {result.albumCount} {result.albumCount === 1 ? "album" : "albums"} fully moved · {result.albumCount} cataloged · {result.trackCount} tracks</strong>
                <small>{cleanupWarnings.length > 0 ? "The catalog is updated, but source cleanup needs attention." : refreshWarning ? "Source folders were removed after verification; Aurora refresh is still pending." : "Source folders were removed after verification. Aurora refreshed the library."}</small>
                {cleanupWarnings.map((warning) => <small key={warning} className="intake-result__warning">{warning}</small>)}
                {refreshWarning ? <small className="intake-result__warning">{refreshWarning}</small> : null}
              </span>
            </section>
          ) : null}
        </div>

        <footer className="add-folder-dialog__footer">
          <span>{selectedCapability?.available ? `Destination: ${selectedCapability.destinationRoot}` : "Choose a source and one destination to continue."}</span>
          <button type="button" className="button button--quiet" disabled={isBusy} onClick={onClose}>{result ? "Done" : "Cancel"}</button>
          {!result ? (
            preview ? (
              <>
                <button type="button" className="button button--quiet" disabled={!canPreview} onClick={() => void createPreview()}>Preview again</button>
                <button type="button" className="button button--primary" disabled={!canApply || confirming} onClick={() => setConfirming(true)}>Review apply</button>
              </>
            ) : (
              <button type="button" className="button button--primary" disabled={!canPreview} onClick={() => void createPreview()}>{busyAction === "previewing" ? "Previewing…" : "Preview batch"}</button>
            )
          ) : null}
        </footer>
      </section>
    </div>
  );
}
