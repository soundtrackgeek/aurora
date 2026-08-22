import {
  AlertTriangle,
  BadgeCheck,
  DatabaseBackup,
  LoaderCircle,
  Orbit,
  RefreshCw,
  Telescope,
  Undo2,
} from "lucide-react";
import type { ArtistReviewFilter, ArtistReviewItem } from "../../musicbrainz";
import "./Observatory.css";

export type ObservatoryLoadState = "loading" | "ready" | "error";

export interface ObservatoryProps {
  items: ArtistReviewItem[];
  selectedArtistKey: string | null;
  filter: ArtistReviewFilter;
  loadState: ObservatoryLoadState;
  errorMessage: string | null;
  hasMore: boolean;
  loadingMore: boolean;
  actionBusy: "export" | "undo" | null;
  message: string | null;
  onFilterChange: (filter: ArtistReviewFilter) => void;
  onSelect: (item: ArtistReviewItem) => void;
  onLoadMore: () => void;
  onRefresh: () => void;
  onUndo: () => void;
  onExport: () => void;
}

const filters: Array<{ id: ArtistReviewFilter; label: string }> = [
  { id: "needsReview", label: "Needs review" },
  { id: "conflict", label: "Conflicts" },
  { id: "unconfirmed", label: "Unconfirmed" },
  { id: "decided", label: "Aurora decisions" },
  { id: "all", label: "All candidates" },
];

function statusCopy(item: ArtistReviewItem): { label: string; tone: string } {
  if (item.decision?.decision === "confirmed") return { label: "Aurora confirmed", tone: "verified" };
  if (item.decision?.decision === "ignored") return { label: "Aurora ignored", tone: "quiet" };
  if (item.matchState === "conflict") return { label: "Conflict", tone: "warning" };
  if (item.matchState === "unconfirmed") return { label: "Unconfirmed", tone: "candidate" };
  if (item.matchState === "verified") return { label: "Curated overlay", tone: "verified" };
  return { label: item.matchState === "ignored" ? "Ignored" : "Unmatched", tone: "quiet" };
}

export function Observatory({
  items,
  selectedArtistKey,
  filter,
  loadState,
  errorMessage,
  hasMore,
  loadingMore,
  actionBusy,
  message,
  onFilterChange,
  onSelect,
  onLoadMore,
  onRefresh,
  onUndo,
  onExport,
}: ObservatoryProps) {
  return (
    <section className="observatory" aria-labelledby="observatory-title">
      <header className="observatory__hero">
        <div className="observatory__scope">
          <p className="eyebrow"><Telescope aria-hidden="true" /> Observatory</p>
          <h1 id="observatory-title">Resolve your music universe.</h1>
          <p>Review local MusicBrainz candidates, keep every source visible, and make only explicit decisions.</p>
        </div>
        <div className="observatory__actions">
          <button type="button" onClick={onUndo} disabled={actionBusy !== null}>
            {actionBusy === "undo" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Undo2 aria-hidden="true" />}
            Undo last
          </button>
          <button type="button" className="is-primary" onClick={onExport} disabled={actionBusy !== null}>
            {actionBusy === "export" ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <DatabaseBackup aria-hidden="true" />}
            Export overlay snapshot
          </button>
        </div>
        <div className="observatory__orbit" aria-hidden="true"><span /><i /><i /><i /></div>
      </header>

      <div className="observatory__toolbar">
        <div role="tablist" aria-label="MusicBrainz review filter">
          {filters.map((option) => (
            <button
              type="button"
              role="tab"
              aria-selected={filter === option.id}
              key={option.id}
              onClick={() => onFilterChange(option.id)}
            >{option.label}</button>
          ))}
        </div>
        <p>Candidate-bearing artists · bounded local pages</p>
      </div>

      {message ? <p className="observatory__message" role="status">{message}</p> : null}

      {loadState === "loading" ? (
        <div className="observatory__feedback" role="status">
          <LoaderCircle className="is-spinning" aria-hidden="true" />
          <strong>Building a bounded review page…</strong>
          <span>MusicBrainz sources stay off Aurora’s startup path.</span>
        </div>
      ) : loadState === "error" ? (
        <div className="observatory__feedback is-error" role="alert">
          <AlertTriangle aria-hidden="true" />
          <strong>The review page could not be opened</strong>
          <span>{errorMessage}</span>
          <button type="button" onClick={onRefresh}><RefreshCw aria-hidden="true" /> Retry</button>
        </div>
      ) : items.length === 0 ? (
        <div className="observatory__feedback">
          <Orbit aria-hidden="true" />
          <strong>No artists in this review slice</strong>
          <span>Try another filter or search term.</span>
        </div>
      ) : (
        <ol className="observatory__list" aria-label="Artists awaiting MusicBrainz review">
          {items.map((item) => {
            const status = statusCopy(item);
            return (
              <li key={item.artistKey} className={selectedArtistKey === item.artistKey ? "is-selected" : undefined}>
                <button type="button" onClick={() => onSelect(item)} aria-pressed={selectedArtistKey === item.artistKey}>
                  <span className={`observatory__status is-${status.tone}`}>
                    {status.tone === "verified" ? <BadgeCheck aria-hidden="true" /> : status.tone === "quiet" ? <Orbit aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
                  </span>
                  <span className="observatory__artist">
                    <strong>{item.displayArtist}</strong>
                    <small>{item.candidates.length} local candidate{item.candidates.length === 1 ? "" : "s"}{item.hasExternalConflict ? " · sources disagree" : ""}</small>
                  </span>
                  <span className={`observatory__badge is-${status.tone}`}>{status.label}</span>
                </button>
              </li>
            );
          })}
        </ol>
      )}

      {loadState === "ready" && hasMore ? (
        <button type="button" className="observatory__more" onClick={onLoadMore} disabled={loadingMore}>
          {loadingMore ? <LoaderCircle className="is-spinning" aria-hidden="true" /> : <Orbit aria-hidden="true" />}
          {loadingMore ? "Tracing the next page…" : "Load next review page"}
        </button>
      ) : null}
    </section>
  );
}
