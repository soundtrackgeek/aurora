import { CalendarDays, Clock3 } from "lucide-react";

export function YearsPlaceholder() {
  return (
    <section className="years-placeholder" aria-labelledby="years-placeholder-title">
      <div className="years-placeholder__icon"><CalendarDays aria-hidden="true" /></div>
      <p className="eyebrow">Library · Years</p>
      <h1 id="years-placeholder-title">Your collection through time.</h1>
      <p>Year-by-year exploration has its own place now. The timeline and decade drill-down will arrive in a focused release.</p>
      <span><Clock3 aria-hidden="true" /> Placeholder in Aurora 0.12.0</span>
    </section>
  );
}
