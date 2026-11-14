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
  Tags,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  addInboxMonitorFolder,
  convertInboxLossless,
  embedInboxAlbumCover,
  fillInboxTrackTotals,
  inboxCoverUrl,
  loadInboxSnapshot,
  removeInboxMonitorFolder,
  renameInboxAlbums,
  selectInboxCoverImage,
  selectInboxMonitorFolder,
  type InboxAlbum,
  type InboxSnapshot,
} from "../../inbox";
import {
  libraryIntakeAdapter,
  libraryIntakeCategories,
  type LibraryIntakeCategoryId,
  type LibraryIntakePreview,
} from "../../ingest";
import { formatDuration } from "../../library";
import { mergeRefreshedInboxAlbums } from "../../inboxRefresh";
import { InboxTagEditor } from "./InboxTagEditor";
import { AlbumAutoTagger } from "./AlbumAutoTagger";
import { InboxLibraryIntakeDialog, type InboxLibraryIntakeTarget } from "./InboxLibraryIntakeDialog";
import { LibraryIntakeActivity } from "./LibraryIntakeActivity";
import { useLibraryIntakeProgress } from "./useLibraryIntakeProgress";
import { useInboxMessages } from "./useInboxMessages";
import { InboxMessageHistoryButton, InboxMessages } from "./InboxMessages";
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

