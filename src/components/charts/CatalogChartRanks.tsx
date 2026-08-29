import { Flag } from "lucide-react";
import type { CatalogChartRank } from "../../charts";
import { CountryFlag } from "../CountryFlag";
import "./CatalogChartRanks.css";

const albumChartCountries: Partial<Record<CatalogChartRank["source"], { code: string; name: string }>> = {
  billboard: { code: "US", name: "United States" },
  officialUk: { code: "GB", name: "United Kingdom" },
  vgLista: { code: "NO", name: "Norway" },
};

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
      {kind === "track" ? <Flag aria-hidden="true" /> : null}
      {ranks.map((rank) => {
        const country = kind === "album" ? albumChartCountries[rank.source] : undefined;
        return (
          <span key={rank.source} title={`${rank.label} #${rank.rank}`}>
            {country
              ? <CountryFlag code={country.code} name={country.name} ariaLabel={`${country.name} chart`} />
              : <><strong>{rank.shortLabel}</strong>:</>}
            #{rank.rank}
          </span>
        );
      })}
    </span>
  );
}
