import { AlertTriangle, ArrowRight, Check, FolderInput, LoaderCircle, X } from "lucide-react";
import { useMemo, useState } from "react";
import {
  libraryIntakeAdapter,
  libraryIntakeCategories,
  type LibraryIntakeCategoryId,
  type LibraryIntakePreview,
} from "../../ingest";
import { LibraryIntakeActivity } from "./LibraryIntakeActivity";
import { useLibraryIntakeProgress } from "./useLibraryIntakeProgress";

export interface InboxLibraryIntakeTarget {
  sourcePath: string;
  label: string;
  albumCount: number;
  unreadyAlbumCount: number;
}

interface InboxLibraryIntakeDialogProps {
  scopeLabel: string;
  targets: InboxLibraryIntakeTarget[];
  onClose: () => void;
  onApplied: () => boolean | void | Promise<boolean | void>;
  onCompleted: (message: string) => void;
}

export function InboxLibraryIntakeDialog({ scopeLabel, targets, onClose, onApplied, onCompleted }: InboxLibraryIntakeDialogProps) {
  const [destinations, setDestinations] = useState<Record<string, LibraryIntakeCategoryId | "">>(() => Object.fromEntries(targets.map((target) => [target.sourcePath, ""])));
  const [previews, setPreviews] = useState<LibraryIntakePreview[] | null>(null);
  const [busy, setBusy] = useState<"preview" | "apply" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [replacementConfirmed, setReplacementConfirmed] = useState(false);
  const [activeTargetIndex, setActiveTargetIndex] = useState(0);
  const { progress, reset: resetProgress } = useLibraryIntakeProgress();

  const albumCount = useMemo(() => targets.reduce((total, target) => total + target.albumCount, 0), [targets]);
  const unreadyAlbumCount = useMemo(() => targets.reduce((total, target) => total + target.unreadyAlbumCount, 0), [targets]);
  const destinationsSelected = targets.every((target) => destinations[target.sourcePath]);
  const canApply = previews?.length === targets.length && previews.every((preview) => preview.canApply);
  const replacements = previews?.flatMap((preview) => preview.albums.filter((album) => album.action === "replace")) ?? [];

  async function previewTargets() {
    if (!destinationsSelected || unreadyAlbumCount > 0) return;
    setBusy("preview");
    setError(null);
    resetProgress();
    try {
      const next: LibraryIntakePreview[] = [];
      for (const [index, target] of targets.entries()) {
        setActiveTargetIndex(index);
        next.push(await libraryIntakeAdapter.preview({
          sourcePath: target.sourcePath,
          category: destinations[target.sourcePath] as LibraryIntakeCategoryId,
        }));
      }
      setPreviews(next);
      setReplacementConfirmed(false);
    } catch (nextError) {
      setPreviews(null);
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setBusy(null);
    }
  }

  async function applyTargets() {
    if (!canApply || !previews) return;
    setBusy("apply");
    setError(null);
    resetProgress();
    let completedAlbums = 0;
    try {
      for (const [index, reviewedPreview] of previews.entries()) {
        setActiveTargetIndex(index);
        const target = targets[index];
        const result = await applyReviewedTarget(
          target,
          destinations[target.sourcePath] as LibraryIntakeCategoryId,
          reviewedPreview,
        );
        completedAlbums += result.albumCount;
      }
      await onApplied();
      onCompleted(`${completedAlbums} ${completedAlbums === 1 ? "album" : "albums"} moved, covers archived, and library catalog updated.`);
      onClose();
    } catch (nextError) {
      if (completedAlbums > 0) {
        try { await onApplied(); } catch { /* The intake error remains the actionable result. */ }
      }
      setPreviews(null);
      const detail = nextError instanceof Error ? nextError.message : String(nextError);
      setError(completedAlbums > 0 ? `${completedAlbums} albums were added before the remaining intake stopped. ${detail}` : detail);
    } finally {
      setBusy(null);
    }
  }

  return <div className="modal-backdrop inbox-intake-backdrop" role="presentation">
    <section className="inbox-intake-dialog" role="dialog" aria-modal="true" aria-labelledby="inbox-intake-title" aria-busy={Boolean(busy)}>
      <header>
        <span className="inbox-intake-dialog__mark"><FolderInput /></span>
        <div><h2 id="inbox-intake-title">Add {scopeLabel} to library</h2><p>{albumCount} {albumCount === 1 ? "album" : "albums"} will use Music Library's reviewed mover and cover workflow.</p></div>
        <button type="button" aria-label="Close Add to Library" disabled={Boolean(busy)} onClick={onClose}><X /></button>
      </header>

      <div className="inbox-intake-dialog__body">
        {unreadyAlbumCount > 0 ? <p className="inbox-intake-dialog__warning" role="alert"><AlertTriangle /><span><strong>{unreadyAlbumCount} {unreadyAlbumCount === 1 ? "album is" : "albums are"} not ready.</strong><small>Resolve the Inbox readiness issues before adding this scope.</small></span></p> : null}
        <div className="inbox-intake-targets">
          {targets.map((target, index) => {
            const preview = previews?.[index];
            return <section key={target.sourcePath}>
              <div><strong>{target.label}</strong><small title={target.sourcePath}>{target.sourcePath}</small></div>
              <select aria-label={`Library destination for ${target.label}`} value={destinations[target.sourcePath] ?? ""} disabled={Boolean(busy)} onChange={(event) => { setDestinations((current) => ({ ...current, [target.sourcePath]: event.target.value as LibraryIntakeCategoryId | "" })); setPreviews(null); setError(null); }}>
                <option value="">Select music root…</option>
                {libraryIntakeCategories.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}
              </select>
              <span>{preview ? <><Check /> {preview.albumCount} {preview.albumCount === 1 ? "album" : "albums"} · {preview.trackCount} {preview.trackCount === 1 ? "track" : "tracks"} → {preview.category.destinationRoot}</> : `${target.albumCount} ${target.albumCount === 1 ? "album" : "albums"}`}</span>
            </section>;
          })}
        </div>
        {busy ? <LibraryIntakeActivity
          mode={busy}
          progress={progress}
          targetLabel={targets.length > 1 ? `${activeTargetIndex + 1} of ${targets.length} · ${targets[activeTargetIndex]?.label ?? "Folder"}` : targets[0]?.label}
        /> : null}
        {error ? <p className="inbox-intake-dialog__error" role="alert"><AlertTriangle />{error}</p> : null}
        {replacements.length ? <section className="inbox-intake-dialog__replacements" role="alert">
          <AlertTriangle />
          <div><strong>{replacements.length} existing {replacements.length === 1 ? "release" : "releases"} will be replaced</strong>
            {replacements.map((album) => <p key={album.destinationPath}><span>{album.artist} — {album.album} ({album.year})</span><small>{album.existingTrackCount} existing → {album.trackCount} new tracks · {album.matchedTrackCount} matched · {album.existingRatedTrackCount} rated · {album.existingLovedTrackCount} loved</small></p>)}
            <label><input type="checkbox" checked={replacementConfirmed} onChange={(event) => setReplacementConfirmed(event.target.checked)} /> I reviewed these replacements. Preserve each old release in the recovery folder.</label>
          </div>
        </section> : null}
      </div>

      <footer>
        <button type="button" className="button button--quiet" disabled={Boolean(busy)} onClick={onClose}>Cancel</button>
        <button type="button" className="button button--primary" disabled={Boolean(busy) || !destinationsSelected || unreadyAlbumCount > 0 || (Boolean(previews) && (!canApply || (replacements.length > 0 && !replacementConfirmed)))} onClick={() => void (previews ? applyTargets() : previewTargets())}>
          {busy ? <LoaderCircle className="is-spinning" /> : previews ? <FolderInput /> : <ArrowRight />}
          {busy === "preview" ? "Building preview…" : busy === "apply" ? "Adding to library…" : previews ? `Add ${albumCount} ${albumCount === 1 ? "album" : "albums"}` : "Preview destinations"}
        </button>
      </footer>
    </section>
  </div>;
}

