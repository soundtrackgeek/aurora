import {
  AlertTriangle,
  ArrowRight,
  AudioLines,
  Check,
  CheckCircle2,
  Disc3,
  FilePenLine,
  Folder,
  FolderInput,
  FolderPlus,
  ImagePlus,
  Inbox as InboxIcon,
  LoaderCircle,
  RefreshCw,
  Search,
  Settings,
  Tags,
  Trash2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  addInboxMonitorFolder,
  applyInboxTags,
  convertInboxLossless,
  embedInboxAlbumCover,
  inboxCoverUrl,
  loadInboxReleaseDetail,
  loadInboxSnapshot,
  removeInboxMonitorFolder,
  renameInboxAlbums,
  searchInboxReleases,
  selectInboxCoverImage,
  selectInboxMonitorFolder,
  type InboxAlbum,
  type InboxSnapshot,
  type InboxTrack,
  type MetadataSource,
  type ReleaseCandidate,
  type ReleaseCandidateDetail,
} from "../../inbox";
import {
  libraryIntakeAdapter,
  libraryIntakeCategories,
  type LibraryIntakeCategoryId,
  type LibraryIntakePreview,
} from "../../ingest";
import type { EditableTagField, EditableTagValues } from "../../tags";
import { loadGenreNames } from "../../genres";
import { formatDuration } from "../../library";
import { reconcileInboxTracks, type InboxTrackMatchStatus } from "../../inboxMatching";
import { InboxTagEditor } from "./InboxTagEditor";
import { InboxLibraryIntakeDialog, type InboxLibraryIntakeTarget } from "./InboxLibraryIntakeDialog";
import { LibraryIntakeActivity } from "./LibraryIntakeActivity";
import { useLibraryIntakeProgress } from "./useLibraryIntakeProgress";
import { applyWindowsSelection } from "../explorer/windowsSelection";
import "./Inbox.css";

type LoadState = "loading" | "ready" | "error";

interface InboxProps {
  onOpenMetadataSettings: () => void;
  onCatalogChanged: () => boolean | void | Promise<boolean | void>;
}

function InboxArtwork({ album, size, decorative = true }: { album: InboxAlbum; size: 64 | 128; decorative?: boolean }) {
  const source = inboxCoverUrl(album, size);
  const [failedSource, setFailedSource] = useState<string | null>(null);
  return <span className={size === 64 ? "inbox-table-art" : "inbox-inspector__art"} aria-hidden={decorative || undefined}>
    {source && source !== failedSource
      ? <img src={source} alt={decorative ? "" : `${album.album ?? album.folderName} cover`} onError={() => setFailedSource(source)} />
      : <Disc3 />}
  </span>;
}

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

function albumInFolder(album: InboxAlbum, folder: string): boolean {
  const prefix = folder.endsWith("\\") || folder.endsWith("/") ? folder : `${folder}\\`;
  return album.path.toLocaleLowerCase().startsWith(prefix.toLocaleLowerCase());
}

function formatTime(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(timestamp);
}

function formatInboxSize(bytes: number): string {
  if (bytes <= 0) return "Unknown size";
  const megabytes = bytes / (1024 * 1024);
  return `${megabytes >= 100 ? megabytes.toFixed(0) : megabytes.toFixed(1)} MB`;
}

function sourceLabel(source: MetadataSource): string {
  return source === "musicbrainz" ? "MusicBrainz" : "Discogs";
}

