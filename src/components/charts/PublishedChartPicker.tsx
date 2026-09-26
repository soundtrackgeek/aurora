import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";
import { isTauriRuntime } from "../../library";
import { loadPublishedChartSeries, loadPublishedChartWeeks, type ChartPageRequest, type PublishedChartSeries } from "../../charts";

export function PublishedChartPicker({ request, onChange, revision, onRefresh }: { request: ChartPageRequest; onChange: Dispatch<SetStateAction<ChartPageRequest>>; revision: number; onRefresh: () => void }) {
  const [series, setSeries] = useState<PublishedChartSeries[]>([]);
  const [calendar, setCalendar] = useState<{ chart: string; year: number; dates: string[] } | null>(null);
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    void loadPublishedChartSeries().then((items) => {
      if (!active) return;
      setSeries(items);
      setError(items.length ? "" : "Open Published Charts in Music Library and wait for the US archive to finish preparing, then refresh here. The annual Singles CSV import does not load weekly charts.");
      onChange((current) => {
        const item = items.find((s) => s.chart === current.publishedChart) ?? items.find((s) => s.chart === "Billboard Hot 100") ?? items[0];
        if (!item) return current;
        const year = item.years.includes(current.selectedYear) ? current.selectedYear : item.years[item.years.length - 1];
        if (item.chart === current.publishedChart && year === current.selectedYear) return current;
        return { ...current, publishedChart: item.chart, publishedWeek: undefined, selectedYear: year, period: { fromYear: year, toYear: year, fromWeek: 1, toWeek: 53, label: `${year} year chart` } };
      });
    }).catch((cause) => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, [onChange, revision, retry]);
  useEffect(() => {
    const chart = request.publishedChart;
    const year = request.selectedYear;
    if (!series.some((item) => item.chart === chart && item.years.includes(year)) || !chart) return;
    let active = true;
    void loadPublishedChartWeeks(chart, year).then((dates) => {
      if (!active) return;
      setCalendar({ chart, year, dates }); setError("");
      onChange((current) => current.publishedChart !== chart || current.selectedYear !== year || dates.includes(current.publishedWeek ?? "")
        ? current : { ...current, publishedWeek: dates[0] });
    }).catch((cause) => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, [request.publishedChart, request.selectedYear, series, onChange, retry]);
  const selected = series.find((item) => item.chart === request.publishedChart);
  const dates = calendar && calendar.chart === request.publishedChart && calendar.year === request.selectedYear ? calendar.dates : [];
  const index = dates.indexOf(request.publishedWeek ?? "");
  function changeYear(year: number, chart = request.publishedChart) {
    onChange((current) => ({ ...current, publishedChart: chart, publishedWeek: undefined, selectedYear: year, selectedWeek: 1, period: { fromYear: year, toYear: year, fromWeek: 1, toWeek: 53, label: `${year} year chart` } }));
  }
  function chooseDate(date: string) { onChange((current) => ({ ...current, publishedWeek: date, scope: "week" })); }
  return <section className="published-chart-picker" aria-label="US weekly archive">
    <div className="published-chart-picker__fields">
      <label>Find a US chart<input type="search" placeholder="Country, rock, airplay…" value={search} onChange={(event) => setSearch(event.target.value)} /></label>
      <label>US chart<select value={selected?.chart ?? ""} onChange={(event) => {
        const next = series.find((item) => item.chart === event.target.value);
        if (next) changeYear(next.years.includes(request.selectedYear) ? request.selectedYear : next.years[next.years.length - 1], next.chart);
      }}><option value="" disabled>Choose chart</option>{series.filter((item) => item.chart === selected?.chart || item.chart.toLocaleLowerCase().includes(search.toLocaleLowerCase().trim())).map((item) => <option key={item.chart}>{item.chart}</option>)}</select></label>
      <label>Chart year<select value={selected ? request.selectedYear : ""} onChange={(event) => changeYear(Number(event.target.value))}>{!selected ? <option value="">—</option> : selected.years.map((year) => <option key={year}>{year}</option>)}</select></label>
      <label>Week ending<select value={index >= 0 ? request.publishedWeek : ""} onChange={(event) => chooseDate(event.target.value)}><option value="" disabled>{dates.length ? "Choose week" : "No available weeks"}</option>{dates.map((date) => <option value={date} key={date}>{new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" }).format(new Date(`${date}T12:00:00Z`))}</option>)}</select></label>
      <div className="published-chart-picker__steps"><button type="button" aria-label="Previous published week" disabled={index <= 0} onClick={() => chooseDate(dates[index - 1])}><ChevronLeft /></button><button type="button" aria-label="Next published week" disabled={index < 0 || index >= dates.length - 1} onClick={() => chooseDate(dates[index + 1])}><ChevronRight /></button><button type="button" aria-label="Refresh US chart archive" onClick={() => { setRetry((value) => value + 1); onRefresh(); }}><RefreshCw /></button></div>
    </div>
    {error ? <p role="alert">{error}</p> : <p>{!isTauriRuntime() ? "Browser preview uses sample songs and dates. " : ""}{series.length} imported US charts · {dates.length} published weeks in {request.selectedYear}. Dates follow the source archive; availability varies by chart.</p>}
  </section>;
}
