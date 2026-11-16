import { useEffect, useRef, useState } from "react";
import { AudioLines, Check, Sparkles, TriangleAlert, X } from "lucide-react";
import { formatCount } from "../library";
import {
  applyGenreProposal,
  dismissGenreProposal,
  genreTagHoldsSeveral,
  type GenreEvidence,
  type GenreEvidenceTrack,
  type GenreProposal,
  type GenreProposalAlbum,
} from "../assistant";
import "./AssistantProposalsDialog.css";

export interface AssistantProposalsDialogProps {
  /** Every proposal to show, newest last; the parent keeps them current from proposal events. */
  proposals: readonly GenreProposal[];
  onProposalChange: (proposal: GenreProposal) => void;
  /** Removes a finished proposal from the list. */
  onClear: (proposalId: string) => void;
  /** Called after an apply wrote the genre of at least one album. */
  onApplied: (albums: number) => void;
  onClose: () => void;
}

const sourceNames: Record<string, string> = {
  "claude-code": "Claude Code",
  "codex-mcp-client": "Codex",
  codex: "Codex",
  "genre-check": "Genre check by sound",
};

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function albumLabel(album: GenreProposalAlbum): string {
  const title = album.title ?? "Unknown album";
  const artist = album.artist ? `${album.artist} — ` : "";
  return `${artist}${title}${album.year ? ` (${album.year})` : ""}`;
}

/** Albums waiting for a decision; failed ones can be tried again. */
function isOpen(album: GenreProposalAlbum): boolean {
  return album.status === "pending" || album.status === "failed";
}

function trackLabel(track: GenreEvidenceTrack): string {
  const artist = track.artist ? `${track.artist} — ` : "";
  return `${artist}${track.title ?? "Unknown track"}${track.album ? ` (${track.album}${track.year ? `, ${track.year}` : ""})` : ""}`;
}

function evidenceLabel(evidence: GenreEvidence): string {
  if (evidence.kind === "fits") return "Fits its genre";
  const confidence = evidence.confidence === "high" ? "High confidence" : "Medium confidence";
  return `${confidence} · ${evidence.kind === "refine" ? "more specific genre" : "sounds like another genre"}`;
}

/** The genre check's reasoning: confidence, the numbers, close genres and the closest songs. */
function Evidence({ evidence }: { evidence: GenreEvidence }) {
  return <div className="assistant-proposal__evidence" data-confidence={evidence.kind === "fits" ? "fits" : evidence.confidence}>
    <span className="assistant-proposal__confidence">{evidenceLabel(evidence)}</span>
    <span>{evidence.summary}</span>
    {evidence.closest.length > 0 && <span className="assistant-proposal__closest" title="How typical of each genre this album would be: closer to the genre's average sound than this share of its albums">
      {evidence.kind === "fits" ? "Next closest" : "Close genres"}: {evidence.closest.map((near) => `${near.genre} ${near.percentile}%`).join(" · ")}
    </span>}
    {evidence.tracks.length > 0 && <ul aria-label="Closest songs of the suggested genre">
      {evidence.tracks.map((track) => <li key={track.trackId}><AudioLines aria-hidden="true" />{trackLabel(track)}</li>)}
    </ul>}
  </div>;
}