const MAX_STALE_PLAN_RETRIES = 2;

async function applyReviewedTarget(
  target: InboxLibraryIntakeTarget,
  category: LibraryIntakeCategoryId,
  reviewedPreview: LibraryIntakePreview,
) {
  let applicablePreview = reviewedPreview;
  for (let stalePlanRetries = 0; ; stalePlanRetries += 1) {
    try {
      return await libraryIntakeAdapter.apply({ planId: applicablePreview.planId, sessionId: applicablePreview.sessionId });
    } catch (error) {
      if (!isStalePlanError(error)) throw error;
      if (stalePlanRetries >= MAX_STALE_PLAN_RETRIES) {
        const retryError = new Error("Music Library's catalog kept changing while Aurora prepared the move. Wait for tag synchronization to finish, then preview destinations again. (stalePlan)");
        (retryError as Error & { cause: unknown }).cause = error;
        throw retryError;
      }
      const freshPreview = await libraryIntakeAdapter.preview({
        sourcePath: target.sourcePath,
        category,
      });
      if (!sameReviewedIntake(reviewedPreview, freshPreview)) {
        const changedError = new Error(`${target.label} changed after review. Preview destinations again before adding it to the library.`);
        (changedError as Error & { cause: unknown }).cause = error;
        throw changedError;
      }
      applicablePreview = freshPreview;
    }
  }
}

function isStalePlanError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.toLowerCase().includes("(staleplan)");
}

function sameReviewedIntake(reviewed: LibraryIntakePreview, fresh: LibraryIntakePreview): boolean {
  return fresh.canApply
    && reviewed.sourcePath === fresh.sourcePath
    && reviewed.category.id === fresh.category.id
    && reviewed.category.destinationRoot === fresh.category.destinationRoot
    && reviewed.albumCount === fresh.albumCount
    && reviewed.trackCount === fresh.trackCount
    && reviewed.albums.length === fresh.albums.length
    && reviewed.albums.every((album, index) => {
      const candidate = fresh.albums[index];
      return candidate
        && album.sourcePath === candidate.sourcePath
        && album.destinationPath === candidate.destinationPath
        && album.artist === candidate.artist
        && album.album === candidate.album
        && album.year === candidate.year
        && album.trackCount === candidate.trackCount
        && album.action === candidate.action
        && album.existingTrackCount === candidate.existingTrackCount
        && album.matchedTrackCount === candidate.matchedTrackCount
        && album.existingRatedTrackCount === candidate.existingRatedTrackCount
        && album.existingLovedTrackCount === candidate.existingLovedTrackCount;
    });
}
