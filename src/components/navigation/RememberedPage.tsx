import { Activity, memo, useState, type ReactNode } from "react";

// Shared library/inspector props must not overwrite a hidden page's selection.
const PageContent = memo(function PageContent({ children }: { active: boolean; children: ReactNode }) {
  return children;
}, (_previous, next) => !next.active);

/** Mount on the first visit, then retain local view state while pausing hidden effects. */
export function RememberedPage({ active, children }: { active: boolean; children: ReactNode }) {
  const [visited, setVisited] = useState(active);
  if (active && !visited) setVisited(true);
  return visited || active ? <Activity mode={active ? "visible" : "hidden"}>
    <PageContent active={active}>{children}</PageContent>
  </Activity> : null;
}
