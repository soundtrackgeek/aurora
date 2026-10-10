import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { LibraryPlayExport } from "./LibraryPlayExport";
import { loadLibraryPlayExportStatus } from "../../history";

vi.mock("../../history", () => ({ loadLibraryPlayExportStatus: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("reports how many plays reached Music Library", async () => {
  vi.mocked(loadLibraryPlayExportStatus).mockResolvedValue({ sentPlays: 1234, lastExportedAtMs: 1_790_000_000_000, lastError: null });
  render(<LibraryPlayExport />);
  expect(await screen.findByRole("status")).toHaveTextContent(/plays sent/);
});

it("shows why plays are not being sent", async () => {
  vi.mocked(loadLibraryPlayExportStatus).mockResolvedValue({
    sentPlays: 0, lastExportedAtMs: null, lastError: "Update Music Library to 0.187.0 or later to receive Aurora plays.",
  });
  render(<LibraryPlayExport />);
  expect(await screen.findByRole("status")).toHaveTextContent("Update Music Library");
});
