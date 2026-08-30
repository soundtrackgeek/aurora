import { ImagePlus, Music2, RefreshCw, RotateCcw, Save, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeEvent } from "react";
import { albumCoverUrl, type Track } from "../library";
import { selectAlbumCoverImage, type SelectedArtwork } from "../artworkSelection";
import { loadGenreNames } from "../genres";
import {
  aggregateEditableTagValues,
  EDITABLE_TAG_FIELDS,
  readTagEditorState,
  updateTagEditor,
  type CatalogSync,
  type EditableTagAggregation,
  type EditableTagField,
  type EditableTagValues,
  type TagEditorSnapshot,
  type TagEditorTarget,
} from "../tags";

interface TagEditorProps {
  target: TagEditorTarget;
  onTracksChange: (tracks: Track[], catalogSync?: CatalogSync) => void | boolean;
  onCatalogSync?: (sync: CatalogSync) => void | Promise<void>;
}

export interface ManualTagEditorSaveResult {
  state: TagEditorSnapshot;
  message: string;
}

export interface AlbumArtworkEditor {
  currentUrl: string | null;
  trackCount?: number;
  choose: () => Promise<SelectedArtwork | null>;
}

interface ManualTagEditorProps {
  kind: "track" | "album";
  label: string;
  loadSnapshot: () => Promise<TagEditorSnapshot>;
  saveSnapshot: (
    expected: TagEditorSnapshot,
    fields: EditableTagField[],
    values: EditableTagValues,
    artworkToken: string | null,
  ) => Promise<ManualTagEditorSaveResult>;
  artwork?: AlbumArtworkEditor;
}

type EditorPhase = "loading" | "ready" | "saving" | "saved" | "error";
type DraftText = Record<EditableTagField, string>;
type FieldKind = "text" | "rating" | "year" | "position";

interface FieldDefinition {
  field: EditableTagField;
  label: string;
  kind: FieldKind;
}

const primaryFields: FieldDefinition[] = [
  { field: "albumArtist", label: "Album artist", kind: "text" },
  { field: "artist", label: "Artist", kind: "text" },
  { field: "album", label: "Album", kind: "text" },
  { field: "title", label: "Track title", kind: "text" },
  { field: "genre", label: "Genre", kind: "text" },
  { field: "publisher", label: "Publisher", kind: "text" },
  { field: "rating", label: "Track rating", kind: "rating" },
  { field: "year", label: "Year", kind: "year" },
  { field: "releaseYear", label: "Release year", kind: "year" },
];

const positionFields: FieldDefinition[] = [
  { field: "trackNumber", label: "Track", kind: "position" },
  { field: "trackTotal", label: "Track total", kind: "position" },
  { field: "discNumber", label: "Disc", kind: "position" },
  { field: "discTotal", label: "Disc total", kind: "position" },
];

const ratingOptions = Array.from({ length: 10 }, (_, index) => (index + 1) / 2);
const requiredTagFields = new Set<EditableTagField>(["albumArtist", "album", "title"]);

function emptyDraft(): DraftText {
  return Object.fromEntries(EDITABLE_TAG_FIELDS.map((field) => [field, ""])) as DraftText;
}

function draftForSnapshot(snapshot: TagEditorSnapshot): DraftText {
  const aggregate = aggregateEditableTagValues(snapshot.tracks);
  return Object.fromEntries(EDITABLE_TAG_FIELDS.map((field) => [
    field,
    aggregate[field].mixed || aggregate[field].value === null ? "" : String(aggregate[field].value),
  ])) as DraftText;
}

