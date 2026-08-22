import { Ban, Heart, RefreshCw, RotateCcw, Save, ShieldCheck } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Track } from "../library";
import {
  readTrackTagState,
  tagValuesForTrack,
  undoTrackTagEdit,
  updateTrackTags,
  type LoveState,
  type TagValues,
  type TrackTagSnapshot,
} from "../tags";

interface TagEditorProps {
  track: Track;
  onTrackChange: (track: Track) => void;
}

type EditorPhase = "loading" | "ready" | "saving" | "saved" | "error";

const ratingOptions = Array.from({ length: 10 }, (_, index) => (index + 1) / 2);

export function TagEditor({ track, onTrackChange }: TagEditorProps) {
  const [confirmed, setConfirmed] = useState<TagValues>(() => tagValuesForTrack(track));
  const [draft, setDraft] = useState<TagValues>(() => tagValuesForTrack(track));
  const [yearText, setYearText] = useState(track.releaseYear?.toString() ?? "");
  const [phase, setPhase] = useState<EditorPhase>("loading");
  const [message, setMessage] = useState<string | null>(null);
  const [canUndo, setCanUndo] = useState(track.canUndoTagEdit);
  const [syncState, setSyncState] = useState(track.tagSyncState);
  const requestRef = useRef(0);
  const dirtyRef = useRef(false);
  const workingRef = useRef(true);

  function applySnapshot(snapshot: TrackTagSnapshot, nextPhase: EditorPhase) {
    setConfirmed(snapshot.tagState.values);
    setDraft(snapshot.tagState.values);
    setYearText(snapshot.tagState.values.releaseYear?.toString() ?? "");
    setCanUndo(snapshot.tagState.canUndo);
    setSyncState(snapshot.tagState.syncState);
    setPhase(nextPhase);
    onTrackChange(snapshot.track);
  }

  // The parent keys this editor by track ID, so each file read belongs to one component lifetime.
  useEffect(() => {
    const requestId = ++requestRef.current;
    void readTrackTagState(track)
      .then((snapshot) => {
        if (requestId !== requestRef.current) return;
        applySnapshot(snapshot, "ready");
      })
      .catch((error: unknown) => {
        if (requestId !== requestRef.current) return;
        setPhase("error");
        setMessage(error instanceof Error ? error.message : String(error));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const parsedYear = useMemo(() => {
    if (!yearText.trim()) return null;
    const year = Number(yearText);
    return Number.isInteger(year) && year >= 1000 && year <= 2999 ? year : Number.NaN;
  }, [yearText]);
  const desired = useMemo<TagValues>(
    () => ({ ...draft, releaseYear: Number.isNaN(parsedYear) ? null : parsedYear }),
    [draft, parsedYear],
  );
  const isDirty = JSON.stringify(desired) !== JSON.stringify(confirmed) || Number.isNaN(parsedYear);
  const isWorking = phase === "loading" || phase === "saving";

  useEffect(() => {
    dirtyRef.current = isDirty;
    workingRef.current = isWorking;
  }, [isDirty, isWorking]);

  useEffect(() => {
    function refreshExternalTags() {
      if (dirtyRef.current || workingRef.current) return;
      const requestId = ++requestRef.current;
      void readTrackTagState(track)
        .then((snapshot) => {
          if (requestId !== requestRef.current) return;
          applySnapshot(snapshot, "ready");
        })
        .catch((error: unknown) => {
          if (requestId !== requestRef.current) return;
          setPhase("error");
          setMessage(error instanceof Error ? error.message : String(error));
        });
    }
    window.addEventListener("focus", refreshExternalTags);
    return () => window.removeEventListener("focus", refreshExternalTags);
    // This editor is keyed by track identity; a new track gets a new component lifetime.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function save() {
    if (Number.isNaN(parsedYear)) {
      setPhase("error");
      setMessage("Release Year must be blank or a four-digit year from 1000 to 2999.");
      return;
    }
    setPhase("saving");
    setMessage(null);
    try {
      const snapshot = await updateTrackTags(track, confirmed, desired);
      applySnapshot(snapshot, "saved");
      setMessage("Verified in the MP3. Music Library will catch up after your next MusicBee TSV import.");
    } catch (error) {
      setPhase("error");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function undo() {
    setPhase("saving");
    setMessage(null);
    try {
      const snapshot = await undoTrackTagEdit(track);
      applySnapshot(snapshot, "saved");
      setMessage("The retained MP3 backup was restored and verified.");
    } catch (error) {
      setPhase("error");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  function chooseLove(loveState: LoveState) {
    setDraft((value) => ({ ...value, loveState }));
    setPhase("ready");
    setMessage(null);
  }

  if (phase === "loading") {
    return (
      <section className="tag-editor tag-editor--loading" aria-live="polite">
        <RefreshCw className="is-spinning" aria-hidden="true" />
        <span>Reading MusicBee tags from the MP3…</span>
      </section>
    );
  }

  return (
    <section className="tag-editor" aria-labelledby="tag-editor-title">
      <div className="tag-editor__heading">
        <div><p className="eyebrow">File metadata</p><h3 id="tag-editor-title">MusicBee tags</h3></div>
        {syncState === "pendingImport" && <span className="sync-badge">Pending TSV import</span>}
      </div>

      <label className="tag-field">
        <span>Rating</span>
        <select
          value={draft.rating ?? ""}
          onChange={(event) => {
            setDraft((value) => ({ ...value, rating: event.target.value ? Number(event.target.value) : null }));
            setPhase("ready");
            setMessage(null);
          }}
          disabled={isWorking}
        >
          <option value="">Unrated</option>
          {ratingOptions.map((rating) => <option value={rating} key={rating}>{rating.toFixed(1)} stars</option>)}
        </select>
      </label>

      <fieldset className="tag-field tag-love" disabled={isWorking}>
        <legend>Love rating</legend>
        <div>
          <button type="button" className={draft.loveState === "neutral" ? "is-active" : undefined} aria-pressed={draft.loveState === "neutral"} onClick={() => chooseLove("neutral")}>Neutral</button>
          <button type="button" className={draft.loveState === "loved" ? "is-active is-loved" : undefined} aria-pressed={draft.loveState === "loved"} onClick={() => chooseLove("loved")}><Heart aria-hidden="true" /> Love</button>
          <button type="button" className={draft.loveState === "banned" ? "is-active is-banned" : undefined} aria-pressed={draft.loveState === "banned"} onClick={() => chooseLove("banned")}><Ban aria-hidden="true" /> Ban</button>
        </div>
      </fieldset>

      <label className="tag-field">
        <span>Release Year</span>
        <input
          type="number"
          min="1000"
          max="2999"
          inputMode="numeric"
          placeholder="Unknown"
          value={yearText}
          onChange={(event) => { setYearText(event.target.value); setPhase("ready"); setMessage(null); }}
          disabled={isWorking}
        />
      </label>

      {message && <p className={`tag-message tag-message--${phase}`} role={phase === "error" ? "alert" : "status"}>{message}</p>}

      <div className="tag-editor__actions">
        <button type="button" className="tag-undo" onClick={() => void undo()} disabled={isWorking || !canUndo}><RotateCcw aria-hidden="true" /> Undo last write</button>
        <button type="button" className="tag-save" onClick={() => void save()} disabled={isWorking || !isDirty || Number.isNaN(parsedYear)}>
          {phase === "saving" ? <RefreshCw className="is-spinning" aria-hidden="true" /> : phase === "saved" ? <ShieldCheck aria-hidden="true" /> : <Save aria-hidden="true" />}
          {phase === "saving" ? "Verifying…" : "Save to MP3"}
        </button>
      </div>
    </section>
  );
}
