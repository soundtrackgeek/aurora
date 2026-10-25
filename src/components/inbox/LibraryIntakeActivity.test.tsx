import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LibraryIntakeActivity } from "./LibraryIntakeActivity";

afterEach(cleanup);

describe("LibraryIntakeActivity", () => {
  it("shows truthful transfer stage and byte progress", () => {
    render(<LibraryIntakeActivity
      mode="apply"
      targetLabel="Inbox"
      progress={{
        operation: "applyBatch",
        stage: "transferring",
        message: "Copied and verified 05 - Example.mp3",
        completedAlbums: 1,
        totalAlbums: 3,
        processedFiles: 18,
        totalFiles: 42,
        processedBytes: 256 * 1024 * 1024,
        totalBytes: 1024 * 1024 * 1024,
      }}
    />);

    expect(screen.getByText("Inbox: Copied and verified 05 - Example.mp3")).toBeInTheDocument();
    expect(screen.getByText("18 of 42 files · 256 MB of 1.0 GB")).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Album transfer progress" })).toHaveValue(25);
    expect(screen.getByText("Transfer")).toHaveClass("is-active");
  });

  const cataloging = {
    operation: "applyBatch", stage: "cataloging", message: "Updating the Music Library catalog.",
    completedAlbums: 0, totalAlbums: 1, processedFiles: 0, totalFiles: 0, processedBytes: 0, totalBytes: 0,
  };

  it("counts up while Music Library catalogs and says finer progress is unavailable", () => {
    vi.useFakeTimers();
    render(<LibraryIntakeActivity mode="apply" progress={cataloging} />);
    expect(screen.getByText("Catalog")).toHaveClass("is-active");
    expect(screen.getByText(/Sources remain recoverable/)).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(65_000); });
    expect(screen.getByText(/updating its catalog database \(1m 05s so far\)/)).toBeInTheDocument();
    expect(screen.getByText(/reports no finer progress/)).toBeInTheDocument();
    vi.useRealTimers();
  });

  it.each(["searchIndex", "statistics", "committing", "somethingNew"])("keeps the %s step under Catalog", (stage) => {
    render(<LibraryIntakeActivity mode="apply" progress={{ ...cataloging, stage }} />);
    expect(screen.getByText("Catalog")).toHaveClass("is-active");
    expect(screen.getByText("Validate")).toHaveClass("is-complete");
  });

  it("shows a separate refresh step once the bridge has finished", () => {
    vi.useFakeTimers();
    render(<LibraryIntakeActivity mode="apply" refreshing progress={{ ...cataloging, stage: "completed", message: "Albums moved, verified, and cataloged." }} />);
    expect(screen.getByText("Added to library. Refreshing Aurora's view of it.")).toBeInTheDocument();
    expect(screen.getByText("Refresh")).toHaveClass("is-active");
    expect(screen.getByText("Finish")).toHaveClass("is-complete");
    act(() => { vi.advanceTimersByTime(25_000); });
    expect(screen.getByText(/Still refreshing after 25s\. The album is already safely in your library/)).toBeInTheDocument();
    vi.useRealTimers();
  });

  it("explains that preview does not change files before companion progress arrives", () => {
    render(<LibraryIntakeActivity mode="preview" progress={null} />);

    expect(screen.getByText("Finding albums, reading tags, and checking catalog destinations.")).toBeInTheDocument();
    expect(screen.getByText("No files are changed during preview.")).toBeInTheDocument();
    expect(screen.getByText("Scan")).toHaveClass("is-active");
  });
});
