import { AlertTriangle, Check, Disc3, ImagePlus, LoaderCircle, Search, Settings, Tags, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  applyInboxTags,
  embedInboxAlbumCover,
  inboxCoverUrl,
  loadInboxReleaseDetail,
  renameInboxAlbums,
  searchInboxReleases,
  selectInboxCoverImage,
  type InboxAlbum,
  type InboxTrack,
  type MetadataSource,
  type ReleaseCandidate,
  type ReleaseCandidateDetail,
} from "../../inbox";
import type { EditableTagField, EditableTagValues } from "../../tags";
import { loadGenreNames } from "../../genres";
import { reconcileInboxTracks, type InboxTrackMatchStatus } from "../../inboxMatching";
import { applyLibraryAutoTags, renameSummary, type LibraryAutoTagResult } from "../../libraryAutoTag";
import "./Inbox.css";

const allFields: Array<{ id: EditableTagField; label: string }> = [
  { id: "albumArtist", label: "Album artist" },
  { id: "artist", label: "Track artists" },
  { id: "album", label: "Album" },
  { id: "title", label: "Track titles" },
  { id: "genre", label: "Genre" },
  { id: "publisher", label: "Publisher" },
  { id: "year", label: "Original year" },
  { id: "releaseYear", label: "Release year" },
  { id: "trackNumber", label: "Track numbers" },
  { id: "trackTotal", label: "Track totals" },
  { id: "discNumber", label: "Disc numbers" },
  { id: "discTotal", label: "Disc total" },
];

function leafName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function sourceLabel(source: MetadataSource): string {
  return source === "musicbrainz" ? "MusicBrainz" : "Discogs";
}

/** A library album, tagged in place with one reviewed Music Library sync instead of an Inbox write. */
export interface LibraryAutoTagTarget {
  albumId: string;
  coverUrl: string | null;
}

