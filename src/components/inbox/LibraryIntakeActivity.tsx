import { LoaderCircle } from "lucide-react";
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
] as const;

export function LibraryIntakeActivity({
  mode,
  progress,
  targetLabel,
}: {
  mode: IntakeMode;
  progress: LibraryIntakeProgress | null;
  targetLabel?: string;
}) {
  const stages = mode === "preview" ? previewStages : applyStages;
  const fallbackStage = mode === "preview" ? "scanning" : "validating";
  const activeStage = progress?.stage === "completed" ? stages[stages.length - 1][0] : progress?.stage ?? fallbackStage;
  const activeIndex = Math.max(0, stages.findIndex(([id]) => id === activeStage));
  const transferring = mode === "apply" && progress?.stage === "transferring";
  const transferPercent = transferring && progress.totalBytes > 0
    ? Math.min(100, Math.round((progress.processedBytes / progress.totalBytes) * 100))
    : null;
  const message = progress?.message ?? (mode === "preview"
    ? "Finding albums, reading tags, and checking catalog destinations."
    : "Confirming the reviewed plan before any files are moved.");

  return <section className="library-intake-activity" role="status" aria-live="polite">
    <header>
      <LoaderCircle className="is-spinning" aria-hidden="true" />
      <span><strong>{targetLabel ? `${targetLabel}: ` : ""}{message}</strong>
        {transferring && progress.totalFiles > 0
          ? <small>{progress.processedFiles} of {progress.totalFiles} files · {formatBytes(progress.processedBytes)} of {formatBytes(progress.totalBytes)}</small>
          : <small>{mode === "preview" ? "No files are changed during preview." : "Sources remain recoverable until the catalog commit is verified."}</small>}
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

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(0, Math.round(bytes / 1024))} KB`;
  const megabytes = bytes / (1024 * 1024);
  return megabytes >= 1024 ? `${(megabytes / 1024).toFixed(1)} GB` : `${Math.round(megabytes)} MB`;
}
