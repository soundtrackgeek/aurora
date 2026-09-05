import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FolderOutput, LoaderCircle } from "lucide-react";
import { libraryIntakeAdapter, previewLibraryRemoveAlbum, type LibraryIntakePreview } from "../../ingest";

export function RemoveAlbumButton({ album, onRemoved }: {
  album: { id: string; title: string };
  onRemoved: (albumId: string, warnings: string[]) => void | Promise<void>;
}) {
  const [preview, setPreview] = useState<LibraryIntakePreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (preview && dialog.current && !dialog.current.open) dialog.current.showModal();
  }, [preview]);

  async function prepare() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      setPreview(await previewLibraryRemoveAlbum(album.id));
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!preview?.canApply || busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await libraryIntakeAdapter.apply({ planId: preview.planId, sessionId: preview.sessionId });
      setPreview(null);
      await onRemoved(album.id, result.cleanupWarnings);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  return <div className="remove-album-action">
    <button type="button" className="deep-explorer-move-inbox" disabled={busy} onClick={() => void prepare()}>
      <FolderOutput aria-hidden="true" />{busy && !preview ? "Preparing…" : "Remove Album"}
    </button>
    {!preview && error ? <p role="alert">{error}</p> : null}
    {preview ? createPortal(<dialog ref={dialog} className="deep-explorer-delete-dialog deep-explorer-move-dialog" aria-labelledby="remove-album-title" aria-describedby="remove-album-description" onCancel={(event) => {
      event.preventDefault();
      if (!busy) setPreview(null);
    }}>
      <span className="deep-explorer-delete-dialog__icon"><FolderOutput aria-hidden="true" /></span>
      <div>
        <h4 id="remove-album-title">Remove “{album.title}”?</h4>
        <p id="remove-album-description">Move the complete folder from <strong>{preview.sourcePath}</strong> to <strong>{preview.albums[0]?.destinationPath}</strong> and remove its {preview.trackCount} tracks from Music Library. Aurora verifies the copy before updating the database, then removes the original folder. This album will disappear from Albums.</p>
        {!preview.canApply ? <p role="alert">This removal preview is blocked. Cancel and prepare it again after resolving the catalog conflict.</p> : null}
        {error ? <p className="deep-explorer-delete-dialog__error" role="alert">{error}</p> : null}
      </div>
      <div className="deep-explorer-delete-dialog__actions">
        <button type="button" autoFocus disabled={busy} onClick={() => setPreview(null)}>Cancel</button>
        <button type="button" disabled={busy || !preview.canApply} onClick={() => void apply()}>
          {busy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <FolderOutput aria-hidden="true" />}{busy ? "Removing…" : "Remove Album"}
        </button>
      </div>
    </dialog>, document.body) : null}
  </div>;
}