export function AlbumAutoTagger({ albums, tracks, discogsConfigured, onOpenSettings, onClose, onApplied, library }: { albums: InboxAlbum[]; tracks: InboxTrack[]; discogsConfigured: boolean; onOpenSettings: () => void; onClose: () => void; onApplied: (message: string, albumPath: string, result?: LibraryAutoTagResult) => void | Promise<void>; library?: LibraryAutoTagTarget }) {
  const album = albums[0];
  const totalAlbumTracks = albums.reduce((total, candidate) => total + candidate.tracks.length, 0);
  const [artist, setArtist] = useState(album.artist ?? "");
  const [title, setTitle] = useState(album.album ?? album.folderName);
  const [candidates, setCandidates] = useState<ReleaseCandidate[]>([]);
  const [selectedCandidate, setSelectedCandidate] = useState<ReleaseCandidate | null>(null);
  const [detail, setDetail] = useState<ReleaseCandidateDetail | null>(null);
  const [preferOriginalEdition, setPreferOriginalEdition] = useState(true);
  const [fields, setFields] = useState<Set<EditableTagField>>(() => new Set(allFields.map(({ id }) => id)));
  const [values, setValues] = useState({ albumArtist: album.artist ?? "", album: album.album ?? "", genre: album.genre ?? "", publisher: album.publisher ?? "", year: album.year?.toString() ?? "", releaseYear: "" });
  const [trackTitles, setTrackTitles] = useState<string[]>(tracks.map((track) => track.title ?? ""));
  const [removeExtraPaths, setRemoveExtraPaths] = useState<Set<string>>(new Set());
  const [manualMatches, setManualMatches] = useState<Map<number, number>>(() => new Map());
  const [manualDrafts, setManualDrafts] = useState<Map<number, number>>(() => new Map());
  const [confirmRemoval, setConfirmRemoval] = useState(false);
  const [renameAfterApply, setRenameAfterApply] = useState(tracks.length === totalAlbumTracks);
  const [discNumber, setDiscNumber] = useState("");
  const [discTotal, setDiscTotal] = useState("");
  const [busy, setBusy] = useState<"search" | "detail" | "apply" | null>(null);
  const [pickedCoverPath, setPickedCoverPath] = useState<string | null>(null);
  const [failedCoverSource, setFailedCoverSource] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [genreSuggestions, setGenreSuggestions] = useState<string[]>([]);
  const loadRequest = useRef(0);
  const updating = busy !== null;
  const embeddedCoverAlbum = albums.find((candidate) => candidate.artworkPresent && candidate.artworkSourcePath) ?? null;
  // Library covers are changed from the sidebar tag editor, so the auto-tagger only shows them.
  const embeddedCoverSource = library ? library.coverUrl : embeddedCoverAlbum ? inboxCoverUrl(embeddedCoverAlbum, 128) : null;
  const hasEmbeddedCover = Boolean(library) || albums.some((candidate) => candidate.artworkPresent);
  const reconciliation = useMemo(
    () => reconcileInboxTracks(
      tracks,
      detail?.tracks ?? [],
      [...manualMatches].map(([releaseIndex, localIndex]) => ({ localIndex, releaseIndex })),
    ),
    [detail?.tracks, manualMatches, tracks],
  );

  useEffect(() => {
    let cancelled = false;
    void loadGenreNames()
      .then((names) => { if (!cancelled) setGenreSuggestions(names); })
      .catch((nextError: unknown) => console.warn("Aurora could not load genre suggestions", nextError));
    return () => { cancelled = true; };
  }, []);

  async function chooseCover() {
    if (updating) return;
    try {
      const path = await selectInboxCoverImage(album.path);
      if (path) setPickedCoverPath(path);
    } catch (nextError) { setError(nextError instanceof Error ? nextError.message : String(nextError)); }
  }

  const hydrateDetail = useCallback((next: ReleaseCandidateDetail) => {
    const nextReconciliation = reconcileInboxTracks(tracks, next.tracks);
    const releaseByLocalIndex = new Map(nextReconciliation.rows
      .filter((row) => row.localIndex !== null && row.releaseIndex !== null)
      .map((row) => [row.localIndex as number, row.releaseIndex as number]));
    setValues({
      albumArtist: next.albumArtist ?? "",
      album: next.album ?? "",
      genre: next.genre ?? "",
      publisher: next.publisher ?? "",
      year: next.candidate.originalYear?.toString() ?? next.year?.toString() ?? "",
      releaseYear: next.year?.toString() ?? "",
    });
    setTrackTitles(tracks.map((track, index) => {
      const releaseIndex = releaseByLocalIndex.get(index);
      return releaseIndex === undefined ? track.title ?? "" : next.tracks[releaseIndex]?.title ?? track.title ?? "";
    }));
    setManualMatches(new Map());
    setManualDrafts(new Map());
    setRemoveExtraPaths(new Set());
    setConfirmRemoval(false);
    const releaseDiscs = new Set(next.tracks.map((track) => track.discNumber).filter((value): value is number => value !== null));
    setDiscNumber(releaseDiscs.size === 1 ? String([...releaseDiscs][0]) : "");
    setDiscTotal(next.discTotal ? String(next.discTotal) : "");
  }, [tracks]);

  const performSearch = useCallback(async (searchArtist: string, searchTitle: string) => {
    const request = ++loadRequest.current;
    setBusy("search"); setError(null); setDetail(null); setSelectedCandidate(null);
    try {
      const result = await searchInboxReleases(searchArtist, searchTitle, tracks.length, preferOriginalEdition);
      if (request !== loadRequest.current) return;
      setCandidates(result.candidates); setWarnings(result.warnings);
      if (result.candidates[0]) {
        setSelectedCandidate(result.candidates[0]);
        setBusy("detail");
        const next = await loadInboxReleaseDetail(result.candidates[0]);
        if (request !== loadRequest.current) return;
        setDetail(next); hydrateDetail(next);
      }
    } catch (nextError) { if (request === loadRequest.current) setError(nextError instanceof Error ? nextError.message : String(nextError)); }
    finally { if (request === loadRequest.current) setBusy(null); }
  }, [hydrateDetail, preferOriginalEdition, tracks.length]);

  useEffect(() => {
    const initial = window.setTimeout(() => void performSearch(album.artist ?? "", album.album ?? album.folderName), 0);
    return () => window.clearTimeout(initial);
  }, [album.album, album.artist, album.folderName, performSearch]);
  useEffect(() => { const close = (event: KeyboardEvent) => { if (event.key === "Escape" && !busy) onClose(); }; window.addEventListener("keydown", close); return () => window.removeEventListener("keydown", close); }, [busy, onClose]);

  async function chooseCandidate(candidate: ReleaseCandidate) {
    if (busy === "apply") return;
    const request = ++loadRequest.current;
    setSelectedCandidate(candidate); setBusy("detail"); setError(null);
    try { const next = await loadInboxReleaseDetail(candidate); if (request !== loadRequest.current) return; setDetail(next); hydrateDetail(next); }
    catch (nextError) { if (request === loadRequest.current) setError(nextError instanceof Error ? nextError.message : String(nextError)); }
    finally { if (request === loadRequest.current) setBusy(null); }
  }

  function toggleField(field: EditableTagField) { setFields((current) => { const next = new Set(current); if (next.has(field)) next.delete(field); else next.add(field); return next; }); }

  const unmatchedLocalIndices = reconciliation.rows.flatMap((row) => (
    row.status === "extra" && row.localIndex !== null ? [row.localIndex] : []
  ));

  function proposedLocalIndex(releaseIndex: number): number | null {
    const draft = manualDrafts.get(releaseIndex);
    if (draft !== undefined && unmatchedLocalIndices.includes(draft)) return draft;
    const releaseTrack = detail?.tracks[releaseIndex];
    const sameNumber = unmatchedLocalIndices.filter((localIndex) => (
      releaseTrack?.trackNumber !== null && tracks[localIndex]?.trackNumber === releaseTrack?.trackNumber
    ));
    if (sameNumber.length === 1) return sameNumber[0];
    return unmatchedLocalIndices.length === 1 ? unmatchedLocalIndices[0] : null;
  }

  function setManualDraft(releaseIndex: number, value: string) {
    setManualDrafts((current) => {
      const next = new Map(current);
      if (value === "") next.delete(releaseIndex);
      else next.set(releaseIndex, Number(value));
      return next;
    });
  }

  function confirmManualMatch(releaseIndex: number, localIndex: number) {
    const releaseTrack = detail?.tracks[releaseIndex];
    const localTrack = tracks[localIndex];
    if (!releaseTrack || !localTrack || !unmatchedLocalIndices.includes(localIndex)) return;
    setManualMatches((current) => {
      const next = new Map(current);
      for (const [currentReleaseIndex, currentLocalIndex] of next) {
        if (currentLocalIndex === localIndex) next.delete(currentReleaseIndex);
      }
      next.set(releaseIndex, localIndex);
      return next;
    });
    setManualDrafts((current) => { const next = new Map(current); next.delete(releaseIndex); return next; });
    setTrackTitles((current) => current.map((title, index) => index === localIndex ? releaseTrack.title : title));
    setRemoveExtraPaths((current) => { const next = new Set(current); next.delete(localTrack.path); return next; });
    setConfirmRemoval(false);
  }

  function undoManualMatch(releaseIndex: number) {
    const localIndex = manualMatches.get(releaseIndex);
    if (localIndex === undefined) return;
    setManualMatches((current) => { const next = new Map(current); next.delete(releaseIndex); return next; });
    setManualDrafts((current) => new Map(current).set(releaseIndex, localIndex));
    setTrackTitles((current) => current.map((title, index) => index === localIndex ? tracks[localIndex]?.title ?? "" : title));
  }

  const selectedRemovalCount = removeExtraPaths.size;
  const allExtrasSelected = reconciliation.extraCount > 0 && selectedRemovalCount === reconciliation.extraCount;
  const removableExtraPaths = reconciliation.cleanupSafe
    ? unmatchedLocalIndices.flatMap((localIndex) => (tracks[localIndex] ? [tracks[localIndex].path] : []))
    : [];
  const everyRemovableExtraSelected = removableExtraPaths.length > 0 && removableExtraPaths.every((path) => removeExtraPaths.has(path));

  function toggleAllExtras() {
    setRemoveExtraPaths(everyRemovableExtraSelected ? new Set() : new Set(removableExtraPaths));
    setConfirmRemoval(false);
  }
  const willRename = renameAfterApply
    && tracks.length === totalAlbumTracks
    && (reconciliation.extraCount === 0 || allExtrasSelected);

  async function apply() {
    if (!detail || fields.size === 0) return;
    if (removeExtraPaths.size > 0 && !confirmRemoval) {
      setConfirmRemoval(true);
      return;
    }
    setBusy("apply"); setError(null);
    try {
      const trackUpdates = reconciliation.rows.flatMap((match) => {
          if (match.localIndex === null || match.releaseIndex === null || (match.status !== "exact" && match.status !== "likely" && match.status !== "confirmed")) return [];
          const track = tracks[match.localIndex];
          const releaseTrack = detail.tracks[match.releaseIndex];
          const next: EditableTagValues = {
            albumArtist: values.albumArtist || null,
            artist: (releaseTrack?.artist ?? values.albumArtist) || null,
            album: values.album || null,
            title: trackTitles[match.localIndex] || null,
            genre: values.genre || null,
            publisher: values.publisher || null,
            rating: null,
            year: values.year ? Number(values.year) : null,
            releaseYear: values.releaseYear ? Number(values.releaseYear) : null,
            trackNumber: releaseTrack?.trackNumber ?? match.releaseIndex + 1,
            trackTotal: releaseTrack?.trackTotal ?? detail.tracks.length,
            discNumber: discNumber ? Number(discNumber) : releaseTrack?.discNumber ?? null,
            discTotal: discTotal ? Number(discTotal) : releaseTrack?.discTotal ?? detail.discTotal ?? null,
          };
          return [{ path: track.path, values: next }];
      });
      if (library) {
        const result = await applyLibraryAutoTags({
          albumId: library.albumId,
          renameFolder: willRename,
          tags: { albumPath: album.path, fields: [...fields], tracks: trackUpdates, removeTrackPaths: [...removeExtraPaths] },
        });
        await onApplied(libraryAutoTagMessage(result), result.rename?.destinationPath ?? album.path, result);
        return;
      }
      // Each disc folder is written separately, in disc order; one release can span several Inbox albums.
      const ownerPath = new Map(albums.flatMap((candidate) => candidate.tracks.map((track) => [track.path, candidate.path] as const)));
      const results: Awaited<ReturnType<typeof applyInboxTags>>[] = [];
      // A cover chosen by hand is embedded before tagging, while the disc folders still have their original paths.
      if (pickedCoverPath) {
        for (const target of albums.filter((candidate) => !candidate.artworkPresent)) {
          if (target.tracks.every((track) => removeExtraPaths.has(track.path))) continue;
          await embedInboxAlbumCover(target.path, pickedCoverPath);
        }
      }
      // Folders emptied by removing their extras (an expanded 2CD edition trimmed to one disc) have nothing left to rename.
      const survivingAlbums = albums.filter((candidate) => candidate.tracks.some((track) => !removeExtraPaths.has(track.path)));
      let mergedRenames = 0;
      for (const target of albums) {
        const targetUpdates = trackUpdates.filter((update) => ownerPath.get(update.path) === target.path);
        const targetRemovals = [...removeExtraPaths].filter((path) => ownerPath.get(path) === target.path);
        if (!targetUpdates.length && !targetRemovals.length) continue;
        results.push(await applyInboxTags({
          albumPath: target.path,
          fields: [...fields],
          tracks: targetUpdates,
          // Disc folders must be renamed together so Aurora can merge them into one album folder.
          renameAfterApply: willRename && survivingAlbums.length === 1 && survivingAlbums[0].path === target.path,
          removeTrackPaths: targetRemovals,
        }));
      }
      if (willRename && survivingAlbums.length > 1) {
        const renameResult = await renameInboxAlbums(survivingAlbums.map((candidate) => candidate.path));
        if (renameResult.failures.length) {
          throw new Error(`Tags were applied, but the discs could not be renamed: ${renameResult.failures[0].message}`);
        }
        mergedRenames = renameResult.renamedTracks;
      }
      const changedTracks = results.reduce((total, result) => total + result.changedTracks, 0);
      const renamedTracks = mergedRenames + results.reduce((total, result) => total + result.renamedTracks, 0);
      const removedTracks = results.reduce((total, result) => total + (result.removedTracks ?? 0), 0);
      const recoveryPath = results.find((result) => result.recoveryPath)?.recoveryPath;
      const discNote = albums.length > 1 ? ` across ${albums.length} discs` : "";
      await onApplied(removedTracks
        ? `Auto-tagged the selected edition${discNote} and moved ${removedTracks} unmatched ${removedTracks === 1 ? "track" : "tracks"} to Aurora recovery${recoveryPath ? ` at ${recoveryPath}` : ""}.`
        : `Auto-tagged ${changedTracks} ${changedTracks === 1 ? "track" : "tracks"}${discNote}${renamedTracks ? (albums.length > 1 ? " and renamed the albums" : " and renamed the album") : ""}.`, results[0]?.albumPath ?? album.path);
    } catch (nextError) { setError(nextError instanceof Error ? nextError.message : String(nextError)); setBusy(null); }
  }

  return <div className="modal-backdrop inbox-tagger-backdrop" role="presentation"><section className="inbox-tagger" role="dialog" aria-modal="true" aria-labelledby="inbox-tagger-title" aria-busy={Boolean(busy)}>
    <header><div className="inbox-tagger__mark"><Tags /></div><div><p className="eyebrow">{library ? "Library metadata" : "Inbox metadata"}</p><h2 id="inbox-tagger-title">Album Auto-Tagger</h2></div><button type="button" aria-label="Close auto-tagger" disabled={Boolean(busy)} onClick={onClose}><X /></button></header>
    <div className="inbox-tagger__content">
    <form className="inbox-tagger__search" onSubmit={(event) => { event.preventDefault(); if (!busy && artist.trim() && title.trim()) void performSearch(artist, title); }}><label>Album artist<input value={artist} onChange={(event) => setArtist(event.target.value)} /></label><label>Album<input value={title} onChange={(event) => setTitle(event.target.value)} /></label><label className="inbox-original-preference"><input type="checkbox" checked={preferOriginalEdition} disabled={Boolean(busy)} onChange={(event) => setPreferOriginalEdition(event.target.checked)} /><span><strong>Prefer the original edition</strong><small>Prioritize the earliest release, then compare its track list.</small></span></label><button type="submit" className="button button--primary" disabled={Boolean(busy) || !artist.trim() || !title.trim()}>{busy === "search" ? <LoaderCircle className="is-spinning" /> : <Search />} Find</button></form>
    <div className="inbox-tagger__notices">
      {!discogsConfigured ? <div className="inbox-provider-note"><AlertTriangle /><span><strong>Discogs is not connected.</strong><small>MusicBrainz results are still available.</small></span><button type="button" onClick={() => { onClose(); onOpenSettings(); }}><Settings /> Connect</button></div> : null}
      {warnings.map((warning) => <p className="inbox-tagger__warning" key={warning}><AlertTriangle />{warning}</p>)}
      {error ? <p className="inbox-tagger__error" role="alert"><AlertTriangle />{error}</p> : null}
      <p className={`inbox-tagger__status ${updating ? "is-updating" : "is-ok"}`} role="status">{updating ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Check aria-hidden="true" />}Status: {updating ? "Updating" : "OK"}{updating ? <small>{busy === "apply" ? "Applying tags…" : "Loading the selected edition — fields are locked until it finishes."}</small> : null}</p>
    </div>
    <div className="inbox-tagger__body">
      <section className="inbox-candidates" aria-label="Release matches"><table><thead><tr><th>Source</th><th>Score</th><th>Album</th><th>Artist</th><th>Original</th><th>Edition</th><th>Tracks</th><th>Format</th><th>Publisher</th></tr></thead><tbody>
        {candidates.map((candidate) => <tr key={`${candidate.source}:${candidate.id}`} className={selectedCandidate?.id === candidate.id && selectedCandidate.source === candidate.source ? "is-selected" : ""} onClick={() => void chooseCandidate(candidate)}><td><span className={`provider provider--${candidate.source}`}>{sourceLabel(candidate.source)}</span></td><td>{candidate.score}%</td><td>{candidate.title}</td><td>{candidate.artist}</td><td>{candidate.originalYear ?? "—"}</td><td>{candidate.year ?? "—"}</td><td>{candidate.trackCount ?? "—"}{candidate.trackCount !== null ? candidate.trackCount === tracks.length ? " · exact" : ` · ${Math.abs(tracks.length - candidate.trackCount)} ${tracks.length > candidate.trackCount ? "extra" : "missing"}` : ""}</td><td>{candidate.format ?? "—"}</td><td>{candidate.publisher ?? "—"}</td></tr>)}
        {!candidates.length && busy !== "search" ? <tr><td colSpan={9}>No release matches found. Broaden the artist or album spelling.</td></tr> : null}
      </tbody></table></section>
      <div className="inbox-tagger__editor" inert={updating}>
        <section className="inbox-release-fields">{hasEmbeddedCover
          ? <div className="inbox-release-art" title="Embedded album cover">{embeddedCoverSource && embeddedCoverSource !== failedCoverSource
            ? <img src={embeddedCoverSource} alt="Embedded album cover" onError={() => setFailedCoverSource(embeddedCoverSource)} />
            : <Disc3 />}</div>
          : <button type="button" className={`inbox-release-art inbox-release-art--empty${pickedCoverPath ? " has-pick" : ""}`} title={pickedCoverPath ? `Cover to embed: ${leafName(pickedCoverPath)}. Click to choose another.` : "No embedded cover. Click to choose an image to embed."} aria-label="Choose album cover" onClick={() => void chooseCover()}>{pickedCoverPath ? <><Check /><small>{leafName(pickedCoverPath)}</small></> : <><ImagePlus /><small>Add cover</small></>}</button>}
        <div className="inbox-release-form"><datalist id="inbox-auto-tagger-genre-suggestions">{genreSuggestions.map((genre) => <option value={genre} key={genre} />)}</datalist><label className="inbox-release-form__disc-total">Disc total<input inputMode="numeric" placeholder="Release" value={discTotal} onChange={(event) => setDiscTotal(event.target.value.replace(/\D/g, "").slice(0, 3))} /></label><label className="inbox-release-form__disc-override">Disc # override<input aria-label="Disc number override" inputMode="numeric" placeholder="Release" value={discNumber} onChange={(event) => setDiscNumber(event.target.value.replace(/\D/g, "").slice(0, 3))} /></label><label className="inbox-release-form__album-artist">Album artist<input value={values.albumArtist} onChange={(event) => setValues((current) => ({ ...current, albumArtist: event.target.value }))} /></label><label className="inbox-release-form__album">Album<input value={values.album} onChange={(event) => setValues((current) => ({ ...current, album: event.target.value }))} /></label><label className="inbox-release-form__publisher">Publisher<input value={values.publisher} onChange={(event) => setValues((current) => ({ ...current, publisher: event.target.value }))} /></label><label className="inbox-release-form__year">Original year<input inputMode="numeric" value={values.year} onChange={(event) => setValues((current) => ({ ...current, year: event.target.value.replace(/\D/g, "").slice(0, 4) }))} /></label><label className="inbox-release-form__release-year">Release year<input inputMode="numeric" value={values.releaseYear} onChange={(event) => setValues((current) => ({ ...current, releaseYear: event.target.value.replace(/\D/g, "").slice(0, 4) }))} /></label><label className="inbox-release-form__genre">Genre<input list="inbox-auto-tagger-genre-suggestions" value={values.genre} onChange={(event) => setValues((current) => ({ ...current, genre: event.target.value }))} /></label></div></section>
        <fieldset className="inbox-fields"><legend>Include fields</legend>{allFields.map((field) => <label key={field.id}><input type="checkbox" checked={fields.has(field.id)} onChange={() => toggleField(field.id)} />{field.label}</label>)}</fieldset>
      </div>
      <section className="inbox-track-compare" inert={updating}>
        {detail ? <div className="inbox-reconciliation-summary" role="status"><strong>{reconciliation.matchedCount} of {detail.tracks.length} release tracks matched</strong><span>{reconciliation.extraCount ? `${reconciliation.extraCount} extra local ${reconciliation.extraCount === 1 ? "track" : "tracks"}` : "No extra local tracks"}{reconciliation.missingCount || reconciliation.ambiguousCount ? ` · ${reconciliation.missingCount + reconciliation.ambiguousCount} unresolved` : ""}</span>{removableExtraPaths.length > 0 ? <button type="button" className="inbox-reconciliation-summary__select-all" aria-pressed={everyRemovableExtraSelected} onClick={toggleAllExtras}>{everyRemovableExtraSelected ? "Clear extra selection" : `Select all extras (${removableExtraPaths.length})`}</button> : null}{reconciliation.extraCount && !reconciliation.cleanupSafe ? <small>Choose an unmatched file and confirm any true match before removing extras.</small> : null}</div> : null}
        <table><thead><tr><th>#</th><th>Your file</th><th>Current title</th><th>Release title</th><th>Status</th><th>Action</th></tr></thead><tbody>{detail ? reconciliation.rows.map((match, rowIndex) => {
          const track = match.localIndex === null ? null : tracks[match.localIndex];
          const releaseTrack = match.releaseIndex === null ? null : detail.tracks[match.releaseIndex];
          const removable = match.status === "extra" && track && reconciliation.cleanupSafe;
          const unresolved = (match.status === "missing" || match.status === "ambiguous") && match.releaseIndex !== null;
          const proposedIndex = unresolved ? proposedLocalIndex(match.releaseIndex as number) : null;
          const proposedTrack = proposedIndex === null ? null : tracks[proposedIndex];
          return <tr key={`${match.localIndex ?? "none"}:${match.releaseIndex ?? "none"}:${rowIndex}`} className={`inbox-track-match inbox-track-match--${match.status}`}><td>{releaseTrack?.trackNumber ?? track?.trackNumber ?? "—"}</td><td title={track?.fileName}>{unresolved ? <select aria-label={`Local file for ${releaseTrack?.title ?? "unresolved release track"}`} value={proposedIndex ?? ""} onChange={(event) => setManualDraft(match.releaseIndex as number, event.target.value)}><option value="">Choose unmatched file</option>{unmatchedLocalIndices.map((localIndex) => <option key={tracks[localIndex]?.path} value={localIndex}>{tracks[localIndex]?.fileName}</option>)}</select> : track?.fileName ?? "—"}</td><td>{track?.title ?? proposedTrack?.title ?? "—"}</td><td>{match.localIndex !== null && releaseTrack ? <input aria-label={`Release title ${match.localIndex + 1}`} value={trackTitles[match.localIndex] ?? ""} onChange={(event) => setTrackTitles((current) => current.map((value, itemIndex) => itemIndex === match.localIndex ? event.target.value : value))} /> : releaseTrack?.title ?? "—"}</td><td><span className={`inbox-track-status inbox-track-status--${match.status}`}>{trackMatchIcon(match.status)}{trackMatchLabel(match.status)}</span></td><td className="inbox-track-action">{removable ? <input type="checkbox" aria-label={`Remove unmatched ${track.fileName}`} checked={removeExtraPaths.has(track.path)} onChange={() => setRemoveExtraPaths((current) => { const next = new Set(current); if (next.has(track.path)) next.delete(track.path); else next.add(track.path); setConfirmRemoval(false); return next; })} /> : unresolved ? <button type="button" disabled={proposedIndex === null} onClick={() => { if (proposedIndex !== null) confirmManualMatch(match.releaseIndex as number, proposedIndex); }}><Check aria-hidden="true" />Confirm match</button> : match.status === "confirmed" && match.releaseIndex !== null ? <button type="button" onClick={() => undoManualMatch(match.releaseIndex as number)}>Undo</button> : "—"}</td></tr>;
        }) : null}</tbody></table>
      </section>
    </div>
    </div>
    <footer><label className="inbox-rename-option"><input type="checkbox" checked={renameAfterApply} disabled={Boolean(busy) || tracks.length !== totalAlbumTracks || (reconciliation.extraCount > 0 && !allExtrasSelected)} onChange={(event) => setRenameAfterApply(event.target.checked)} /><span><strong>{library ? "Rename folder after tagging" : "Rename after tagging"}</strong><small>{tracks.length !== totalAlbumTracks ? "Tag all album tracks before renaming" : reconciliation.extraCount > 0 && !allExtrasSelected ? "Select every extra track for recovery before renaming" : "Album Artist - Album (Original year) · 1-01 or 01 - Artist - Title"}</small></span></label><span className="inbox-tagger__source">{detail ? `${sourceLabel(detail.candidate.source)} release ${detail.candidate.id}` : "Choose a release to review its tags."}</span><button type="button" className="button button--quiet" disabled={Boolean(busy)} onClick={onClose}>Cancel</button><button type="button" className="button button--primary" disabled={!detail || fields.size === 0 || reconciliation.matchedCount === 0 || Boolean(busy)} onClick={() => void apply()}>{busy === "apply" ? <LoaderCircle className="is-spinning" /> : selectedRemovalCount ? <Trash2 /> : <Check />} {selectedRemovalCount ? `${willRename ? "Apply, rename & move" : "Apply & move"} ${selectedRemovalCount} ${selectedRemovalCount === 1 ? "extra" : "extras"}` : willRename ? "Apply & rename" : `Apply to ${reconciliation.matchedCount} tracks`}</button></footer>
    {confirmRemoval ? <div className="inbox-extra-confirmation-backdrop" role="presentation"><dialog className="inbox-extra-confirmation" open role="alertdialog" aria-modal="true" aria-labelledby="inbox-extra-confirmation-title" aria-describedby="inbox-extra-confirmation-description" onKeyDown={(event) => { if (event.key === "Escape" && !busy) { event.stopPropagation(); setConfirmRemoval(false); } }}><span className="inbox-extra-confirmation__icon"><Trash2 aria-hidden="true" /></span><div><h3 id="inbox-extra-confirmation-title">Move {selectedRemovalCount} unmatched {selectedRemovalCount === 1 ? "track" : "tracks"} out of this album?</h3><p id="inbox-extra-confirmation-description">Aurora will copy and verify {selectedRemovalCount === 1 ? "this MP3" : "these MP3s"} in its recovery folder before removing {selectedRemovalCount === 1 ? "it" : "them"} from the {library ? "library album. Music Library then drops them in the same update as the new tags." : "Inbox album."} Tagging{library ? "" : " and renaming"} roll{library ? "s" : ""} back if removal fails.</p><ul>{[...removeExtraPaths].map((path) => <li key={path}>{leafName(path)}</li>)}</ul></div><div className="inbox-extra-confirmation__actions"><button type="button" disabled={Boolean(busy)} onClick={() => setConfirmRemoval(false)}>Cancel</button><button type="button" className="is-destructive" disabled={Boolean(busy)} autoFocus onClick={() => void apply()}>{busy === "apply" ? <LoaderCircle className="is-spinning" /> : <Trash2 />}Move to recovery</button></div></dialog></div> : null}
  </section></div>;
}

