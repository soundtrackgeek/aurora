import { CalendarRange, ChevronDown, SlidersHorizontal, X } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { emptyChartFilters, monthPeriod, periodSeasons, type ChartArtistFilters, type ChartPeriod } from "../../charts";

const months = Array.from({ length: 12 }, (_, index) => new Intl.DateTimeFormat("en", { month: "long", timeZone: "UTC" }).format(new Date(Date.UTC(2000, index, 1))));
const regionNames = new Intl.DisplayNames(["en"], { type: "region" });
const countries = Object.keys(import.meta.glob("/node_modules/flag-icons/flags/4x3/*.svg"))
  .map((path) => path.split("/").pop()!.replace(".svg", "").toUpperCase())
  .filter((code) => /^[A-Z]{2}$/.test(code))
  .map((code) => ({ code, name: regionNames.of(code) ?? code }))
  .sort((a, b) => a.name.localeCompare(b.name));

export function ChartPeriodControls({ period, filters = emptyChartFilters, onApply, onFilters }: {
  period: ChartPeriod; filters?: ChartArtistFilters;
  onApply: (period: ChartPeriod) => void; onFilters: (filters: ChartArtistFilters) => void;
}) {
  const [presetOpen, setPresetOpen] = useState(false);
  const [year, setYear] = useState(period.fromYear);
  const [unit, setUnit] = useState("week");
  const [fromYear, setFromYear] = useState(period.fromYear);
  const [toYear, setToYear] = useState(period.toYear);
  const [from, setFrom] = useState(period.fromWeek);
  const [to, setTo] = useState(period.toWeek);
  const [error, setError] = useState("");
  const [draftFilters, setDraftFilters] = useState(filters);
  const [lastPeriod, setLastPeriod] = useState(period);
  const dialog = useRef<HTMLDialogElement>(null);
  const picker = useRef<HTMLDivElement>(null);
  if (lastPeriod !== period) {
    setLastPeriod(period);
    setYear(period.fromYear);
    setFromYear(period.fromYear);
    setToYear(period.toYear);
    setUnit("week");
    setFrom(period.fromWeek);
    setTo(period.toWeek);
    setError("");
  }
  useEffect(() => {
    if (!presetOpen) return;
    const close = (event: PointerEvent) => { if (!picker.current?.contains(event.target as Node)) setPresetOpen(false); };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [presetOpen]);
  function apply(next: ChartPeriod, custom = false) {
    setLastPeriod(next);
    setYear(next.fromYear);
    setError("");
    setPresetOpen(false);
    if (!custom) {
      setFromYear(next.fromYear);
      setToYear(next.toYear);
      setUnit("week");
      setFrom(next.fromWeek);
      setTo(next.toWeek);
    }
    onApply(next);
  }
  function submit(event: FormEvent) {
    event.preventDefault();
    if (fromYear * 100 + from > toYear * 100 + to || toYear - fromYear > 20) {
      setError("Choose a chronological range no longer than 20 years.");
      return;
    }
    apply(unit === "month"
      ? monthPeriod(fromYear, from, toYear, to, `${months[from - 1]} ${fromYear} – ${months[to - 1]} ${toYear}`)
      : { fromYear, toYear, fromWeek: from, toWeek: to, label: `${fromYear} W${from} – ${toYear} W${to}` }, true);
  }
  const filterCount = Object.values(filters).filter(Boolean).length;
  return <div className="chart-period-controls">
    <div className="chart-period-picker" ref={picker} onKeyDown={(event) => { if (event.key === "Escape") setPresetOpen(false); }}>
      <button className="chart-period-trigger" type="button" aria-expanded={presetOpen} aria-controls="chart-period-presets" onClick={() => setPresetOpen(!presetOpen)}>
        <CalendarRange aria-hidden="true" /><span><small>Preset period</small><strong>{period.label}</strong></span><ChevronDown aria-hidden="true" />
      </button>
      {presetOpen && <div className="chart-period-popover" id="chart-period-presets">
        <h2>Choose a period</h2><p>A ready-made moment, in any year.</p>
        <label>Preset year<input type="number" min="1890" max="2199" value={year} onChange={(event) => setYear(Number(event.target.value))} /></label>
        <div className="chart-season-grid">{periodSeasons.map((season) => <button key={season.name} type="button" aria-pressed={period.label === `${season.name} ${year}`} disabled={year < 1890 || year > 2199} onClick={() => apply(monthPeriod(year, season.from, year + Number(season.to < season.from), season.to, `${season.name} ${year}`))}><strong>{season.name}</strong><small>{season.detail}</small></button>)}</div>
      </div>}
    </div>
    <form className="chart-custom-row" onSubmit={submit}>
      <strong>Custom</strong>
      <label>From Year<input type="number" min="1890" max="2200" required value={fromYear} onChange={(event) => setFromYear(Number(event.target.value))} /></label>
      <label>To Year<input type="number" min="1890" max="2200" required value={toYear} onChange={(event) => setToYear(Number(event.target.value))} /></label>
      <label>Unit<select value={unit} onChange={(event) => { setUnit(event.target.value); setFrom(1); setTo(event.target.value === "month" ? 12 : 53); }}><option value="week">Weeks</option><option value="month">Months</option></select></label>
      <label>From {unit === "month" ? "Month" : "Week"}{unit === "month" ? <select value={from} onChange={(event) => setFrom(Number(event.target.value))}>{months.map((month, index) => <option key={month} value={index + 1}>{month}</option>)}</select> : <input type="number" min="1" max="53" required value={from} onChange={(event) => setFrom(Number(event.target.value))} />}</label>
      <label>To {unit === "month" ? "Month" : "Week"}{unit === "month" ? <select value={to} onChange={(event) => setTo(Number(event.target.value))}>{months.map((month, index) => <option key={month} value={index + 1}>{month}</option>)}</select> : <input type="number" min="1" max="53" required value={to} onChange={(event) => setTo(Number(event.target.value))} />}</label>
      <button className="button button--primary" type="submit">Apply range</button>
      <button className="button button--quiet" type="button" onClick={() => { setDraftFilters(filters); dialog.current?.showModal(); }}><SlidersHorizontal aria-hidden="true" />Advanced filters{filterCount ? ` (${filterCount})` : ""}</button>
    </form>
    {error && <p role="alert">{error}</p>}
    {unit === "month" && <p className="chart-period-note">Months include the chart weeks touching that range. Annual sources use the selected years.</p>}
    {filterCount > 0 && <p className="chart-period-note">Artist filters: {Object.values(filters).filter(Boolean).join(" · ")} <button type="button" onClick={() => onFilters(emptyChartFilters)}>Clear filters</button></p>}
    <dialog className="chart-dialog chart-filter-dialog" aria-label="Advanced artist filters" ref={dialog} onClick={(event) => { if (event.target === event.currentTarget) dialog.current?.close(); }}>
      <form onSubmit={(event) => { event.preventDefault(); onFilters({ ...draftFilters, country: draftFilters.country.trim() }); dialog.current?.close(); }}>
        <header><div><p className="eyebrow">Charts</p><h2>Advanced artist filters</h2></div><button type="button" aria-label="Close advanced filters" onClick={() => dialog.current?.close()}><X aria-hidden="true" /></button></header>
        <label>Country<input list="chart-countries" placeholder="Any country — search by name or code" value={draftFilters.country} maxLength={80} onChange={(event) => setDraftFilters({ ...draftFilters, country: event.target.value })} /></label>
        <datalist id="chart-countries">{countries.map(({ code, name }) => <option key={code} value={code}>{name}</option>)}</datalist>
        <label>Type<select value={draftFilters.artistType} onChange={(event) => setDraftFilters({ ...draftFilters, artistType: event.target.value as ChartArtistFilters["artistType"], status: "" })}><option value="">Any type</option><option>Person</option><option>Group</option></select></label>
        <label>Status<select value={draftFilters.status} onChange={(event) => setDraftFilters({ ...draftFilters, status: event.target.value as ChartArtistFilters["status"] })}><option value="">Any status</option>{draftFilters.artistType !== "Group" && <><option value="alive">Alive</option><option value="dead">Dead</option></>}{draftFilters.artistType !== "Person" && <><option value="active">Active group</option><option value="disbanded">Disbanded</option></>}</select></label>
        <p>Uses Music Library’s MusicBrainz artist metadata. Unknown values do not match an active filter. Status reflects the latest stored metadata, not the chart year. Joint artist credits require their own matching metadata.</p>
        <footer><button type="button" className="button button--quiet" onClick={() => setDraftFilters(emptyChartFilters)}>Reset</button><button type="button" className="button button--quiet" onClick={() => dialog.current?.close()}>Cancel</button><button type="submit" className="button button--primary">Apply filters</button></footer>
      </form>
    </dialog>
  </div>;
}
