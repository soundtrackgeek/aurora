import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FolderOutput, LoaderCircle } from "lucide-react";
import { libraryIntakeAdapter, previewLibraryRemoveAlbum, type LibraryIntakePreview } from "../../ingest";
import { loadInboxSettings } from "../../inbox";

export interface AlbumMoveRequest {
  album: { id: string; title: string };
  mode: "remove" | "inbox";
}

// Mounted by App, independently of the selected album, inspector, and navigation view.
export function AlbumMoveOperation({ request, onDismiss, onRemoved }: {
  request: AlbumMoveRequest;
  onDismiss: () => void;
  onRemoved: (albumId: string, warnings: string[], destination: string) => Promise<void>;
}) {
  const [preview, setPreview] = useState<LibraryIntakePreview | null>(null);
  const [stage, setStage] = useState<"preparing" | "ready" | "moving" | "completed" | "failed">("preparing");
  const [message, setMessage] = useState("Preparing move · you can keep browsing");
  const [review, setReview] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const applying = useRef(false);
  const preparation = useRef<Promise<LibraryIntakePreview> | null>(null);
  const label = request.mode === "remove" ? "Remove Album" : "Move to Inbox";

  useEffect(() => {
    let active = true;
    // Keep StrictMode effect replay from issuing a second native preview.
    preparation.current ??= (async () => {
      if (request.mode === "remove") return previewLibraryRemoveAlbum(request.album.id);
      const settings = await loadInboxSettings();
      if (!settings.monitoredFolders.length) throw new Error("Add a monitored Inbox folder before moving an album.");
      const inboxPath = settings.monitoredFolders.length === 1
        ? settings.monitoredFolders[0] : await libraryIntakeAdapter.selectFolder();
      if (!inboxPath) throw new Error("No Inbox folder was selected. No files were moved.");
      const normalized = (path: string) => path.replace(/^\\\\\?\\/, "").replace(/\//g, "\\").replace(/\\+$/, "").toLocaleLowerCase();
      if (!settings.monitoredFolders.some((folder) => normalized(folder) === normalized(inboxPath))) {
        throw new Error("Choose a monitored folder configured in Aurora Inbox.");
      }
      return libraryIntakeAdapter.previewMoveToInbox({ albumId: request.album.id, inboxPath });
    })();
    void preparation.current.then((result) => {
      if (!active) return;
      setPreview(result);
      setStage("ready");
      setMessage(result.canApply ? "Ready for confirmation · no files moved yet" : "The preview is blocked. No files moved.");
    }).catch((error: unknown) => {
      if (!active) return;
      setStage("failed");
      setMessage(error instanceof Error ? error.message : String(error));
    });
    return () => { active = false; };
  }, [request]);

  useEffect(() => {
    if (review && dialog.current && !dialog.current.open) dialog.current.showModal();
  }, [review]);

  async function apply() {
    if (!preview?.canApply || applying.current) return;
    applying.current = true;
    setReview(false);
    setStage("moving");
    setMessage("Moving and verifying files · you can keep browsing");
    try {
      const result = await libraryIntakeAdapter.apply({ planId: preview.planId, sessionId: preview.sessionId });
      const destination = preview.albums[0]?.destinationPath ?? preview.category.destinationRoot;
      // Report the committed operation even if refreshing the UI fails; never offer to apply it again.
      try {
        await onRemoved(request.album.id, result.cleanupWarnings, destination);
        setStage(result.cleanupWarnings.length ? "failed" : "completed");
        setMessage(result.cleanupWarnings.length ? `Album removed from the catalog. ${result.cleanupWarnings.join(" ")}` : `Moved to ${destination} · removed from Albums`);
      } catch (error) {
        setStage("failed");
        setMessage(`Album moved and catalog updated, but Aurora could not refresh: ${error instanceof Error ? error.message : String(error)}`);
      }
    } catch (error) {
      setStage("failed");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  const busy = stage === "preparing" || stage === "moving";
  return createPortal(<>
    <section className="album-move-operation" role={stage === "failed" ? "alert" : "status"} aria-label={`${label}: ${request.album.title}`}>
      <strong>{busy ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <FolderOutput aria-hidden="true" />}{label} · {request.album.title}</strong>
      <p>{message}</p>
      {stage === "ready" ? <button type="button" onClick={() => setReview(true)}>Review move</button> : null}
      {!busy ? <button type="button" onClick={onDismiss}>{stage === "ready" ? "Cancel" : "Dismiss"}</button> : null}
    </section>
    {review && preview ? <dialog ref={dialog} className="deep-explorer-delete-dialog deep-explorer-move-dialog" aria-labelledby="album-move-title" onCancel={(event) => { event.preventDefault(); setReview(false); }}>
      <span className="deep-explorer-delete-dialog__icon"><FolderOutput aria-hidden="true" /></span>
      <div>
        <h4 id="album-move-title">{label}: {request.album.title}?</h4>
        <p>Move the complete folder from <strong>{preview.sourcePath}</strong> to <strong>{preview.albums[0]?.destinationPath}</strong> and remove its {preview.trackCount} tracks from Music Library. The copy is verified before the database is updated and the original folder is removed.</p>
      </div>
      <div className="deep-explorer-delete-dialog__actions">
        <button type="button" autoFocus onClick={() => setReview(false)}>Back</button>
        <button type="button" disabled={!preview.canApply} onClick={() => void apply()}>{label}</button>
      </div>
    </dialog> : null}
  </>, document.body);
}