function libraryAutoTagMessage(result: LibraryAutoTagResult): string {
  const tagged = `Auto-tagged ${result.changedTracks} ${result.changedTracks === 1 ? "track" : "tracks"}`;
  const removed = result.removedTracks
    ? ` and moved ${result.removedTracks} unmatched ${result.removedTracks === 1 ? "track" : "tracks"} to Aurora recovery${result.recoveryPath ? ` at ${result.recoveryPath}` : ""}`
    : "";
  const sync = result.catalogSync.status === "synced"
    ? " Music Library updated."
    : ` ${result.catalogSync.message ?? "Music Library sync is pending; Aurora retries automatically."}`;
  const rename = result.rename ? renameSummary(result.rename) : null;
  return `${tagged}${removed}.${sync}${rename ? ` ${rename}` : ""}${result.renameError ? ` ${result.renameError}` : ""}`;
}

function trackMatchLabel(status: InboxTrackMatchStatus): string {
  if (status === "exact") return "Matched";
  if (status === "likely") return "Likely match";
  if (status === "confirmed") return "Confirmed";
  if (status === "extra") return "Extra local";
  if (status === "missing") return "Missing local";
  return "Ambiguous";
}

function trackMatchIcon(status: InboxTrackMatchStatus) {
  if (status === "exact" || status === "confirmed") return <Check aria-hidden="true" />;
  return <AlertTriangle aria-hidden="true" />;
}
