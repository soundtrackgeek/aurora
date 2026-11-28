import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";

export type DiscTotalScanActivity = "starting" | "reading" | "checking" | "resting" | "paused";

export type DiscTotalScanStatus = {
  activity: DiscTotalScanActivity;
  readFiles: number;
  catalogFiles: number;
  filesPerMinute: number | null;
  error: string | null;
};

export async function loadDiscTotalScanStatus(): Promise<DiscTotalScanStatus | null> {
  if (!isTauriRuntime()) return null;
  return invoke("disc_total_scan_status");
}

const count = new Intl.NumberFormat("en-US");

export function formatRemaining(minutes: number): string {
  if (minutes < 1) return "less than a minute";
  if (minutes < 60) return `about ${Math.round(minutes)} min`;
  const hours = minutes / 60;
  return hours < 10 ? `about ${hours.toFixed(1)} h` : `about ${Math.round(hours)} h`;
}

/** One line describing the scan, for the Settings panel. */
export function describeDiscTotalScan(status: DiscTotalScanStatus): string {
  const read = `${count.format(status.readFiles)} of ${count.format(status.catalogFiles)} files read`;
  const left = status.catalogFiles - status.readFiles;
  switch (status.activity) {
    case "paused":
      return `Paused in Network or Laptop Mode · ${read} on the home PC`;
    case "starting":
      return `Starting · ${read}`;
    case "checking":
      return `Checking files for changes · ${read}`;
    case "resting":
      return left > 0 ? `Up to date · ${read} · ${count.format(left)} could not be read` : `Up to date · ${read}`;
    case "reading": {
      const eta = status.filesPerMinute && left > 0 ? ` · ${formatRemaining(left / status.filesPerMinute)} left` : "";
      return `Reading tags · ${read}${eta}`;
    }
  }
}
