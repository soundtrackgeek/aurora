import type { ChartEntry, ChartPageRequest } from "./charts";

const key = "aurora:charts:v1";
type Selection = Pick<ChartEntry, "artistKey" | "titleKey">;
interface ChartPreferences { request: ChartPageRequest; selection: Selection | null }

export function loadChartPreferences(fallback: ChartPageRequest): ChartPreferences {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? "null");
    const r = value?.request;
    const p = r?.period;
    const integer = (n: unknown, min: number, max: number) => Number.isInteger(n) && Number(n) >= min && Number(n) <= max;
    const sources = r?.kind === "albums" ? ["auroraScore", "officialUk", "vgLista", "billboard"] : ["officialUk", "vgLista", "tiISkuddet", "norsktoppen", "billboard"];
    if (!r || !["singles", "albums"].includes(r.kind) || !sources.includes(r.source)
      || !["week", "period"].includes(r.scope) || !["year", "releaseYear"].includes(r.yearBasis)
      || !integer(r.selectedYear, 1890, 2200) || !integer(r.selectedWeek, 1, 53)
      || !integer(r.limit, 1, 1000) || !p || typeof p.label !== "string"
      || !integer(p.fromYear, 1890, 2200) || !integer(p.toYear, p.fromYear, Math.min(2200, p.fromYear + 20))
      || !integer(p.fromWeek, 1, 53) || !integer(p.toWeek, 1, 53)
      || p.fromYear * 100 + p.fromWeek > p.toYear * 100 + p.toWeek
      || (["billboard", "auroraScore"].includes(r.source) && r.scope !== "period")
      || (r.filters && (typeof r.filters.country !== "string" || r.filters.country.length > 80
        || !["", "Person", "Group"].includes(r.filters.artistType)
        || !["", "alive", "dead", "active", "disbanded"].includes(r.filters.status)))) {
      return { request: fallback, selection: null };
    }
    const selection = value.selection;
    return { request: r, selection: selection && typeof selection.artistKey === "string" && typeof selection.titleKey === "string" ? selection : null };
  } catch { return { request: fallback, selection: null }; }
}

export function saveChartPreferences(request: ChartPageRequest, selection: Selection | null) {
  try {
    localStorage.setItem(key, JSON.stringify({ request, selection: selection ? { artistKey: selection.artistKey, titleKey: selection.titleKey } : null }));
  } catch { /* Charts remains usable when storage is unavailable. */ }
}
