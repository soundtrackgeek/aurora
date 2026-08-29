import { Flag } from "lucide-react";
import type { CatalogChartRank } from "../../charts";
import "./CatalogChartRanks.css";

export function CatalogChartRanks({
  ranks,
  kind,
}: {
  ranks?: readonly CatalogChartRank[];
  kind: "track" | "album";
}) {
  if (!ranks?.length) return null;
  const description = ranks.map((rank) => `${rank.label} number ${rank.rank}`).join(", ");
  return (
    <span className={`catalog-chart-ranks catalog-chart-ranks--${kind}`} aria-label={`Chart rankings: ${description}`}>
      <Flag aria-hidden="true" />
      {ranks.map((rank) => (
        <span key={rank.source} title={`${rank.label} #${rank.rank}`}>
          <strong>{rank.shortLabel}</strong>:#{rank.rank}
        </span>
      ))}
    </span>
  );
}
