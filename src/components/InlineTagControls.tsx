import { Ban, Heart, Star, StarHalf } from "lucide-react";
import { useContext } from "react";
import { ShortcutRatingSaveContext } from "../app/tags/ratingSaveContext";
import type { LoveState } from "../tags";

interface InlineRatingControlProps {
  trackKey: string;
  title: string;
  rating: number | null;
  busy: boolean;
  allowClear?: boolean;
  onRatingChange: (rating: number | null) => void;
}

interface InlineLoveControlProps {
  title: string;
  loveState: LoveState;
  busy: boolean;
  onLoveChange: (loveState: LoveState) => void;
}

function RatingIcon({ rating, star }: { rating: number | null; star: number }) {
  if (rating !== null && rating === star - 0.5) {
    return <StarHalf aria-hidden="true" className="is-filled" />;
  }
  return (
    <Star
      aria-hidden="true"
      className={rating !== null && rating >= star ? "is-filled" : undefined}
    />
  );
}

export function InlineRatingControl({
  trackKey,
  title,
  rating,
  busy,
  allowClear = false,
  onRatingChange,
}: InlineRatingControlProps) {
  const save = useContext(ShortcutRatingSaveContext)[trackKey];
  const pending = save?.pending === true;
  const displayedRating = pending ? save.rating : rating;
  const savingLabel = pending
    ? (save.rating === null ? `Clearing rating for ${title}…` : `Saving ${save.rating} star rating for ${title}…`)
    : undefined;
  return (
    <span className={`inline-rating${pending ? " is-saving" : ""}`}
      data-clearing={pending && displayedRating === null ? "true" : undefined}
      role="group" aria-label={`Rate ${title}`} aria-busy={busy || pending}
      aria-description={savingLabel} title={savingLabel}>
      {[1, 2, 3, 4, 5].map((star) => (
        <span className="inline-rating__star" key={star}>
          <RatingIcon rating={displayedRating} star={star} />
          {[star].map((value) => (
            <button
              type="button"
              className="inline-rating__whole"
              aria-label={allowClear && displayedRating === value
                ? `Clear rating for ${title}`
                : `Rate ${title} ${value.toFixed(1)} stars`}
              aria-pressed={displayedRating === value}
              disabled={busy || pending}
              onClick={(event) => {
                event.stopPropagation();
                onRatingChange(allowClear && rating === value ? null : value);
              }}
              onDoubleClick={(event) => event.stopPropagation()}
              key={value}
            />
          ))}
        </span>
      ))}
    </span>
  );
}

export function InlineLoveControl({
  title,
  loveState,
  busy,
  onLoveChange,
}: InlineLoveControlProps) {
  const nextLoveState = loveState === "loved" ? "neutral" : "loved";
  const loveLabel = loveState === "loved"
    ? `Remove Love from ${title}`
    : loveState === "banned"
      ? `Love ${title}, currently banned`
      : `Love ${title}`;

  return (
    <button
      type="button"
      className={`inline-love${loveState === "loved" ? " is-loved" : loveState === "banned" ? " is-banned" : ""}`}
      aria-label={loveLabel}
      aria-pressed={loveState === "loved"}
      disabled={busy}
      onClick={(event) => {
        event.stopPropagation();
        onLoveChange(nextLoveState);
      }}
      onDoubleClick={(event) => event.stopPropagation()}
    >
      {loveState === "banned" ? <Ban aria-hidden="true" /> : <Heart aria-hidden="true" />}
    </button>
  );
}
