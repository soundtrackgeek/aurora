import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FolderOutput, LoaderCircle } from "lucide-react";
import { libraryIntakeAdapter, previewLibraryRemoveAlbum } from "../../ingest";
import { useLibraryIntakeProgress } from "../inbox/useLibraryIntakeProgress";
import { BUSY_RETRY_DELAYS_MS, isLibraryBusyFailure } from "./albumBatchRemovalRetry";

export const REMOVED_ALBUMS_FOLDER = "D:\\MUSIC_NOT_ALBUMS";

export interface AlbumBatchRemovalRequest {
  albums: ReadonlyArray<{ id: string; title: string; artist: string; totalTracks?: number }>;
}

export interface AlbumBatchRemovalSummary {
  removed: number;
  total: number;
  warnings: string[];
  /** Why the batch stopped early, or null when it finished or was stopped by the user. */
  failure: string | null;
  stopped: boolean;
}

type Stage = "review" | "moving" | "completed" | "failed";

const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
const pause = (ms: number) => new Promise<void>((resolve) => { setTimeout(resolve, ms); });

/**
 * Music Library keeps one pending plan at a time (a new preview discards an unapplied one) and
 * a removal plan holds exactly one album, so the reviewed selection is removed album by album:
 * preview, then apply, before the next album is touched. Each album keeps the single-album
 * guarantees: the copy is verified before the catalog changes and the original folder goes.
 */
