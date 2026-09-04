import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
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

  it("explains that preview does not change files before companion progress arrives", () => {
    render(<LibraryIntakeActivity mode="preview" progress={null} />);

    expect(screen.getByText("Finding albums, reading tags, and checking catalog destinations.")).toBeInTheDocument();
    expect(screen.getByText("No files are changed during preview.")).toBeInTheDocument();
    expect(screen.getByText("Scan")).toHaveClass("is-active");
  });
});
