import { Ban, Heart, Star, StarHalf } from "lucide-react";
import type { LoveState } from "../tags";

interface InlineRatingControlProps {
  title: string;
  rating: number | null;
  busy: boolean;
  onRatingChange: (rating: number) => void;
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
  title,
  rating,
  busy,
  onRatingChange,
}: InlineRatingControlProps) {
  return (
    <span className="inline-rating" role="group" aria-label={`Rate ${title}`} aria-busy={busy}>
      {[1, 2, 3, 4, 5].map((star) => (
        <span className="inline-rating__star" key={star}>
          <RatingIcon rating={rating} star={star} />
          {[star - 0.5, star].map((value) => (
            <button
              type="button"
              className={value === star ? "inline-rating__full" : "inline-rating__half"}
              aria-label={`Rate ${title} ${value.toFixed(1)} stars`}
              aria-pressed={rating === value}
              disabled={busy}
              onClick={(event) => {
                event.stopPropagation();
                onRatingChange(value);
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