export function AlbumBatchRemoval({ request, onDismiss, onAlbumRemoved, onFinished, busyRetryDelaysMs = BUSY_RETRY_DELAYS_MS }: {
  request: AlbumBatchRemovalRequest;
  onDismiss: () => void;
  onAlbumRemoved: (albumId: string) => void;
  onFinished: (summary: AlbumBatchRemovalSummary) => Promise<void>;
  busyRetryDelaysMs?: readonly number[];
}) {
  const { albums } = request;
  const [stage, setStage] = useState<Stage>("review");
  const [current, setCurrent] = useState<{ index: number; albumId: string; planId: string | null } | null>(null);
  const [removedCount, setRemovedCount] = useState(0);
  const [message, setMessage] = useState("");
  const [waitMessage, setWaitMessage] = useState<string | null>(null);
  const [stopRequested, setStopRequested] = useState(false);
  const stopRef = useRef(false);
  const running = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const { progress } = useLibraryIntakeProgress();

  useEffect(() => {
    if (stage === "review" && dialog.current && !dialog.current.open) dialog.current.showModal();
  }, [stage]);

  const describe = (album: AlbumBatchRemovalRequest["albums"][number]) => `${album.artist} – ${album.title}`;

  async function run() {
    if (running.current) return;
    running.current = true;
    setStage("moving");
    const warnings: string[] = [];
    let removed = 0;
    let failure: string | null = null;
    for (const [index, album] of albums.entries()) {
      if (stopRef.current) break;
      for (let attempt = 0; ; attempt += 1) {
        setCurrent({ index, albumId: album.id, planId: null });
        try {
          const preview = await previewLibraryRemoveAlbum(album.id);
          if (!preview.canApply) throw new Error("Music Library blocked the removal preview. No files moved for this album.");
          setCurrent({ index, albumId: album.id, planId: preview.planId });
          const result = await libraryIntakeAdapter.apply({ planId: preview.planId, sessionId: preview.sessionId });
          removed += 1;
          setRemovedCount(removed);
          warnings.push(...result.cleanupWarnings.map((warning) => `${describe(album)}: ${warning}`));
          onAlbumRemoved(album.id);
          break;
        } catch (error) {
          const text = errorMessage(error);
          const delay = busyRetryDelaysMs[attempt];
          if (delay === undefined || !isLibraryBusyFailure(text) || stopRef.current) {
            failure = `${describe(album)}: ${text}`;
            break;
          }
          setCurrent({ index, albumId: album.id, planId: null });
          setWaitMessage(`Music Library is busy finishing the previous album. Nothing was removed for this one; retrying in ${Math.round(delay / 1000)} s (attempt ${attempt + 2} of ${busyRetryDelaysMs.length + 1}).`);
          await pause(delay);
          setWaitMessage(null);
        }
      }
      if (failure) break;
    }
    setCurrent(null);
    const stopped = !failure && removed < albums.length;
    const summary: AlbumBatchRemovalSummary = { removed, total: albums.length, warnings, failure, stopped };
    const untouched = albums.length - removed - (failure ? 1 : 0);
    let text = `Removed ${removed} of ${albums.length} albums to ${REMOVED_ALBUMS_FOLDER}.`;
    if (failure) text += ` Stopped at ${failure}${untouched > 0 ? ` The remaining ${untouched} albums were not touched.` : ""}`;
    else if (stopped) text += ` Stopped on request; the remaining ${albums.length - removed} albums were not touched.`;
    if (warnings.length) text += ` ${warnings.join(" ")}`;
    try {
      await onFinished(summary);
    } catch (error) {
      text += ` Aurora could not refresh the catalog: ${errorMessage(error)}`;
      failure ??= "refresh";
    }
    setMessage(text);
    setStage(failure || warnings.length ? "failed" : "completed");
  }

  function requestStop() {
    stopRef.current = true;
    setStopRequested(true);
  }

  const active = current ? albums[current.index] : null;
  const liveProgress = current && progress && (
    (progress.operation === "previewRemoveAlbum" && progress.albumId === current.albumId)
    || (progress.operation === "applyBatch" && current.planId !== null && progress.planId === current.planId)
  ) ? progress : null;
  const liveMessage = liveProgress
    ? `${liveProgress.message}${liveProgress.stage === "transferring" && liveProgress.totalFiles > 0
      ? ` · ${liveProgress.processedFiles}/${liveProgress.totalFiles} files` : ""}`
    : "Waiting for Music Library · you can keep browsing";
  const label = `Remove ${albums.length} Albums`;

  return createPortal(<>
    {stage !== "review" ? <section className="album-move-operation" role={stage === "failed" ? "alert" : "status"} aria-label={label}>
      <strong>{stage === "moving" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <FolderOutput aria-hidden="true" />}{label}</strong>
      {stage === "moving" && active && current ? <>
        <p>Album {current.index + 1} of {albums.length} · {describe(active)}</p>
        <p>{waitMessage ?? liveMessage}</p>
        <progress className="album-batch-removal__progress" max={albums.length} value={removedCount} aria-label={`${removedCount} of ${albums.length} albums removed`} />
        <button type="button" disabled={stopRequested} onClick={requestStop}>{stopRequested ? "Stopping after this album…" : "Stop after this album"}</button>
      </> : <p>{message}</p>}
      {stage === "completed" || stage === "failed" ? <button type="button" onClick={onDismiss}>Dismiss</button> : null}
    </section> : null}
    {stage === "review" ? <dialog ref={dialog} className="deep-explorer-delete-dialog deep-explorer-move-dialog" aria-labelledby="album-batch-removal-title" onCancel={(event) => { event.preventDefault(); onDismiss(); }}>
      <span className="deep-explorer-delete-dialog__icon"><FolderOutput aria-hidden="true" /></span>
      <div>
        <h4 id="album-batch-removal-title">{label}?</h4>
        <p>Move each album folder to <strong>{REMOVED_ALBUMS_FOLDER}</strong> and remove its tracks from Music Library, one album at a time. Every copy is verified before the database is updated and the original folder is removed. If an album cannot be removed, Aurora stops there and leaves the rest untouched.</p>
        <ol className="album-batch-removal__list">
          {albums.map((album) => <li key={album.id}>{describe(album)}{album.totalTracks ? <span> · {album.totalTracks} tracks</span> : null}</li>)}
        </ol>
      </div>
      <div className="deep-explorer-delete-dialog__actions">
        <button type="button" autoFocus onClick={onDismiss}>Cancel</button>
        <button type="button" onClick={() => void run()}>{label}</button>
      </div>
    </dialog> : null}
  </>, document.body);
}