function ProposalCard({ proposal, onProposalChange, onClear, onApplied }: Omit<AssistantProposalsDialogProps, "proposals" | "onClose"> & { proposal: GenreProposal }) {
  // Medium-confidence findings and albums that fit wait for an explicit tick.
  const [unchecked, setUnchecked] = useState<ReadonlySet<string>>(() => new Set(proposal.albums.filter((album) => album.evidence && (album.evidence.kind === "fits" || album.evidence.confidence === "medium")).map((album) => album.albumId)));
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  const busy = working || proposal.status === "applying";
  const open = proposal.albums.filter(isOpen);
  const chosen = open.filter((album) => !unchecked.has(album.albumId));
  const genreFor = (album: GenreProposalAlbum) => edits[album.albumId] ?? album.proposedGenre;
  const invalid = chosen.some((album) => !genreFor(album).trim());
  const applied = proposal.albums.filter((album) => album.status === "applied").length;
  const failed = proposal.albums.filter((album) => album.status === "failed").length;

  const toggle = (albumId: string, checked: boolean) => setUnchecked((current) => {
    const next = new Set(current);
    if (checked) next.delete(albumId);
    else next.add(albumId);
    return next;
  });

  const apply = async () => {
    setWorking(true);
    setError(null);
    const before = applied;
    try {
      const result = await applyGenreProposal(proposal.id, chosen.map((album) => ({ albumId: album.albumId, genre: genreFor(album).trim() })));
      onProposalChange(result);
      const written = result.albums.filter((album) => album.status === "applied").length - before;
      if (written > 0) onApplied(written);
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setWorking(false);
    }
  };

  const dismiss = async () => {
    setWorking(true);
    setError(null);
    try {
      onProposalChange(await dismissGenreProposal(proposal.id));
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setWorking(false);
    }
  };

  return <section className="assistant-proposal" aria-label={`Genre changes from ${sourceNames[proposal.source] ?? proposal.source}`}>
    <header>
      <div>
        <strong>{sourceNames[proposal.source] ?? proposal.source}</strong>
        <small>{new Date(proposal.createdAtMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} · {formatCount(proposal.albums.length)} {proposal.albums.length === 1 ? "album" : "albums"}</small>
      </div>
      {proposal.status === "applying" && <span className="assistant-proposal__state">Writing tags…</span>}
      {proposal.status === "done" && <span className="assistant-proposal__state" data-done>{applied ? `${formatCount(applied)} changed` : "Nothing changed"}</span>}
    </header>
    {proposal.reason && <p className="assistant-proposal__reason">{proposal.reason}</p>}
    <ul>
      {proposal.albums.map((album) => {
        const editable = isOpen(album) && !busy;
        return <li key={album.albumId} data-status={album.status}>
          <input type="checkbox" aria-label={`Change ${albumLabel(album)}`} checked={isOpen(album) && !unchecked.has(album.albumId)} disabled={!editable} onChange={(event) => toggle(album.albumId, event.target.checked)} />
          <div className="assistant-proposal__album">
            <strong>{albumLabel(album)}</strong>
            <span className="assistant-proposal__tags">
              {album.currentTags.length ? album.currentTags.map((tag) => <span key={tag.tag} data-several={genreTagHoldsSeveral(tag.tag) || undefined}>{tag.tag}<small> · {formatCount(tag.tracks)}</small></span>) : <span>No genre</span>}
            </span>
            {album.evidence && <Evidence evidence={album.evidence} />}
            {album.error && <span className="assistant-proposal__error">{album.error}</span>}
          </div>
          {isOpen(album)
            ? <>
              <input className="assistant-proposal__genre" aria-label={`New genre for ${albumLabel(album)}`} value={genreFor(album)} maxLength={256} disabled={!editable || unchecked.has(album.albumId)} list={album.evidence?.closest.length ? `genre-options-${proposal.id}-${album.albumId}` : undefined} onChange={(event) => setEdits((current) => ({ ...current, [album.albumId]: event.target.value }))} />
              {album.evidence?.closest.length ? <datalist id={`genre-options-${proposal.id}-${album.albumId}`}>
                {[album.proposedGenre, ...album.evidence.closest.map((near) => near.genre)].filter((genre, index, all) => all.indexOf(genre) === index).map((genre) => <option key={genre} value={genre} />)}
              </datalist> : null}
            </>
            : <span className="assistant-proposal__result">{album.status === "applied" ? <><Check aria-hidden="true" />{album.appliedGenre}</> : "Skipped"}</span>}
        </li>;
      })}
    </ul>
    {error && <p className="assistant-proposal__error" role="alert"><TriangleAlert aria-hidden="true" />{error}</p>}
    {failed > 0 && !busy && <p className="assistant-proposal__error">{formatCount(failed)} {failed === 1 ? "album was" : "albums were"} not changed. You can try again.</p>}
    <footer>
      {open.length > 0 ? <>
        <button type="button" onClick={() => void dismiss()} disabled={busy}>Dismiss</button>
        <button type="button" className="assistant-proposal__apply" onClick={() => void apply()} disabled={busy || chosen.length === 0 || invalid}>
          {busy ? "Writing tags…" : `Write genre to ${formatCount(chosen.length)} ${chosen.length === 1 ? "album" : "albums"}`}
        </button>
      </> : <button type="button" onClick={() => onClear(proposal.id)} disabled={busy}>Clear</button>}
    </footer>
  </section>;
}

/** Shows genre changes an AI assistant proposed; nothing is written until the user applies them here. */
export function AssistantProposalsDialog({ proposals, onProposalChange, onClear, onApplied, onClose }: AssistantProposalsDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialogRef.current?.showModal(); }, []);
  const visible = proposals.filter((proposal) => proposal.status !== "dismissed");

  return <dialog ref={dialogRef} className="assistant-proposals-dialog" aria-labelledby="assistant-proposals-title" onClose={onClose} onCancel={(event) => {
    // Escape must not close the dialog while tags are being written.
    if (visible.some((proposal) => proposal.status === "applying")) event.preventDefault();
  }}>
    <header>
      <div>
        <p className="eyebrow"><Sparkles aria-hidden="true" /> Genre review</p>
        <h2 id="assistant-proposals-title">Review genre changes</h2>
        <p>Each album gets the genre you approve on every track, written to the MP3 files and sent to Music Library. You can change a genre before writing it.</p>
      </div>
      <button type="button" aria-label="Close" onClick={() => dialogRef.current?.close()}><X aria-hidden="true" /></button>
    </header>
    <div className="assistant-proposals-dialog__body">
      {visible.length ? [...visible].reverse().map((proposal) => <ProposalCard key={proposal.id} proposal={proposal} onProposalChange={onProposalChange} onClear={onClear} onApplied={onApplied} />)
        : <p className="assistant-proposals-dialog__empty">No changes are waiting. Run “Check genres by sound” on the Genres page, or ask an assistant, for example: “Find albums with more than one genre and propose one genre for each.”</p>}
    </div>
  </dialog>;
}