export function Inbox({ onOpenMetadataSettings, onCatalogChanged }: InboxProps) {
  const [snapshot, setSnapshot] = useState<InboxSnapshot | null>(null);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [error, setError] = useState<string | null>(null);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [selectedAlbumId, setSelectedAlbumId] = useState<string | null>(null);
  const [selectedAlbumIds, setSelectedAlbumIds] = useState<ReadonlySet<string>>(new Set());
  const [albumSelectionAnchorId, setAlbumSelectionAnchorId] = useState<string | null>(null);
  const [taggerAlbum, setTaggerAlbum] = useState<InboxAlbum | null>(null);
  const [moveCategory, setMoveCategory] = useState<LibraryIntakeCategoryId | "">("");
  const [movePreview, setMovePreview] = useState<LibraryIntakePreview | null>(null);
  const [moveBusy, setMoveBusy] = useState(false);
  const [moveMessage, setMoveMessage] = useState<string | null>(null);
  const [replacementConfirmed, setReplacementConfirmed] = useState(false);
  const [renameBusy, setRenameBusy] = useState(false);
  const [renameMessage, setRenameMessage] = useState<string | null>(null);
  const [coverBusy, setCoverBusy] = useState(false);
  const [coverMessage, setCoverMessage] = useState<string | null>(null);
  const [convertBusy, setConvertBusy] = useState(false);
  const [convertMessage, setConvertMessage] = useState<string | null>(null);
  const [excludedTrackPaths, setExcludedTrackPaths] = useState<Set<string>>(new Set());
  const [inspectorView, setInspectorView] = useState<"album" | "tags">("album");
  const [intakeScope, setIntakeScope] = useState<{ label: string; targets: InboxLibraryIntakeTarget[] } | null>(null);
  const [intakeMessage, setIntakeMessage] = useState<string | null>(null);
  const { progress: moveProgress, reset: resetMoveProgress } = useLibraryIntakeProgress();

  const refresh = useCallback(async (quiet = false) => {
    if (!quiet) setLoadState("loading");
    try {
      const next = await loadInboxSnapshot();
      setSnapshot(next);
      setError(null);
      setLoadState("ready");
      setSelectedAlbumId((current) => current && next.albums.some((album) => album.id === current) ? current : next.albums[0]?.id ?? null);
      setSelectedAlbumIds((current) => {
        const availableIds = new Set(next.albums.map((album) => album.id));
        const retained = new Set([...current].filter((id) => availableIds.has(id)));
        if (!retained.size && next.albums[0]) retained.add(next.albums[0].id);
        return retained;
      });
      setAlbumSelectionAnchorId((current) => current && next.albums.some((album) => album.id === current) ? current : next.albums[0]?.id ?? null);
      setSelectedFolder((current) => current && next.settings.monitoredFolders.includes(current) ? current : null);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
      setLoadState("error");
    }
  }, []);

  useEffect(() => {
    const initial = window.setTimeout(() => void refresh(), 0);
    const interval = window.setInterval(() => { if (document.visibilityState === "visible") void refresh(true); }, 15_000);
    const onFocus = () => void refresh(true);
    window.addEventListener("focus", onFocus);
    return () => { window.clearTimeout(initial); window.clearInterval(interval); window.removeEventListener("focus", onFocus); };
  }, [refresh]);

  const albums = useMemo(() => snapshot?.albums.filter((album) => !selectedFolder || albumInFolder(album, selectedFolder)) ?? [], [selectedFolder, snapshot]);
  const selectedAlbum = snapshot?.albums.find((album) => album.id === selectedAlbumId) ?? null;
  const selectedAlbums = useMemo(
    () => albums.filter((album) => selectedAlbumIds.has(album.id)),
    [albums, selectedAlbumIds],
  );
  const selectedAlbumTrackRows = useMemo(
    () => selectedAlbums.flatMap((album) => album.tracks.map((track, index) => ({ album, track, index }))),
    [selectedAlbums],
  );
  const selectedAlbumHasLossless = Boolean(selectedAlbum?.losslessTrackCount);
  const selectedAlbumsHaveLossless = selectedAlbums.some((album) => album.losslessTrackCount > 0);

  function selectFolder(folder: string | null) {
    const folderAlbums = snapshot?.albums.filter((album) => !folder || albumInFolder(album, folder)) ?? [];
    const firstId = folderAlbums[0]?.id ?? null;
    setSelectedFolder(folder);
    setSelectedAlbumId(firstId);
    setSelectedAlbumIds(firstId ? new Set([firstId]) : new Set());
    setAlbumSelectionAnchorId(firstId);
    setExcludedTrackPaths(new Set());
    setMovePreview(null);
    setReplacementConfirmed(false);
    setMoveMessage(null);
    if (folderAlbums[0]?.losslessTrackCount) setInspectorView("album");
  }

  function selectAlbum(album: InboxAlbum, ctrl: boolean, shift: boolean) {
    const selection = applyWindowsSelection(
      albums.map((candidate) => candidate.id),
      selectedAlbumIds,
      albumSelectionAnchorId,
      album.id,
      { ctrl, shift },
    );
    const activeId = selection.selectedKeys.has(album.id)
      ? album.id
      : albums.find((candidate) => selection.selectedKeys.has(candidate.id))?.id ?? null;
    setSelectedAlbumIds(selection.selectedKeys);
    setAlbumSelectionAnchorId(selection.anchorKey);
    setSelectedAlbumId(activeId);
    setExcludedTrackPaths(new Set());
    setMovePreview(null);
    setReplacementConfirmed(false);
    setMoveMessage(null);
    if (albums.some((candidate) => selection.selectedKeys.has(candidate.id) && candidate.losslessTrackCount > 0)) setInspectorView("album");
  }

  const selectedTracks = useMemo(
    () => selectedAlbum?.tracks.filter((track) => track.format === "MP3" && !excludedTrackPaths.has(track.path)) ?? [],
    [excludedTrackPaths, selectedAlbum],
  );
  const tagEditorTracks = useMemo(
    () => selectedAlbumTrackRows.filter(({ track }) => track.format === "MP3" && !excludedTrackPaths.has(track.path)).map(({ track }) => track),
    [excludedTrackPaths, selectedAlbumTrackRows],
  );
  const taggerTracks = useMemo(
    () => taggerAlbum?.tracks.filter((track) => track.format === "MP3" && !excludedTrackPaths.has(track.path)) ?? [],
    [excludedTrackPaths, taggerAlbum],
  );

  const renameSelectedAlbums = useCallback(async (albumsToRename: InboxAlbum[]) => {
    if (!albumsToRename.length) return;
    setRenameBusy(true);
    setRenameMessage(null);
    setError(null);
    try {
      const result = await renameInboxAlbums(albumsToRename.map((album) => album.path));
      if (result.renamedAlbums) {
        if (albumsToRename.length === 1 && result.failures.length === 0) {
          setRenameMessage(`${result.renamedTracks} ${result.renamedTracks === 1 ? "track" : "tracks"} renamed${result.renamedFolders ? " with the album folder" : ""}.`);
        } else {
          setRenameMessage(`${result.renamedTracks} ${result.renamedTracks === 1 ? "track" : "tracks"} renamed across ${result.renamedAlbums} ${result.renamedAlbums === 1 ? "album" : "albums"}${result.renamedFolders ? ` with ${result.renamedFolders} album ${result.renamedFolders === 1 ? "folder" : "folders"} renamed` : ""}.`);
        }
      }
      if (result.failures.length) {
        const firstFailure = result.failures[0];
        const failedAlbum = albumsToRename.find((album) => album.path === firstFailure.albumPath);
        setError(`${result.failures.length} ${result.failures.length === 1 ? "album" : "albums"} could not be renamed. ${failedAlbum?.album ?? failedAlbum?.folderName ?? leafName(firstFailure.albumPath)}: ${firstFailure.message}`);
      }
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setRenameBusy(false);
    }
  }, [refresh]);

  useEffect(() => {
    function handleInboxShortcut(event: KeyboardEvent) {
      if (event.ctrlKey && event.shiftKey && event.key.toLocaleLowerCase() === "t" && selectedAlbum && !selectedAlbumHasLossless && selectedTracks.length) {
        event.preventDefault();
        setTaggerAlbum(selectedAlbum);
      } else if (event.ctrlKey && !event.shiftKey && event.key.toLocaleLowerCase() === "r" && selectedAlbums.length && !selectedAlbumsHaveLossless && !taggerAlbum && !renameBusy) {
        event.preventDefault();
        void renameSelectedAlbums(selectedAlbums);
      }
    }
    window.addEventListener("keydown", handleInboxShortcut);
    return () => window.removeEventListener("keydown", handleInboxShortcut);
  }, [renameBusy, renameSelectedAlbums, selectedAlbum, selectedAlbumHasLossless, selectedAlbums, selectedAlbumsHaveLossless, selectedTracks.length, taggerAlbum]);

  async function addFolder() {
    try {
      const folder = await selectInboxMonitorFolder();
      if (!folder) return;
      await addInboxMonitorFolder(folder);
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    }
  }

  async function removeFolder(folder: string) {
    try {
      await removeInboxMonitorFolder(folder);
      if (selectedFolder === folder) selectFolder(null);
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    }
  }

  async function repairAlbumCover() {
    if (!selectedAlbum || coverBusy) return;
    setCoverBusy(true);
    setCoverMessage(null);
    setError(null);
    try {
      const imagePath = selectedAlbum.artworkPresent ? null : await selectInboxCoverImage();
      if (!selectedAlbum.artworkPresent && !imagePath) return;
      const result = await embedInboxAlbumCover(selectedAlbum.path, imagePath);
      setCoverMessage(`Embedded the album cover in ${result.trackCount} ${result.trackCount === 1 ? "track" : "tracks"}${result.changedTracks < result.trackCount ? `; ${result.changedTracks} needed updating` : ""}.`);
      setMovePreview(null);
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setCoverBusy(false);
    }
  }

  async function convertSelectedAlbum() {
    if (!selectedAlbum || !selectedAlbum.losslessTrackCount || convertBusy) return;
    setConvertBusy(true);
    setConvertMessage(null);
    setError(null);
    try {
      const result = await convertInboxLossless(selectedAlbum.path);
      if (result.convertedTracks) {
        setConvertMessage(`${result.convertedTracks} ${result.convertedTracks === 1 ? "track" : "tracks"} converted to 320 kbps MP3; ${result.deletedSources} source ${result.deletedSources === 1 ? "file" : "files"} deleted.`);
      }
      if (result.failures.length) {
        const first = result.failures[0];
        setError(`${result.failures.length} ${result.failures.length === 1 ? "track" : "tracks"} could not be converted. ${first.fileName}: ${first.message}`);
      }
      setMovePreview(null);
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setConvertBusy(false);
    }
  }

  function intakeTargetForFolder(folder: string): InboxLibraryIntakeTarget | null {
    if (!snapshot) return null;
    const folderAlbums = snapshot.albums.filter((album) => albumInFolder(album, folder));
    if (!folderAlbums.length) return null;
    return {
      sourcePath: folder,
      label: leafName(folder),
      albumCount: folderAlbums.length,
      unreadyAlbumCount: folderAlbums.filter((album) => !album.readiness.ready).length,
    };
  }

  function openFolderIntake(folder: string) {
    const target = intakeTargetForFolder(folder);
    if (target) setIntakeScope({ label: target.label, targets: [target] });
  }

  function openAllFoldersIntake() {
    if (!snapshot) return;
    const targets = snapshot.settings.monitoredFolders.map(intakeTargetForFolder).filter((target): target is InboxLibraryIntakeTarget => Boolean(target));
    if (targets.length) setIntakeScope({ label: "all folders", targets });
  }

  async function previewMove() {
    if (!selectedAlbum || !moveCategory) return;
    setMoveBusy(true);
    setMoveMessage(null);
    resetMoveProgress();
    try {
      setMovePreview(await libraryIntakeAdapter.preview({ sourcePath: selectedAlbum.path, category: moveCategory }));
      setReplacementConfirmed(false);
    } catch (nextError) {
      setMoveMessage(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setMoveBusy(false);
    }
  }

  async function applyMove() {
    if (!movePreview || (movePreview.albums.some((album) => album.action === "replace") && !replacementConfirmed)) return;
    setMoveBusy(true);
    setMoveMessage(null);
    resetMoveProgress();
    try {
      const result = await libraryIntakeAdapter.apply({ planId: movePreview.planId, sessionId: movePreview.sessionId });
      await onCatalogChanged();
      setMovePreview(null);
      setMoveCategory("");
      setMoveMessage(`${result.albumCount} ${result.albumCount === 1 ? "album" : "albums"} moved and cataloged.`);
      await refresh();
    } catch (nextError) {
      setMoveMessage(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setMoveBusy(false);
    }
  }

  if (!snapshot && loadState === "loading") return <section className="inbox-load" aria-live="polite"><LoaderCircle className="is-spinning" /><span>Scanning monitored folders…</span></section>;
  if (!snapshot && loadState === "error") return <section className="inbox-load inbox-load--error" role="alert"><AlertTriangle /><strong>Inbox could not scan your folders.</strong><span>{error}</span><button className="button button--primary" onClick={() => void refresh()}>Try again</button></section>;
  if (!snapshot) return null;

  return (
    <section className="inbox-page" aria-label="Inbox">
      <header className="inbox-page__header">
        <div><h1>Inbox</h1><p>Review and tag new music before adding it to your library.</p></div>
        <div>
          <button type="button" onClick={() => void refresh()} disabled={loadState === "loading"}><RefreshCw className={loadState === "loading" ? "is-spinning" : ""} /> Rescan</button>
          <button type="button" disabled={!selectedAlbums.length || selectedAlbumsHaveLossless || renameBusy} onClick={() => void renameSelectedAlbums(selectedAlbums)}>{renameBusy ? <LoaderCircle className="is-spinning" /> : <FilePenLine />} Rename{selectedAlbums.length > 1 ? ` ${selectedAlbums.length} albums` : ""} <kbd>Ctrl R</kbd></button>
          <button type="button" className="button button--primary" disabled={!selectedAlbum || selectedAlbumHasLossless || !selectedTracks.length || renameBusy} onClick={() => selectedAlbum && setTaggerAlbum(selectedAlbum)}><Tags /> Auto-tag {selectedAlbum && selectedTracks.length !== selectedAlbum.trackCount ? `${selectedTracks.length} tracks` : ""} <kbd>Ctrl Shift T</kbd></button>
        </div>
      </header>

      {error ? <p className="inbox-banner" role="alert"><AlertTriangle />{error}</p> : null}
      {renameMessage ? <p className="inbox-banner inbox-banner--success" role="status"><CheckCircle2 />{renameMessage}</p> : null}
      {coverMessage ? <p className="inbox-banner inbox-banner--success" role="status"><CheckCircle2 />{coverMessage}</p> : null}
      {convertMessage ? <p className="inbox-banner inbox-banner--success" role="status"><CheckCircle2 />{convertMessage}</p> : null}
      {intakeMessage ? <p className="inbox-banner inbox-banner--success" role="status"><CheckCircle2 />{intakeMessage}</p> : null}
      {snapshot.settings.warning ? <p className="inbox-banner" role="status"><AlertTriangle />{snapshot.settings.warning}</p> : null}

      <div className="inbox-workspace">
        <aside className="inbox-folders" aria-label="Monitored folders">
          <header><strong>Monitored folders</strong><button type="button" disabled={snapshot.settings.monitoredFolders.length >= 10} onClick={() => void addFolder()}><FolderPlus /> Add folder</button></header>
          <div className={`inbox-folder-row inbox-folder-row--all${!selectedFolder ? " is-selected" : ""}`}>
            <button type="button" onClick={() => selectFolder(null)}><InboxIcon /><span><strong>All folders</strong><small>{snapshot.albums.length} albums</small></span></button>
            <button type="button" className="inbox-folder-add" aria-label="Add All folders to library" title="Add All folders to library" disabled={!snapshot.albums.length} onClick={() => openAllFoldersIntake()}><FolderInput /></button>
          </div>
          {snapshot.settings.monitoredFolders.map((folder) => {
            const count = snapshot.albums.filter((album) => albumInFolder(album, folder)).length;
            return <div className={`inbox-folder-row${selectedFolder === folder ? " is-selected" : ""}`} key={folder}>
              <button type="button" onClick={() => selectFolder(folder)} title={folder}><Folder /><span><strong>{leafName(folder)}</strong><small>{folder}</small></span><output>{count}</output></button>
              <span className="inbox-folder-actions">
                <button type="button" className="inbox-folder-add" aria-label={`Add ${leafName(folder)} to library`} title="Add folder to library" disabled={!count} onClick={() => openFolderIntake(folder)}><FolderInput /></button>
                <button type="button" aria-label={`Stop monitoring ${folder}`} title="Stop monitoring" onClick={() => void removeFolder(folder)}><Trash2 /></button>
              </span>
            </div>;
          })}
          {snapshot.settings.monitoredFolders.length === 0 ? <div className="inbox-folders__empty"><FolderPlus /><strong>Add your first monitored folder</strong><small>Aurora scans it without adding anything to the library.</small><button type="button" onClick={() => void addFolder()}>Choose folder</button></div> : null}
          <footer>{snapshot.settings.monitoredFolders.length} of 10 folders <span>Last scan {formatTime(snapshot.scannedAtMs)}</span></footer>
        </aside>

        <section className="inbox-albums" aria-label="Staged albums">
          <header><strong>{selectedFolder ? leafName(selectedFolder) : "All staged albums"}</strong><span aria-live="polite">{selectedAlbumIds.size} selected · {albums.length} {albums.length === 1 ? "album" : "albums"} outside the library</span></header>
          {albums.length ? <div className="inbox-table-wrap"><table><thead><tr><th>Album</th><th>Artist</th><th>Year</th><th>Tracks</th><th>Status</th><th>Updated</th></tr></thead><tbody>
            {albums.map((album) => {
              const isSelected = selectedAlbumIds.has(album.id);
              const isActive = selectedAlbum?.id === album.id;
              return <tr
                key={album.id}
                className={`${isSelected ? "is-selected" : ""}${isActive ? " is-active" : ""}`.trim()}
                aria-label={`${album.album ?? album.folderName} by ${album.artist ?? "Unknown artist"}`}
                aria-selected={isSelected}
                aria-current={isActive ? "true" : undefined}
                onMouseDown={(event) => { if (event.shiftKey) event.preventDefault(); }}
                onClick={(event) => selectAlbum(album, event.ctrlKey || event.metaKey, event.shiftKey)}
              >
              <td><InboxArtwork album={album} size={64} /><span><strong>{album.album ?? album.folderName}</strong><small>{album.folderName}</small></span></td>
              <td>{album.artist ?? "Unknown artist"}</td><td>{album.year ?? "—"}</td><td>{album.trackCount}</td>
              <td><span className={album.readiness.ready ? "inbox-status is-ready" : "inbox-status"}>{album.readiness.ready ? <CheckCircle2 /> : <AlertTriangle />}{album.readiness.ready ? "Ready" : `${album.readiness.issues.length} ${album.readiness.issues.length === 1 ? "issue" : "issues"}`}</span></td>
              <td>{formatTime(album.modifiedAtMs)}</td>
            </tr>;
            })}
          </tbody></table></div> : <div className="inbox-albums__empty"><CheckCircle2 /><strong>Nothing is waiting here.</strong><span>New album folders will appear on the next scan.</span></div>}
        </section>

        <aside className="inbox-inspector" aria-label="Selected Inbox album">
          {selectedAlbum ? <div className="inbox-inspector-tabs" role="tablist" aria-label="Inbox album details">
            <button type="button" role="tab" aria-selected={inspectorView === "album"} onClick={() => setInspectorView("album")}>Album</button>
            <button type="button" role="tab" aria-selected={inspectorView === "tags" && !selectedAlbumsHaveLossless} disabled={selectedAlbumsHaveLossless} onClick={() => setInspectorView("tags")}>Tags</button>
          </div> : null}
          {selectedAlbum && inspectorView === "tags" && !selectedAlbumsHaveLossless ? <div className="inbox-manual-tags">
            <section className="inbox-track-selection"><header><h3>Tracks to edit</h3><span><button type="button" onClick={() => setExcludedTrackPaths(new Set())}>All</button><button type="button" onClick={() => setExcludedTrackPaths(new Set(selectedAlbumTrackRows.map(({ track }) => track.path)))}>None</button></span></header><div>{selectedAlbumTrackRows.map(({ album, track, index }) => <label key={track.path}><input type="checkbox" aria-label={`${album.album ?? album.folderName} — ${track.discNumber ? `${track.discNumber}-` : ""}${String(track.trackNumber ?? index + 1).padStart(2, "0")} ${track.title ?? track.fileName}`} checked={!excludedTrackPaths.has(track.path)} onChange={() => setExcludedTrackPaths((current) => { const next = new Set(current); if (next.has(track.path)) next.delete(track.path); else next.add(track.path); return next; })} /><span>{track.discNumber ? `${track.discNumber}-` : ""}{String(track.trackNumber ?? index + 1).padStart(2, "0")}</span><strong>{track.title ?? track.fileName}</strong></label>)}</div><small>{tagEditorTracks.length} of {selectedAlbumTrackRows.length} selected across {selectedAlbums.length} {selectedAlbums.length === 1 ? "album" : "albums"}</small></section>
            {tagEditorTracks.length ? <InboxTagEditor
              key={`${selectedAlbums.map((album) => album.id).join("|")}:${tagEditorTracks.map((track) => track.path).join("|")}`}
              albums={selectedAlbums}
              tracks={tagEditorTracks}
              onApplied={() => refresh()}
            /> : <p className="inbox-manual-tags__empty">Select one or more tracks to edit their tags.</p>}
          </div> : selectedAlbum ? <>
            <header><InboxArtwork album={selectedAlbum} size={128} decorative={false} /><div><h2>{selectedAlbum.album ?? selectedAlbum.folderName}</h2><p>{selectedAlbum.artist ?? "Unknown artist"}</p></div></header>
            <dl><div><dt>Status</dt><dd className={selectedAlbum.readiness.ready ? "is-ready" : "has-issues"}>{selectedAlbum.readiness.ready ? "Ready" : "Needs attention"}</dd></div><div><dt>Folder</dt><dd title={selectedAlbum.path}>{selectedAlbum.path}</dd></div><div><dt>Tracks</dt><dd>{selectedAlbum.trackCount}</dd></div><div><dt>Format</dt><dd>{selectedAlbum.formats?.length ? selectedAlbum.formats.join(" · ") : "Unknown"}</dd></div><div><dt>Bitrate</dt><dd>{selectedAlbum.avgBitrateKbps ? `${selectedAlbum.avgBitrateKbps} kbps average` : "Unknown"}</dd></div><div><dt>Audio</dt><dd>{formatInboxSize(selectedAlbum.totalSizeBytes ?? 0)} · {formatDuration(Math.round((selectedAlbum.durationMs ?? 0) / 1000))}</dd></div><div><dt>Artwork</dt><dd className={selectedAlbum.artworkReady ? "is-ready" : "has-issues"}>{selectedAlbum.artworkTrackCount} / {selectedAlbum.trackCount} embedded</dd></div><div><dt>Genre</dt><dd>{selectedAlbum.genre ?? "Missing"}</dd></div><div><dt>Publisher</dt><dd>{selectedAlbum.publisher ?? "Missing"}</dd></div></dl>
            <section><h3>Readiness</h3>{selectedAlbum.readiness.ready ? <p className="inbox-check"><CheckCircle2 /> Tags and embedded artwork are ready for intake.</p> : <ul>{selectedAlbum.readiness.issues.map((issue) => <li key={issue}><AlertTriangle />{issue}</li>)}</ul>}</section>
            {selectedAlbumHasLossless ? <button type="button" className="inbox-autotag inbox-convert" disabled={convertBusy} onClick={() => void convertSelectedAlbum()}>{convertBusy ? <LoaderCircle className="is-spinning" /> : <AudioLines />}<span><strong>Convert to 320 kbps MP3</strong><small>Convert {selectedAlbum.losslessTrackCount} FLAC/APE {selectedAlbum.losslessTrackCount === 1 ? "track" : "tracks"} here, verify each MP3, then delete each source</small></span></button> : null}
            {!selectedAlbumHasLossless && !selectedAlbum.artworkReady ? <button type="button" className="inbox-autotag inbox-artwork" disabled={coverBusy} onClick={() => void repairAlbumCover()}>{coverBusy ? <LoaderCircle className="is-spinning" /> : <ImagePlus />}<span><strong>{selectedAlbum.artworkPresent ? "Embed cover in all tracks" : "Choose album cover"}</strong><small>{selectedAlbum.artworkPresent ? `Use the displayed cover for all ${selectedAlbum.trackCount} MP3s` : "Select a JPG, PNG, GIF, BMP, or WebP image"}</small></span></button> : null}
            {!selectedAlbumHasLossless ? <>
            <section className="inbox-track-selection"><header><h3>Tracks to tag</h3><span><button type="button" onClick={() => setExcludedTrackPaths(new Set())}>All</button><button type="button" onClick={() => setExcludedTrackPaths(new Set(selectedAlbum.tracks.map((track) => track.path)))}>None</button></span></header><div>{selectedAlbum.tracks.map((track, index) => <label key={track.path}><input type="checkbox" checked={!excludedTrackPaths.has(track.path)} onChange={() => setExcludedTrackPaths((current) => { const next = new Set(current); if (next.has(track.path)) next.delete(track.path); else next.add(track.path); return next; })} /><span>{track.discNumber ? `${track.discNumber}-` : ""}{String(track.trackNumber ?? index + 1).padStart(2, "0")}</span><strong>{track.title ?? track.fileName}</strong></label>)}</div><small>{selectedTracks.length} of {selectedAlbum.trackCount} selected</small></section>
            <button type="button" className="inbox-autotag" disabled={!selectedTracks.length} onClick={() => setTaggerAlbum(selectedAlbum)}><Tags /><span><strong>Album Auto-Tagger</strong><small>{selectedTracks.length === selectedAlbum.trackCount ? "Match the full album" : `Match ${selectedTracks.length} selected tracks`}</small></span><kbd>Ctrl Shift T</kbd></button>
            <button type="button" className="inbox-autotag inbox-rename" disabled={!selectedAlbums.length || selectedAlbumsHaveLossless || renameBusy} onClick={() => void renameSelectedAlbums(selectedAlbums)}><FilePenLine /><span><strong>Rename from tags</strong><small>{selectedAlbums.length > 1 ? `Standardize ${selectedAlbums.length} selected album folders and track filenames` : "Standardize the album folder and track filenames"}</small></span><kbd>Ctrl R</kbd></button>
            </> : null}
            <section className="inbox-move"><h3>Move to library</h3><p>Uses the same reviewed, preview-first flow as Add Music.</p><select aria-label="Library destination" value={moveCategory} onChange={(event) => { setMoveCategory(event.target.value as LibraryIntakeCategoryId | ""); setMovePreview(null); setReplacementConfirmed(false); }}><option value="">Select destination…</option>{libraryIntakeCategories.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}</select>
              {movePreview ? <div className="inbox-move__preview"><Check /><span><strong>{movePreview.trackCount} tracks verified</strong><small>{movePreview.category.destinationRoot}</small></span></div> : null}
              {moveBusy ? <LibraryIntakeActivity mode={movePreview ? "apply" : "preview"} progress={moveProgress} /> : null}
              {movePreview?.albums.some((album) => album.action === "replace") ? <label className="inbox-move__replacement"><AlertTriangle /><span><strong>Replace existing release</strong><small>{movePreview.albums[0].existingTrackCount} existing → {movePreview.albums[0].trackCount} new tracks · old release preserved for recovery</small></span><input type="checkbox" aria-label="Confirm replacement" checked={replacementConfirmed} onChange={(event) => setReplacementConfirmed(event.target.checked)} /></label> : null}
              {moveMessage ? <p className="inbox-move__message" role="status">{moveMessage}</p> : null}
              <button type="button" className="button button--primary" disabled={!moveCategory || moveBusy || !selectedAlbum.readiness.ready || Boolean(movePreview?.albums.some((album) => album.action === "replace") && !replacementConfirmed)} onClick={() => void (movePreview ? applyMove() : previewMove())}>{moveBusy ? <LoaderCircle className="is-spinning" /> : <ArrowRight />}{movePreview?.albums.some((album) => album.action === "replace") ? "Replace and catalog" : movePreview ? "Move and catalog" : "Preview move"}</button>
            </section>
          </> : <div className="inbox-inspector__empty"><InboxIcon /><span>Select an album to review it.</span></div>}
        </aside>
      </div>

      {taggerAlbum ? <AlbumAutoTagger album={taggerAlbum} tracks={taggerTracks} discogsConfigured={snapshot.settings.discogsConfigured} onOpenSettings={onOpenMetadataSettings} onClose={() => setTaggerAlbum(null)} onApplied={async (message) => { setIntakeMessage(message); setTaggerAlbum(null); await refresh(); }} /> : null}
      {intakeScope ? <InboxLibraryIntakeDialog
        scopeLabel={intakeScope.label}
        targets={intakeScope.targets}
        onClose={() => setIntakeScope(null)}
        onApplied={async () => { await onCatalogChanged(); await refresh(); }}
        onCompleted={setIntakeMessage}
      /> : null}
    </section>
  );
}

function AlbumAutoTagger({ album, tracks, discogsConfigured, onOpenSettings, onClose, onApplied }: { album: InboxAlbum; tracks: InboxTrack[]; discogsConfigured: boolean; onOpenSettings: () => void; onClose: () => void; onApplied: (message: string) => void | Promise<void> }) {
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
  const [renameAfterApply, setRenameAfterApply] = useState(tracks.length === album.tracks.length);
  const [discNumber, setDiscNumber] = useState("");
  const [discTotal, setDiscTotal] = useState("");
  const [busy, setBusy] = useState<"search" | "detail" | "apply" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [genreSuggestions, setGenreSuggestions] = useState<string[]>([]);
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
    setBusy("search"); setError(null); setDetail(null); setSelectedCandidate(null);
    try {
      const result = await searchInboxReleases(searchArtist, searchTitle, tracks.length, preferOriginalEdition);
      setCandidates(result.candidates); setWarnings(result.warnings);
      if (result.candidates[0]) {
        setSelectedCandidate(result.candidates[0]);
        setBusy("detail");
        const next = await loadInboxReleaseDetail(result.candidates[0]);
        setDetail(next); hydrateDetail(next);
      }
    } catch (nextError) { setError(nextError instanceof Error ? nextError.message : String(nextError)); }
    finally { setBusy(null); }
  }, [hydrateDetail, preferOriginalEdition, tracks.length]);

  useEffect(() => {
    const initial = window.setTimeout(() => void performSearch(album.artist ?? "", album.album ?? album.folderName), 0);
    return () => window.clearTimeout(initial);
  }, [album.album, album.artist, album.folderName, performSearch]);
  useEffect(() => { const close = (event: KeyboardEvent) => { if (event.key === "Escape" && !busy) onClose(); }; window.addEventListener("keydown", close); return () => window.removeEventListener("keydown", close); }, [busy, onClose]);

  async function chooseCandidate(candidate: ReleaseCandidate) {
    setSelectedCandidate(candidate); setBusy("detail"); setError(null);
    try { const next = await loadInboxReleaseDetail(candidate); setDetail(next); hydrateDetail(next); }
    catch (nextError) { setError(nextError instanceof Error ? nextError.message : String(nextError)); }
    finally { setBusy(null); }
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
  const willRename = renameAfterApply
    && tracks.length === album.tracks.length
    && (reconciliation.extraCount === 0 || allExtrasSelected);

  async function apply() {
    if (!detail || fields.size === 0) return;
    if (removeExtraPaths.size > 0 && !confirmRemoval) {
      setConfirmRemoval(true);
      return;
    }
    setBusy("apply"); setError(null);
    try {
      const result = await applyInboxTags({
        albumPath: album.path,
        fields: [...fields],
        tracks: reconciliation.rows.flatMap((match) => {
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
        }),
        renameAfterApply: willRename,
        removeTrackPaths: [...removeExtraPaths],
      });
      await onApplied(result.removedTracks
        ? `Auto-tagged the selected edition and moved ${result.removedTracks} unmatched ${result.removedTracks === 1 ? "track" : "tracks"} to Aurora recovery${result.recoveryPath ? ` at ${result.recoveryPath}` : ""}.`
        : `Auto-tagged ${result.changedTracks} ${result.changedTracks === 1 ? "track" : "tracks"}${result.renamedTracks ? " and renamed the album" : ""}.`);
    } catch (nextError) { setError(nextError instanceof Error ? nextError.message : String(nextError)); setBusy(null); }
  }

  return <div className="modal-backdrop inbox-tagger-backdrop" role="presentation"><section className="inbox-tagger" role="dialog" aria-modal="true" aria-labelledby="inbox-tagger-title" aria-busy={Boolean(busy)}>
    <header><div className="inbox-tagger__mark"><Tags /></div><div><p className="eyebrow">Inbox metadata</p><h2 id="inbox-tagger-title">Album Auto-Tagger</h2></div><button type="button" aria-label="Close auto-tagger" disabled={Boolean(busy)} onClick={onClose}><X /></button></header>
    <div className="inbox-tagger__content">
    <div className="inbox-tagger__search"><label>Album artist<input value={artist} onChange={(event) => setArtist(event.target.value)} /></label><label>Album<input value={title} onChange={(event) => setTitle(event.target.value)} /></label><label className="inbox-original-preference"><input type="checkbox" checked={preferOriginalEdition} disabled={Boolean(busy)} onChange={(event) => setPreferOriginalEdition(event.target.checked)} /><span><strong>Prefer the original edition</strong><small>Prioritize the earliest release, then compare its track list.</small></span></label><button type="button" className="button button--primary" disabled={Boolean(busy) || !artist.trim() || !title.trim()} onClick={() => void performSearch(artist, title)}>{busy === "search" ? <LoaderCircle className="is-spinning" /> : <Search />} Find</button></div>
    <div className="inbox-tagger__notices">
      {!discogsConfigured ? <div className="inbox-provider-note"><AlertTriangle /><span><strong>Discogs is not connected.</strong><small>MusicBrainz results are still available.</small></span><button type="button" onClick={() => { onClose(); onOpenSettings(); }}><Settings /> Connect</button></div> : null}
      {warnings.map((warning) => <p className="inbox-tagger__warning" key={warning}><AlertTriangle />{warning}</p>)}
      {error ? <p className="inbox-tagger__error" role="alert"><AlertTriangle />{error}</p> : null}
    </div>
    <div className="inbox-tagger__body">
      <section className="inbox-candidates" aria-label="Release matches"><table><thead><tr><th>Source</th><th>Score</th><th>Album</th><th>Artist</th><th>Original</th><th>Edition</th><th>Tracks</th><th>Format</th><th>Publisher</th></tr></thead><tbody>
        {candidates.map((candidate) => <tr key={`${candidate.source}:${candidate.id}`} className={selectedCandidate?.id === candidate.id && selectedCandidate.source === candidate.source ? "is-selected" : ""} onClick={() => void chooseCandidate(candidate)}><td><span className={`provider provider--${candidate.source}`}>{sourceLabel(candidate.source)}</span></td><td>{candidate.score}%</td><td>{candidate.title}</td><td>{candidate.artist}</td><td>{candidate.originalYear ?? "—"}</td><td>{candidate.year ?? "—"}</td><td>{candidate.trackCount ?? "—"}{candidate.trackCount !== null ? candidate.trackCount === tracks.length ? " · exact" : ` · ${Math.abs(tracks.length - candidate.trackCount)} ${tracks.length > candidate.trackCount ? "extra" : "missing"}` : ""}</td><td>{candidate.format ?? "—"}</td><td>{candidate.publisher ?? "—"}</td></tr>)}
        {!candidates.length && busy !== "search" ? <tr><td colSpan={9}>No release matches found. Broaden the artist or album spelling.</td></tr> : null}
      </tbody></table></section>
      <div className="inbox-tagger__editor">
        <section className="inbox-release-fields"><div className="inbox-release-art"><Disc3 /></div><div className="inbox-release-form"><datalist id="inbox-auto-tagger-genre-suggestions">{genreSuggestions.map((genre) => <option value={genre} key={genre} />)}</datalist><label className="inbox-release-form__disc-total">Disc total<input inputMode="numeric" placeholder="Release" value={discTotal} onChange={(event) => setDiscTotal(event.target.value.replace(/\D/g, "").slice(0, 3))} /></label><label className="inbox-release-form__disc-override">Disc # override<input aria-label="Disc number override" inputMode="numeric" placeholder="Release" value={discNumber} onChange={(event) => setDiscNumber(event.target.value.replace(/\D/g, "").slice(0, 3))} /></label><label className="inbox-release-form__album-artist">Album artist<input value={values.albumArtist} onChange={(event) => setValues((current) => ({ ...current, albumArtist: event.target.value }))} /></label><label className="inbox-release-form__album">Album<input value={values.album} onChange={(event) => setValues((current) => ({ ...current, album: event.target.value }))} /></label><label className="inbox-release-form__publisher">Publisher<input value={values.publisher} onChange={(event) => setValues((current) => ({ ...current, publisher: event.target.value }))} /></label><label className="inbox-release-form__year">Original year<input inputMode="numeric" value={values.year} onChange={(event) => setValues((current) => ({ ...current, year: event.target.value.replace(/\D/g, "").slice(0, 4) }))} /></label><label className="inbox-release-form__release-year">Release year<input inputMode="numeric" value={values.releaseYear} onChange={(event) => setValues((current) => ({ ...current, releaseYear: event.target.value.replace(/\D/g, "").slice(0, 4) }))} /></label><label className="inbox-release-form__genre">Genre<input list="inbox-auto-tagger-genre-suggestions" value={values.genre} onChange={(event) => setValues((current) => ({ ...current, genre: event.target.value }))} /></label></div></section>
        <fieldset className="inbox-fields"><legend>Include fields</legend>{allFields.map((field) => <label key={field.id}><input type="checkbox" checked={fields.has(field.id)} onChange={() => toggleField(field.id)} />{field.label}</label>)}</fieldset>
      </div>
      <section className="inbox-track-compare">
        {detail ? <div className="inbox-reconciliation-summary" role="status"><strong>{reconciliation.matchedCount} of {detail.tracks.length} release tracks matched</strong><span>{reconciliation.extraCount ? `${reconciliation.extraCount} extra local ${reconciliation.extraCount === 1 ? "track" : "tracks"}` : "No extra local tracks"}{reconciliation.missingCount || reconciliation.ambiguousCount ? ` · ${reconciliation.missingCount + reconciliation.ambiguousCount} unresolved` : ""}</span>{reconciliation.extraCount && !reconciliation.cleanupSafe ? <small>Choose an unmatched file and confirm any true match before removing extras.</small> : null}</div> : null}
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
    <footer><label className="inbox-rename-option"><input type="checkbox" checked={renameAfterApply} disabled={Boolean(busy) || tracks.length !== album.tracks.length || (reconciliation.extraCount > 0 && !allExtrasSelected)} onChange={(event) => setRenameAfterApply(event.target.checked)} /><span><strong>Rename after tagging</strong><small>{tracks.length !== album.tracks.length ? "Tag all album tracks before renaming" : reconciliation.extraCount > 0 && !allExtrasSelected ? "Select every extra track for recovery before renaming" : "Album Artist - Album (Original year) · 1-01 or 01 - Artist - Title"}</small></span></label><span className="inbox-tagger__source">{detail ? `${sourceLabel(detail.candidate.source)} release ${detail.candidate.id}` : "Choose a release to review its tags."}</span><button type="button" className="button button--quiet" disabled={Boolean(busy)} onClick={onClose}>Cancel</button><button type="button" className="button button--primary" disabled={!detail || fields.size === 0 || reconciliation.matchedCount === 0 || Boolean(busy)} onClick={() => void apply()}>{busy === "apply" ? <LoaderCircle className="is-spinning" /> : selectedRemovalCount ? <Trash2 /> : <Check />} {selectedRemovalCount ? `${willRename ? "Apply, rename & move" : "Apply & move"} ${selectedRemovalCount} ${selectedRemovalCount === 1 ? "extra" : "extras"}` : willRename ? "Apply & rename" : `Apply to ${reconciliation.matchedCount} tracks`}</button></footer>
    {confirmRemoval ? <div className="inbox-extra-confirmation-backdrop" role="presentation"><dialog className="inbox-extra-confirmation" open role="alertdialog" aria-modal="true" aria-labelledby="inbox-extra-confirmation-title" aria-describedby="inbox-extra-confirmation-description" onKeyDown={(event) => { if (event.key === "Escape" && !busy) { event.stopPropagation(); setConfirmRemoval(false); } }}><span className="inbox-extra-confirmation__icon"><Trash2 aria-hidden="true" /></span><div><h3 id="inbox-extra-confirmation-title">Move {selectedRemovalCount} unmatched {selectedRemovalCount === 1 ? "track" : "tracks"} out of this album?</h3><p id="inbox-extra-confirmation-description">Aurora will copy and verify {selectedRemovalCount === 1 ? "this MP3" : "these MP3s"} in its recovery folder before removing {selectedRemovalCount === 1 ? "it" : "them"} from the Inbox album. Tagging and renaming roll back if removal fails.</p><ul>{[...removeExtraPaths].map((path) => <li key={path}>{leafName(path)}</li>)}</ul></div><div className="inbox-extra-confirmation__actions"><button type="button" disabled={Boolean(busy)} onClick={() => setConfirmRemoval(false)}>Cancel</button><button type="button" className="is-destructive" disabled={Boolean(busy)} autoFocus onClick={() => void apply()}>{busy === "apply" ? <LoaderCircle className="is-spinning" /> : <Trash2 />}Move to recovery</button></div></dialog></div> : null}
  </section></div>;
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