function nullableDraftText(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function nullableDraftNumber(value: string): number | null {
  return value.trim() ? Number(value) : null;
}

function valuesForDraft(draft: DraftText): EditableTagValues {
  return {
    albumArtist: nullableDraftText(draft.albumArtist),
    artist: nullableDraftText(draft.artist),
    album: nullableDraftText(draft.album),
    title: nullableDraftText(draft.title),
    genre: nullableDraftText(draft.genre),
    publisher: nullableDraftText(draft.publisher),
    rating: nullableDraftNumber(draft.rating),
    year: nullableDraftNumber(draft.year),
    releaseYear: nullableDraftNumber(draft.releaseYear),
    trackNumber: nullableDraftNumber(draft.trackNumber),
    trackTotal: nullableDraftNumber(draft.trackTotal),
    discNumber: nullableDraftNumber(draft.discNumber),
    discTotal: nullableDraftNumber(draft.discTotal),
  };
}

function validateField(field: EditableTagField, text: string): string | null {
  if (!text.trim()) {
    return requiredTagFields.has(field)
      ? "Music Library requires this field; enter a value or leave it unchecked."
      : null;
  }
  if (!["rating", "year", "releaseYear", "trackNumber", "trackTotal", "discNumber", "discTotal"].includes(field)) {
    return null;
  }
  const value = Number(text);
  if (!Number.isFinite(value)) return "Enter a number or leave the field blank to clear it.";
  if (field === "rating" && (!Number.isInteger(value * 2) || value < 0.5 || value > 5)) {
    return "Rating must be between 0.5 and 5 in half-star steps.";
  }
  if ((field === "year" || field === "releaseYear") && (!Number.isInteger(value) || value < 1000 || value > 2999)) {
    return "Years must be four digits from 1000 to 2999.";
  }
  if (["trackNumber", "trackTotal", "discNumber", "discTotal"].includes(field)
    && (!Number.isInteger(value) || value < 1 || value > 9999)) {
    return "Track and disc values must be whole numbers from 1 to 9999.";
  }
  return null;
}

interface TagFieldProps {
  definition: FieldDefinition;
  aggregate: EditableTagAggregation[EditableTagField];
  checked: boolean;
  value: string;
  disabled: boolean;
  error: string | null;
  suggestionListId?: string;
  onCheck: (field: EditableTagField, checked: boolean) => void;
  onChange: (field: EditableTagField, value: string) => void;
}

function TagField({ definition, aggregate, checked, value, disabled, error, suggestionListId, onCheck, onChange }: TagFieldProps) {
  const { field, label, kind } = definition;
  const placeholder = aggregate.mixed && !checked
    ? "Mixed"
    : checked && requiredTagFields.has(field)
      ? "Required"
      : checked
        ? "Clear on save"
        : "Blank";
  const controlProps = {
    id: `tag-editor-${field}`,
    "aria-label": label,
    "aria-invalid": error ? true : undefined,
    disabled,
    value,
    onChange: (event: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => onChange(field, event.target.value),
  };

  return (
    <div className={`tag-field${checked ? " is-selected" : ""}`}>
      <div className="tag-field__toggle">
        <input
          id={`tag-editor-write-${field}`}
          type="checkbox"
          aria-label={`Write ${label}`}
          checked={checked}
          disabled={disabled}
          onChange={(event) => onCheck(field, event.target.checked)}
        />
        <span aria-hidden="true">{label}</span>
        {aggregate.mixed && !checked ? <small>Mixed</small> : null}
      </div>
      {kind === "rating" ? (
        <select {...controlProps}>
          <option value="">{aggregate.mixed && !checked ? "Mixed" : "Unrated"}</option>
          {ratingOptions.map((rating) => <option value={rating} key={rating}>{rating.toFixed(1)} stars</option>)}
        </select>
      ) : (
        <input
          {...controlProps}
          list={field === "genre" ? suggestionListId : undefined}
          type={kind === "text" ? "text" : "number"}
          inputMode={kind === "text" ? undefined : "numeric"}
          min={kind === "year" ? 1000 : kind === "position" ? 1 : undefined}
          max={kind === "year" ? 2999 : kind === "position" ? 9999 : undefined}
          step={kind === "text" ? undefined : 1}
          placeholder={placeholder}
        />
      )}
      {error ? <small className="tag-field__error">{error}</small> : null}
    </div>
  );
}

function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function ArtworkField({
  source,
  fileName,
  trackCount,
  disabled,
  pending,
  onChoose,
}: {
  source: string | null;
  fileName: string | null;
  trackCount: number;
  disabled: boolean;
  pending: boolean;
  onChoose: () => void;
}) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const showImage = source && source !== failedSource;
  return (
    <button
      type="button"
      className={`tag-editor__artwork${pending ? " is-pending" : ""}`}
      aria-label={pending ? "Choose a different replacement album cover" : "Choose replacement album cover"}
      disabled={disabled}
      onClick={onChoose}
    >
      <span className="tag-editor__artwork-image">
        {showImage
          ? <img src={source} alt="" onError={() => setFailedSource(source)} />
          : <Music2 aria-hidden="true" />}
        <span><ImagePlus aria-hidden="true" />{pending ? "Change selection" : "Replace cover"}</span>
      </span>
      <strong>{pending ? fileName : "Album artwork"}</strong>
      <small>{pending ? `Pending · saves to all ${trackCount} album ${trackCount === 1 ? "MP3" : "MP3s"}` : "Click to choose an image from the album folder"}</small>
    </button>
  );
}

export function ManualTagEditor({ kind, label, loadSnapshot, saveSnapshot, artwork }: ManualTagEditorProps) {
  const [snapshot, setSnapshot] = useState<TagEditorSnapshot | null>(null);
  const [draft, setDraft] = useState<DraftText>(emptyDraft);
  const [selectedFields, setSelectedFields] = useState<Set<EditableTagField>>(() => new Set());
  const [phase, setPhase] = useState<EditorPhase>("loading");
  const [message, setMessage] = useState<string | null>(null);
  const [genreSuggestions, setGenreSuggestions] = useState<string[]>([]);
  const [artworkDraft, setArtworkDraft] = useState<SelectedArtwork | null>(null);
  const [savedArtworkUrl, setSavedArtworkUrl] = useState<string | null>(null);
  const requestRef = useRef(0);
  const dirtyRef = useRef(false);
  const workingRef = useRef(true);
  useEffect(() => {
    let cancelled = false;
    void loadGenreNames()
      .then((names) => { if (!cancelled) setGenreSuggestions(names); })
      .catch((error: unknown) => console.warn("Aurora could not load genre suggestions", error));
    return () => { cancelled = true; };
  }, []);
  const acceptSnapshot = useCallback((next: TagEditorSnapshot, nextPhase: EditorPhase) => {
    setSnapshot(next);
    setDraft(draftForSnapshot(next));
    setSelectedFields(new Set());
    setPhase(nextPhase);
  }, []);

  const loadState = useCallback(async (showLoading: boolean) => {
    const requestId = ++requestRef.current;
    await Promise.resolve();
    if (requestId !== requestRef.current) return;
    if (showLoading) setPhase("loading");
    setMessage(null);
    try {
      const next = await loadSnapshot();
      if (requestId !== requestRef.current) return;
      if (!showLoading && dirtyRef.current) return;
      acceptSnapshot(next, "ready");
    } catch (error) {
      if (requestId !== requestRef.current) return;
      if (!showLoading && dirtyRef.current) return;
      setPhase("error");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [acceptSnapshot, loadSnapshot]);

  useEffect(() => {
    const requestId = ++requestRef.current;
    void loadSnapshot()
      .then((next) => {
        if (requestId !== requestRef.current) return;
        acceptSnapshot(next, "ready");
      })
      .catch((error: unknown) => {
        if (requestId !== requestRef.current) return;
        setPhase("error");
        setMessage(error instanceof Error ? error.message : String(error));
      });
    return () => { requestRef.current += 1; };
  }, [acceptSnapshot, loadSnapshot]);

  const isWorking = phase === "loading" || phase === "saving";
  const isDirty = selectedFields.size > 0 || artworkDraft !== null;

  useEffect(() => {
    dirtyRef.current = isDirty;
    workingRef.current = isWorking;
  }, [isDirty, isWorking]);

  useEffect(() => {
    function refreshExternalTags() {
      if (dirtyRef.current || workingRef.current) return;
      void loadState(false);
    }
    window.addEventListener("focus", refreshExternalTags);
    return () => window.removeEventListener("focus", refreshExternalTags);
  }, [loadState]);

  const aggregate = useMemo(
    () => aggregateEditableTagValues(snapshot?.tracks ?? []),
    [snapshot],
  );
  const selectedInOrder = useMemo(
    () => EDITABLE_TAG_FIELDS.filter((field) => selectedFields.has(field)),
    [selectedFields],
  );
  const validation = useMemo(() => Object.fromEntries(EDITABLE_TAG_FIELDS.map((field) => [
    field,
    selectedFields.has(field) ? validateField(field, draft[field]) : null,
  ])) as Record<EditableTagField, string | null>, [draft, selectedFields]);
  const validationMessage = selectedInOrder.map((field) => validation[field]).find(Boolean) ?? null;
  const trackCount = snapshot?.tracks.length ?? 0;
  const artworkTrackCount = artwork?.trackCount ?? trackCount;

  function checkField(field: EditableTagField, checked: boolean) {
    setSelectedFields((current) => {
      const next = new Set(current);
      if (checked) next.add(field);
      else next.delete(field);
      return next;
    });
    setPhase("ready");
    setMessage(null);
  }

  function editField(field: EditableTagField, value: string) {
    setDraft((current) => ({ ...current, [field]: value }));
    setSelectedFields((current) => new Set(current).add(field));
    setPhase("ready");
    setMessage(null);
  }

  function resetDraft() {
    if (!snapshot) return;
    setDraft(draftForSnapshot(snapshot));
    setSelectedFields(new Set());
    setArtworkDraft(null);
    setPhase("ready");
    setMessage(null);
  }

  async function chooseArtwork() {
    if (!artwork || isWorking) return;
    setMessage(null);
    try {
      const selected = await artwork.choose();
      if (!selected) return;
      setArtworkDraft(selected);
      setPhase("ready");
    } catch (error) {
      setPhase("error");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function save() {
    if (!snapshot || (!selectedInOrder.length && !artworkDraft) || validationMessage) return;
    const savingCount = snapshot.tracks.length;
    const fieldCount = selectedInOrder.length;
    const savingArtwork = artworkDraft;
    setPhase("saving");
    setMessage(null);
    try {
      const result = await saveSnapshot(
        snapshot,
        selectedInOrder,
        valuesForDraft(draft),
        savingArtwork?.token ?? null,
      );
      acceptSnapshot(result.state, "saved");
      if (savingArtwork) {
        setSavedArtworkUrl(savingArtwork.previewUrl);
        setArtworkDraft(null);
      }
      const changeCount = fieldCount + (savingArtwork ? 1 : 0);
      setMessage(result.message || `Saved ${countLabel(changeCount, "change")} directly to ${countLabel(Math.max(savingCount, artworkTrackCount), "MP3", "MP3s")}.`);
    } catch (error) {
      setPhase("error");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  if (!snapshot && phase === "loading") {
    return (
      <section className="tag-editor tag-editor--loading" aria-live="polite">
        <RefreshCw className="is-spinning" aria-hidden="true" />
        <span>Reading tags from the MP3 {kind === "album" ? "files" : "file"}…</span>
      </section>
    );
  }

  if (!snapshot) {
    return (
      <section className="tag-editor" aria-labelledby="tag-editor-heading">
        <div className="tag-editor__heading">
          <div><p className="eyebrow">Tag editor</p><h3 id="tag-editor-heading">Could not read tags</h3></div>
        </div>
        {message ? <p className="tag-message tag-message--error" role="alert">{message}</p> : null}
        <button type="button" className="tag-editor__retry" onClick={() => void loadState(true)}>
          <RefreshCw aria-hidden="true" /> Retry
        </button>
      </section>
    );
  }

  return (
    <section className="tag-editor" aria-labelledby="tag-editor-heading" aria-busy={isWorking}>
      <div className="tag-editor__heading">
        <div>
          <p className="eyebrow">{kind === "album" ? "Album selection" : "Track selection"}</p>
          <h3 id="tag-editor-heading">{label}</h3>
          <span>{countLabel(trackCount, "MP3", "MP3s")}</span>
        </div>
        <button
          type="button"
          className="tag-editor__refresh"
          aria-label="Refresh tags from MP3 files"
          title={isDirty ? "Reset the draft before refreshing" : "Refresh tags from MP3 files"}
          disabled={isWorking || isDirty}
          onClick={() => void loadState(false)}
        >
          <RefreshCw className={phase === "loading" ? "is-spinning" : undefined} aria-hidden="true" />
        </button>
      </div>

      <div className="tag-editor__fields">
        {artwork ? <ArtworkField
          source={artworkDraft?.previewUrl ?? savedArtworkUrl ?? artwork.currentUrl}
          fileName={artworkDraft?.fileName ?? null}
          trackCount={artworkTrackCount}
          disabled={isWorking}
          pending={artworkDraft !== null}
          onChoose={() => void chooseArtwork()}
        /> : null}
        <datalist id="tag-editor-genre-suggestions">
          {genreSuggestions.map((genre) => <option value={genre} key={genre} />)}
        </datalist>
        {primaryFields.map((definition) => (
          <TagField
            key={definition.field}
            definition={definition}
            aggregate={aggregate[definition.field]}
            checked={selectedFields.has(definition.field)}
            value={draft[definition.field]}
            disabled={isWorking}
            error={validation[definition.field]}
            suggestionListId="tag-editor-genre-suggestions"
            onCheck={checkField}
            onChange={editField}
          />
        ))}
        <div className="tag-editor__position-fields">
          {positionFields.map((definition) => (
            <TagField
              key={definition.field}
              definition={definition}
              aggregate={aggregate[definition.field]}
              checked={selectedFields.has(definition.field)}
              value={draft[definition.field]}
              disabled={isWorking}
              error={validation[definition.field]}
              onCheck={checkField}
              onChange={editField}
            />
          ))}
        </div>
      </div>

      {validationMessage ? <p className="tag-message tag-message--error" role="alert">{validationMessage}</p> : null}
      {message ? <p className={`tag-message tag-message--${phase}`} role={phase === "error" ? "alert" : "status"}>{message}</p> : null}

      <div className="tag-editor__actions">
        <button type="button" className="tag-reset" onClick={resetDraft} disabled={isWorking || !isDirty}>
          <RotateCcw aria-hidden="true" /> Reset draft
        </button>
        <button
          type="button"
          className="tag-save"
          onClick={() => void save()}
          disabled={isWorking || !isDirty || Boolean(validationMessage)}
        >
          {phase === "saving" ? <RefreshCw className="is-spinning" aria-hidden="true" /> : phase === "saved" ? <ShieldCheck aria-hidden="true" /> : <Save aria-hidden="true" />}
          {phase === "saving"
            ? `Saving ${countLabel(Math.max(trackCount, artworkDraft ? artworkTrackCount : 0), "MP3", "MP3s")}…`
            : artworkDraft
              ? `Save ${countLabel(selectedFields.size + 1, "change")} to ${countLabel(Math.max(trackCount, artworkTrackCount), "MP3", "MP3s")}`
              : `Save ${countLabel(selectedFields.size, "field")} to ${countLabel(trackCount, "MP3", "MP3s")}`}
        </button>
      </div>
    </section>
  );
}

export function TagEditor({ target, onTracksChange, onCatalogSync }: TagEditorProps) {
  const targetKind = target.kind;
  const targetAlbumId = target.kind === "album" ? target.albumId : null;
  const targetTrackId = target.kind === "track" ? target.trackId : null;
  const targetTrackKey = target.kind === "track" ? target.trackKey : null;
  const targetTrackSelectionJson = target.kind === "tracks" ? JSON.stringify(target.tracks) : "[]";
  const targetAlbumSelectionJson = target.kind === "albums" ? JSON.stringify(target.albumIds) : "[]";
  const targetLabel = target.label;
  const requestTarget = useMemo<TagEditorTarget>(() => {
    if (targetKind === "album") return { kind: "album", albumId: targetAlbumId!, label: targetLabel };
    if (targetKind === "track") return { kind: "track", trackId: targetTrackId!, trackKey: targetTrackKey!, label: targetLabel };
    if (targetKind === "albums") return { kind: "albums", albumIds: JSON.parse(targetAlbumSelectionJson) as string[], label: targetLabel };
    return { kind: "tracks", tracks: JSON.parse(targetTrackSelectionJson) as Array<{ trackId: string; trackKey: string }>, label: targetLabel };
  }, [
    targetAlbumId,
    targetAlbumSelectionJson,
    targetKind,
    targetLabel,
    targetTrackId,
    targetTrackKey,
    targetTrackSelectionJson,
  ]);
  const loadSnapshot = useCallback(() => readTagEditorState(requestTarget), [requestTarget]);
  const artwork = useMemo<AlbumArtworkEditor | undefined>(() => targetKind === "album" ? {
    currentUrl: albumCoverUrl(targetAlbumId!, 256),
    choose: () => selectAlbumCoverImage({ source: "library", target: requestTarget }),
  } : undefined, [requestTarget, targetAlbumId, targetKind]);
  const saveSnapshot = useCallback(async (
    expected: TagEditorSnapshot,
    fields: EditableTagField[],
    values: EditableTagValues,
    artworkToken: string | null,
  ): Promise<ManualTagEditorSaveResult> => {
    const result = await updateTagEditor(requestTarget, expected, fields, values, artworkToken);
    const projectionAccepted = result.catalogSync
      ? onTracksChange(result.tracks, result.catalogSync)
      : onTracksChange(result.tracks);
    if (projectionAccepted === false) {
      if (result.catalogSync && onCatalogSync) await onCatalogSync(result.catalogSync);
      return {
        state: await readTagEditorState(requestTarget),
        message: result.catalogSync?.message ?? "Tags were saved; the latest Music Library state is shown.",
      };
    }
    const savedFiles = artworkToken
      ? fields.length
        ? `Saved ${countLabel(fields.length, "field")} and embedded the replacement cover in ${countLabel(expected.tracks.length, "MP3", "MP3s")}.`
        : `Embedded the replacement cover in ${countLabel(expected.tracks.length, "MP3", "MP3s")}.`
      : `Saved ${countLabel(fields.length, "field")} directly to ${countLabel(expected.tracks.length, "MP3", "MP3s")}.`;
    let message = savedFiles;
    if (result.catalogSync?.status === "synced") {
      const remaining = result.catalogSync.pendingFolderCount > 0
        ? ` ${countLabel(result.catalogSync.pendingFolderCount, "other folder")} still pending; Aurora is retrying automatically.`
        : "";
      message = `${savedFiles} Music Library updated.${remaining}`;
    } else if (result.catalogSync?.status === "pending") {
      message = `${savedFiles} ${result.catalogSync.message ?? "The MP3 write is verified; catalog sync is pending."}`;
    } else if (result.catalogSync?.status === "blocked") {
      message = `${savedFiles} ${result.catalogSync.message ?? "Music Library update needs attention; automatic retries are paused."}`;
    }
    if (result.catalogSync && onCatalogSync) {
      try {
        await onCatalogSync(result.catalogSync);
      } catch (error) {
        console.warn("Music Library updated, but Aurora could not refresh its catalog views yet", error);
      }
    }
    return { state: result.state, message };
  }, [onCatalogSync, onTracksChange, requestTarget]);

  return <ManualTagEditor
    key={JSON.stringify(requestTarget)}
    kind={target.kind === "album" || target.kind === "albums" ? "album" : "track"}
    label={target.label}
    loadSnapshot={loadSnapshot}
    saveSnapshot={saveSnapshot}
    artwork={artwork}
  />;
}
