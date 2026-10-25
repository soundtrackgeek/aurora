import { LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import type { LibraryIntakeProgress } from "../../ingest";

type IntakeMode = "preview" | "apply";

const previewStages = [
  ["scanning", "Scan"],
  ["analyzing", "Analyze"],
  ["fingerprinting", "Verify files"],
] as const;

const applyStages = [
  ["validating", "Validate"],
  ["transferring", "Transfer"],
  ["verifying", "Verify"],
  ["cataloging", "Catalog"],
  ["artwork", "Artwork"],
  ["finalizing", "Finish"],
  ["refreshing", "Refresh"],
] as const;

/** Finer steps Music Library may report while it updates its catalog. */
const catalogSubstages = new Set([
  "backingUp", "maintenance", "searchIndex", "catalogRows", "statistics", "charts", "committing",
]);

/** After this long without finishing, say what is (and is not) known. */
const LONG_WAIT_SECONDS = 20;

export function LibraryIntakeActivity({
  mode,
  progress,
  targetLabel,
  refreshing = false,
}: {
  mode: IntakeMode;
  progress: LibraryIntakeProgress | null;
  targetLabel?: string;
  /** The move is complete on disk; Aurora is refreshing its own catalog and playback views. */
  refreshing?: boolean;
}) {
  const stages = mode === "preview" ? previewStages : applyStages;
  const fallbackStage = mode === "preview" ? "scanning" : "validating";
  const activeStage = stageForDisplay(mode, progress?.stage, fallbackStage, refreshing);
  const activeIndex = Math.max(0, stages.findIndex(([id]) => id === activeStage));
  const transferring = mode === "apply" && !refreshing && progress?.stage === "transferring";
  const transferPercent = transferring && progress.totalBytes > 0
    ? Math.min(100, Math.round((progress.processedBytes / progress.totalBytes) * 100))
    : null;
  const seconds = useSecondsInStep(`${mode}:${activeStage}`);
  const cataloging = mode === "apply" && !refreshing && activeStage === "cataloging";
  const message = refreshing
    ? "Added to library. Refreshing Aurora's view of it."
    : progress?.message ?? (mode === "preview"
      ? "Finding albums, reading tags, and checking catalog destinations."
      : "Confirming the reviewed plan before any files are moved.");

  return <section className="library-intake-activity" role="status" aria-live="polite">
    <header>
      <LoaderCircle className="is-spinning" aria-hidden="true" />
      <span><strong>{targetLabel ? `${targetLabel}: ` : ""}{message}</strong>
        {transferring && progress.totalFiles > 0
          ? <small>{progress.processedFiles} of {progress.totalFiles} files · {formatBytes(progress.processedBytes)} of {formatBytes(progress.totalBytes)}</small>
          : <small>{detail(mode, refreshing, cataloging, seconds)}</small>}
      </span>
    </header>
    <ol aria-label={`${mode === "preview" ? "Preview" : "Move"} stages`}>
      {stages.map(([id, label], index) => <li key={id} className={index < activeIndex ? "is-complete" : index === activeIndex ? "is-active" : ""}>{label}</li>)}
    </ol>
    {transferPercent !== null ? <div className="library-intake-activity__meter">
      <progress aria-label="Album transfer progress" max={100} value={transferPercent} />
      <span>{transferPercent}%</span>
    </div> : null}
  </section>;
}

function stageForDisplay(mode: IntakeMode, stage: string | undefined, fallback: string, refreshing: boolean): string {
  if (mode === "apply") {
    if (refreshing) return "refreshing";
    if (stage === "completed") return "finalizing";
    if (stage && (catalogSubstages.has(stage) || !applyStages.some(([id]) => id === stage))) return "cataloging";
    return stage ?? fallback;
  }
  return stage === "completed" ? previewStages[previewStages.length - 1][0] : stage ?? fallback;
}

function detail(mode: IntakeMode, refreshing: boolean, cataloging: boolean, seconds: number): string {
  if (mode === "preview") return "No files are changed during preview.";
  if (refreshing) {
    return seconds < LONG_WAIT_SECONDS
      ? "The files are moved and cataloged. Aurora is updating its library and playback views."
      : `Still refreshing after ${formatElapsed(seconds)}. The album is already safely in your library, so you can close this dialog. Aurora's playback engine may be busy.`;
  }
  if (cataloging && seconds >= 5) {
    return `Music Library is updating its catalog database (${formatElapsed(seconds)} so far). It reports no finer progress, and large catalogs can take several minutes. Your files are already moved and verified.`;
  }
  return "Sources remain recoverable until the catalog commit is verified.";
}

/** Whole seconds spent on the current step; restarts whenever `step` changes. */
function useSecondsInStep(step: string): number {
  const [started, setStarted] = useState(() => ({ step, at: Date.now() }));
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  if (started.step !== step) setStarted({ step, at: now });
  return Math.max(0, Math.floor((now - started.at) / 1000));
}

function formatElapsed(totalSeconds: number): string {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m ${String(seconds).padStart(2, "0")}s` : `${seconds}s`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(0, Math.round(bytes / 1024))} KB`;
  const megabytes = bytes / (1024 * 1024);
  return megabytes >= 1024 ? `${(megabytes / 1024).toFixed(1)} GB` : `${Math.round(megabytes)} MB`;
}