export function Inbox({ onOpenMetadataSettings, onCatalogChanged }: InboxProps) {
  const [snapshot, setSnapshot] = useState<InboxSnapshot | null>(null);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [error, setError] = useState<string | null>(null);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [selectedAlbumId, setSelectedAlbumId] = useState<string | null>(null);
  const [selectedAlbumIds, setSelectedAlbumIds] = useState<ReadonlySet<string>>(new Set());
  const [albumSelectionAnchorId, setAlbumSelectionAnchorId] = useState<string | null>(null);
  const [taggerAlbums, setTaggerAlbums] = useState<InboxAlbum[]>([]);
  const taggerOpen = taggerAlbums.length > 0;
  const [moveCategory, setMoveCategory] = useState<LibraryIntakeCategoryId | "">("");
  const [movePreview, setMovePreview] = useState<LibraryIntakePreview | null>(null);
  const [moveBusy, setMoveBusy] = useState(false);
  const [moveRefreshing, setMoveRefreshing] = useState(false);
  const [replacementConfirmed, setReplacementConfirmed] = useState(false);
  const [renameBusy, setRenameBusy] = useState(false);
  const [trackTotalsBusy, setTrackTotalsBusy] = useState(false);
  const [coverBusy, setCoverBusy] = useState(false);
  const [convertBusy, setConvertBusy] = useState(false);
  const [excludedTrackPaths, setExcludedTrackPaths] = useState<Set<string>>(new Set());
  const [inspectorView, setInspectorView] = useState<"album" | "tags">("album");
  const [intakeScope, setIntakeScope] = useState<{ label: string; targets: InboxLibraryIntakeTarget[] } | null>(null);
  const { messages, notify, dismiss } = useInboxMessages();
  const [historyOpen, setHistoryOpen] = useState(false);
  const { progress: moveProgress, reset: resetMoveProgress } = useLibraryIntakeProgress();

  const refresh = useCallback(async (quiet = false, renamedAlbum?: { id: string; path: string }) => {
    if (!quiet) setLoadState("loading");
    try {
      const next = await loadInboxSnapshot();
      const renamed = renamedAlbum && next.albums.find((album) => album.path === renamedAlbum.path);
      const identityChange = renamed && { from: renamedAlbum.id, to: renamed.id };
      const currentId = (id: string | null) => id === identityChange?.from ? identityChange.to : id;
      setSnapshot((current) => ({ ...next, albums: mergeRefreshedInboxAlbums(current?.albums ?? [], next.albums, identityChange) }));
      setError(null);
      setLoadState("ready");
      setSelectedAlbumId((current) => {
        const id = currentId(current);
        return id && next.albums.some((album) => album.id === id) ? id : next.albums[0]?.id ?? null;
      });
      setSelectedAlbumIds((current) => {
        const availableIds = new Set(next.albums.map((album) => album.id));
        const retained = new Set([...current].map((id) => currentId(id) ?? id).filter((id) => availableIds.has(id)));
        if (!retained.size && next.albums[0]) retained.add(next.albums[0].id);
        return retained;
      });
      setAlbumSelectionAnchorId((current) => {
        const id = currentId(current);
        return id && next.albums.some((album) => album.id === id) ? id : next.albums[0]?.id ?? null;
      });
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
  const selectedAlbumsMissingTotals = selectedAlbums.filter((album) => album.tracks.some((track) => track.trackTotal === null));
  const trackTotalsDisabled = !selectedAlbumsMissingTotals.length || selectedAlbumsHaveLossless || trackTotalsBusy || renameBusy || convertBusy || coverBusy || moveBusy || taggerOpen || Boolean(intakeScope);

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
    () => taggerAlbums.flatMap((album) => album.tracks.filter((track) => track.format === "MP3" && !excludedTrackPaths.has(track.path))),
    [excludedTrackPaths, taggerAlbums],
  );
  // Several selected albums (e.g. CD1 + CD2 folders) are auto-tagged as one release, ordered by disc.
  const taggableAlbums = useMemo(
    () => selectedAlbums.length > 1 ? orderDiscAlbums(selectedAlbums) : selectedAlbum ? [selectedAlbum] : [],
    [selectedAlbum, selectedAlbums],
  );
  const taggableTrackCount = selectedAlbums.length > 1
    ? taggableAlbums.reduce((total, album) => total + album.tracks.filter((track) => track.format === "MP3" && !excludedTrackPaths.has(track.path)).length, 0)
    : selectedTracks.length;
  const taggableHaveLossless = selectedAlbums.length > 1 ? selectedAlbumsHaveLossless : selectedAlbumHasLossless;

  const renameSelectedAlbums = useCallback(async (albumsToRename: InboxAlbum[]) => {
    if (!albumsToRename.length) return;
    setRenameBusy(true);
    try {
      const result = await renameInboxAlbums(albumsToRename.map((album) => album.path));
      if (result.renamedAlbums) {
        if (albumsToRename.length === 1 && result.failures.length === 0) {
          notify(`${result.renamedTracks} ${result.renamedTracks === 1 ? "track" : "tracks"} renamed${result.renamedFolders ? " with the album folder" : ""}.`);
        } else {
          notify(`${result.renamedTracks} ${result.renamedTracks === 1 ? "track" : "tracks"} renamed across ${result.renamedAlbums} ${result.renamedAlbums === 1 ? "album" : "albums"}${result.renamedFolders ? ` with ${result.renamedFolders} album ${result.renamedFolders === 1 ? "folder" : "folders"} renamed` : ""}.`);
        }
      }
      if (result.failures.length) {
        const firstFailure = result.failures[0];
        const failedAlbum = albumsToRename.find((album) => album.path === firstFailure.albumPath);
        notify(`${result.failures.length} ${result.failures.length === 1 ? "album" : "albums"} could not be renamed. ${failedAlbum?.album ?? failedAlbum?.folderName ?? leafName(firstFailure.albumPath)}: ${firstFailure.message}`, "error");
      }
      await refresh();
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    } finally {
      setRenameBusy(false);
    }
  }, [notify, refresh]);

  async function fillSelectedTrackTotals() {
    if (trackTotalsDisabled) return;
    const targets = [...selectedAlbums];
    setTrackTotalsBusy(true);
    setMovePreview(null);
    setReplacementConfirmed(false);
    try {
      const result = await fillInboxTrackTotals(targets.map((album) => album.path));
      await refresh();
      if (result.changedTracks) {
        notify(`Filled missing track totals in ${result.changedTracks} ${result.changedTracks === 1 ? "track" : "tracks"} across ${result.changedAlbums} ${result.changedAlbums === 1 ? "album" : "albums"}.`);
      } else if (!result.failures.length) {
        notify("All selected albums already have track totals.");
      }
      if (result.failures.length) {
        const first = result.failures[0];
        const album = targets.find((target) => target.path === first.albumPath);
        notify(`${result.failures.length} ${result.failures.length === 1 ? "album" : "albums"} could not have track totals filled. ${album?.album ?? leafName(first.albumPath)}: ${first.message}`, "error");
      }
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    } finally {
      setTrackTotalsBusy(false);
    }
  }

  useEffect(() => {
    function handleInboxShortcut(event: KeyboardEvent) {
      if (trackTotalsBusy) return;
      if (event.ctrlKey && event.key.toLocaleLowerCase() === "t" && taggableAlbums.length && !taggableHaveLossless && taggableTrackCount && !taggerOpen) {
        event.preventDefault();
        setTaggerAlbums(taggableAlbums);
      } else if (event.ctrlKey && !event.shiftKey && event.key.toLocaleLowerCase() === "r" && selectedAlbums.length && !selectedAlbumsHaveLossless && !taggerOpen && !renameBusy) {
        event.preventDefault();
        void renameSelectedAlbums(selectedAlbums);
      }
    }
    window.addEventListener("keydown", handleInboxShortcut);
    return () => window.removeEventListener("keydown", handleInboxShortcut);
  }, [renameBusy, renameSelectedAlbums, selectedAlbums, selectedAlbumsHaveLossless, taggableAlbums, taggableHaveLossless, taggableTrackCount, taggerOpen, trackTotalsBusy]);

  async function addFolder() {
    try {
      const folder = await selectInboxMonitorFolder();
      if (!folder) return;
      await addInboxMonitorFolder(folder);
      await refresh();
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    }
  }

  async function removeFolder(folder: string) {
    try {
      await removeInboxMonitorFolder(folder);
      if (selectedFolder === folder) selectFolder(null);
      await refresh();
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    }
  }

  async function repairAlbumCover() {
    if (!selectedAlbum || coverBusy) return;
    setCoverBusy(true);
    try {
      const imagePath = selectedAlbum.artworkPresent ? null : await selectInboxCoverImage(selectedAlbum.path);
      if (!selectedAlbum.artworkPresent && !imagePath) return;
      const result = await embedInboxAlbumCover(selectedAlbum.path, imagePath);
      notify(`Embedded the album cover in ${result.trackCount} ${result.trackCount === 1 ? "track" : "tracks"}${result.changedTracks < result.trackCount ? `; ${result.changedTracks} needed updating` : ""}.`);
      setMovePreview(null);
      await refresh();
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    } finally {
      setCoverBusy(false);
    }
  }

  async function convertSelectedAlbum() {
    if (!selectedAlbum || !selectedAlbum.losslessTrackCount || convertBusy) return;
    setConvertBusy(true);
    try {
      const result = await convertInboxLossless(selectedAlbum.path);
      if (result.convertedTracks) {
        notify(`${result.convertedTracks} ${result.convertedTracks === 1 ? "track" : "tracks"} converted to 320 kbps MP3; ${result.deletedSources} source ${result.deletedSources === 1 ? "file" : "files"} deleted.`);
      }
      if (result.failures.length) {
        const first = result.failures[0];
        notify(`${result.failures.length} ${result.failures.length === 1 ? "track" : "tracks"} could not be converted. ${first.fileName}: ${first.message}`, "error");
      }
      setMovePreview(null);
      await refresh();
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
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

  function openSelectedAlbumsIntake() {
    const targets = selectedAlbums.map((album): InboxLibraryIntakeTarget => ({
      sourcePath: album.path,
      label: `${album.artist ?? "Unknown artist"} — ${album.album ?? album.folderName}`,
      albumCount: 1,
      unreadyAlbumCount: album.readiness.ready ? 0 : 1,
      albumOnly: true,
    }));
    if (targets.length) setIntakeScope({ label: targets.length === 1 ? "selected album" : "selected albums", targets });
  }

  async function previewMove() {
    if (!selectedAlbum || !moveCategory) return;
    setMoveBusy(true);
    resetMoveProgress();
    try {
      setMovePreview(await libraryIntakeAdapter.preview({ sourcePath: selectedAlbum.path, category: moveCategory }));
      setReplacementConfirmed(false);
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    } finally {
      setMoveBusy(false);
    }
  }

  async function applyMove() {
    if (!movePreview || (movePreview.albums.some((album) => album.action === "replace") && !replacementConfirmed)) return;
    setMoveBusy(true);
    resetMoveProgress();
    try {
      const result = await libraryIntakeAdapter.apply({ planId: movePreview.planId, sessionId: movePreview.sessionId });
      setMoveRefreshing(true);
      await onCatalogChanged();
      setMovePreview(null);
      setMoveCategory("");
      notify(`${result.albumCount} ${result.albumCount === 1 ? "album" : "albums"} moved and cataloged.`);
      await refresh();
    } catch (nextError) {
      notify(nextError instanceof Error ? nextError.message : String(nextError), "error");
    } finally {
      setMoveRefreshing(false);
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
          <button type="button" disabled={trackTotalsDisabled} title="Fill missing track totals from all local MP3s in each selected album, counted separately per numbered disc. Existing totals are kept." onClick={() => void fillSelectedTrackTotals()}>{trackTotalsBusy ? <LoaderCircle className="is-spinning" /> : <Check />} {trackTotalsBusy ? "Filling track totals…" : `Fill track totals${selectedAlbums.length > 1 ? ` (${selectedAlbums.length})` : ""}`}</button>
          <button type="button" disabled={!selectedAlbums.length || renameBusy || convertBusy || coverBusy || moveBusy || trackTotalsBusy} title="Move only selected albums to the library. Ctrl-click to toggle albums; Shift-click to select a range." onClick={openSelectedAlbumsIntake}><FolderInput /> Move selected{selectedAlbums.length ? ` (${selectedAlbums.length})` : ""}</button>
          <button type="button" disabled={!selectedAlbums.length || selectedAlbumsHaveLossless || renameBusy || trackTotalsBusy} onClick={() => void renameSelectedAlbums(selectedAlbums)}>{renameBusy ? <LoaderCircle className="is-spinning" /> : <FilePenLine />} Rename{selectedAlbums.length > 1 ? ` ${selectedAlbums.length} albums` : ""} <kbd>Ctrl R</kbd></button>
          <button type="button" className="button button--primary" disabled={!taggableAlbums.length || taggableHaveLossless || !taggableTrackCount || renameBusy || trackTotalsBusy} title={taggableAlbums.length > 1 ? "Auto-tag the selected albums (for example CD1 and CD2) as one release." : undefined} onClick={() => setTaggerAlbums(taggableAlbums)}><Tags /> Auto-tag {taggableAlbums.length > 1 ? `${taggableAlbums.length} discs` : selectedAlbum && selectedTracks.length !== selectedAlbum.trackCount ? `${selectedTracks.length} tracks` : ""} <kbd>Ctrl T</kbd></button>
          <InboxMessageHistoryButton count={messages.length} open={historyOpen} onToggle={() => setHistoryOpen((current) => !current)} />
        </div>
      </header>

      {error ? <p className="inbox-banner" role="alert"><AlertTriangle />{error}</p> : null}
      <InboxMessages messages={messages} historyOpen={historyOpen} onCloseHistory={() => setHistoryOpen(false)} onDismiss={dismiss} />
      {snapshot.settings.warning ? <p className="inbox-banner" role="status"><AlertTriangle />{snapshot.settings.warning}</p> : null}

      <div className="inbox-workspace">
        <aside className="inbox-folders" aria-label="Monitored folders">
          <header><strong>Monitored folders</strong><button type="button" disabled={snapshot.settings.monitoredFolders.length >= 10} onClick={() => void addFolder()}><FolderPlus /> Add folder</button></header>
          <div className={`inbox-folder-row inbox-folder-row--all${!selectedFolder ? " is-selected" : ""}`}>
            <button type="button" onClick={() => selectFolder(null)}><InboxIcon /><span><strong>All folders</strong><small>{snapshot.albums.length} albums</small></span></button>
            <button type="button" className="inbox-folder-add" aria-label="Add All folders to library" title="Add All folders to library" disabled={!snapshot.albums.length || trackTotalsBusy} onClick={() => openAllFoldersIntake()}><FolderInput /></button>
          </div>
          {snapshot.settings.monitoredFolders.map((folder) => {
            const count = snapshot.albums.filter((album) => albumInFolder(album, folder)).length;
            return <div className={`inbox-folder-row${selectedFolder === folder ? " is-selected" : ""}`} key={folder}>
              <button type="button" onClick={() => selectFolder(folder)} title={folder}><Folder /><span><strong>{leafName(folder)}</strong><small>{folder}</small></span><output>{count}</output></button>
              <span className="inbox-folder-actions">
                <button type="button" className="inbox-folder-add" aria-label={`Add ${leafName(folder)} to library`} title="Add folder to library" disabled={!count || trackTotalsBusy} onClick={() => openFolderIntake(folder)}><FolderInput /></button>
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
          {selectedAlbum && inspectorView === "tags" && !selectedAlbumsHaveLossless ? <div className="inbox-manual-tags" inert={trackTotalsBusy}>
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
            <section><h3>Readiness</h3>{selectedAlbum.readiness.ready ? <p className="inbox-check"><CheckCircle2 /> Tags and embedded artwork are ready for intake.</p> : <ul>{selectedAlbum.readiness.issues.map((issue) => <li key={issue}><AlertTriangle />{issue}</li>)}</ul>}{selectedAlbumsMissingTotals.length > 0 ? <button type="button" className="inbox-autotag" disabled={trackTotalsDisabled} onClick={() => void fillSelectedTrackTotals()}>{trackTotalsBusy ? <LoaderCircle className="is-spinning" /> : <Check />}<span><strong>Fill missing track totals</strong><small>{selectedAlbums.length > 1 ? `Use each album's local tracks across ${selectedAlbums.length} selected albums` : "Use this album's local tracks"} · per numbered disc</small></span></button> : null}</section>
            {selectedAlbumHasLossless ? <button type="button" className="inbox-autotag inbox-convert" disabled={convertBusy || trackTotalsBusy} onClick={() => void convertSelectedAlbum()}>{convertBusy ? <LoaderCircle className="is-spinning" /> : <AudioLines />}<span><strong>Convert to 320 kbps MP3</strong><small>Convert {selectedAlbum.losslessTrackCount} FLAC/APE {selectedAlbum.losslessTrackCount === 1 ? "track" : "tracks"} here, verify each MP3, then delete each source</small></span></button> : null}
            {!selectedAlbumHasLossless && !selectedAlbum.artworkReady ? <button type="button" className="inbox-autotag inbox-artwork" disabled={coverBusy || trackTotalsBusy} onClick={() => void repairAlbumCover()}>{coverBusy ? <LoaderCircle className="is-spinning" /> : <ImagePlus />}<span><strong>{selectedAlbum.artworkPresent ? "Embed cover in all tracks" : "Choose album cover"}</strong><small>{selectedAlbum.artworkPresent ? `Use the displayed cover for all ${selectedAlbum.trackCount} MP3s` : "Select a JPG, PNG, GIF, BMP, or WebP image"}</small></span></button> : null}
            {!selectedAlbumHasLossless ? <>
            <section className="inbox-track-selection"><header><h3>Tracks to tag</h3><span><button type="button" onClick={() => setExcludedTrackPaths(new Set())}>All</button><button type="button" onClick={() => setExcludedTrackPaths(new Set(selectedAlbum.tracks.map((track) => track.path)))}>None</button></span></header><div>{selectedAlbum.tracks.map((track, index) => <label key={track.path}><input type="checkbox" checked={!excludedTrackPaths.has(track.path)} onChange={() => setExcludedTrackPaths((current) => { const next = new Set(current); if (next.has(track.path)) next.delete(track.path); else next.add(track.path); return next; })} /><span>{track.discNumber ? `${track.discNumber}-` : ""}{String(track.trackNumber ?? index + 1).padStart(2, "0")}</span><strong>{track.title ?? track.fileName}</strong></label>)}</div><small>{selectedTracks.length} of {selectedAlbum.trackCount} selected</small></section>
            <button type="button" className="inbox-autotag" disabled={!taggableTrackCount || trackTotalsBusy} onClick={() => setTaggerAlbums(taggableAlbums)}><Tags /><span><strong>Album Auto-Tagger</strong><small>{taggableAlbums.length > 1 ? `Match ${taggableAlbums.length} selected discs as one release` : selectedTracks.length === selectedAlbum.trackCount ? "Match the full album" : `Match ${selectedTracks.length} selected tracks`}</small></span><kbd>Ctrl T</kbd></button>
            <button type="button" className="inbox-autotag inbox-rename" disabled={!selectedAlbums.length || selectedAlbumsHaveLossless || renameBusy || trackTotalsBusy} onClick={() => void renameSelectedAlbums(selectedAlbums)}><FilePenLine /><span><strong>Rename from tags</strong><small>{selectedAlbums.length > 1 ? `Standardize ${selectedAlbums.length} selected album folders and track filenames` : "Standardize the album folder and track filenames"}</small></span><kbd>Ctrl R</kbd></button>
            </> : null}
            <section className="inbox-move"><h3>Move to library</h3><p>Uses the same reviewed, preview-first flow as Add Music.</p><select aria-label="Library destination" value={moveCategory} onChange={(event) => { setMoveCategory(event.target.value as LibraryIntakeCategoryId | ""); setMovePreview(null); setReplacementConfirmed(false); }}><option value="">Select destination…</option>{libraryIntakeCategories.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}</select>
              {movePreview ? <div className="inbox-move__preview"><Check /><span><strong>{movePreview.trackCount} tracks verified</strong><small>{movePreview.category.destinationRoot}</small></span></div> : null}
              {moveBusy ? <LibraryIntakeActivity mode={movePreview ? "apply" : "preview"} progress={moveProgress} refreshing={moveRefreshing} /> : null}
              {movePreview?.albums.some((album) => album.action === "replace") ? <label className="inbox-move__replacement"><AlertTriangle /><span><strong>Replace existing release</strong><small>{movePreview.albums[0].existingTrackCount} existing → {movePreview.albums[0].trackCount} new tracks · old release preserved for recovery</small></span><input type="checkbox" aria-label="Confirm replacement" checked={replacementConfirmed} onChange={(event) => setReplacementConfirmed(event.target.checked)} /></label> : null}
              <button type="button" className="button button--primary" disabled={!moveCategory || moveBusy || trackTotalsBusy || !selectedAlbum.readiness.ready || Boolean(movePreview?.albums.some((album) => album.action === "replace") && !replacementConfirmed)} onClick={() => void (movePreview ? applyMove() : previewMove())}>{moveBusy ? <LoaderCircle className="is-spinning" /> : <ArrowRight />}{movePreview?.albums.some((album) => album.action === "replace") ? "Replace and catalog" : movePreview ? "Move and catalog" : "Preview move"}</button>
            </section>
          </> : <div className="inbox-inspector__empty"><InboxIcon /><span>Select an album to review it.</span></div>}
        </aside>
      </div>

      {taggerOpen ? <AlbumAutoTagger albums={taggerAlbums} tracks={taggerTracks} discogsConfigured={snapshot.settings.discogsConfigured} onOpenSettings={onOpenMetadataSettings} onClose={() => setTaggerAlbums([])} onApplied={async (message, path) => { notify(message); const focusId = taggerAlbums[0].id; setTaggerAlbums([]); await refresh(false, { id: focusId, path }); }} /> : null}
      {intakeScope ? <InboxLibraryIntakeDialog
        scopeLabel={intakeScope.label}
        targets={intakeScope.targets}
        onClose={() => setIntakeScope(null)}
        onApplied={async () => { await onCatalogChanged(); await refresh(); }}
        onCompleted={notify}
      /> : null}
    </section>
  );
}

function discOrderKey(album: InboxAlbum): number {
  const discs = album.tracks.map((track) => track.discNumber).filter((value): value is number => value !== null);
  return discs.length ? Math.min(...discs) : Number.POSITIVE_INFINITY;
}

/** Orders multi-disc folders (CD1, CD2, …) by their disc tag, falling back to a natural folder-name sort. */
function orderDiscAlbums(albums: readonly InboxAlbum[]): InboxAlbum[] {
  return [...albums].sort((left, right) => {
    const leftKey = discOrderKey(left);
    const rightKey = discOrderKey(right);
    if (leftKey !== rightKey) return leftKey < rightKey ? -1 : 1;
    return left.folderName.localeCompare(right.folderName, undefined, { numeric: true, sensitivity: "base" });
  });
}
