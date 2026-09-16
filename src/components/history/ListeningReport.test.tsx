import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ListeningReport } from "./ListeningReport";
import * as history from "../../history";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe("ListeningReport", () => {
  it("keeps the displayed report and its axes together while loading, and ignores an older response", async () => {
    const load = vi.spyOn(history, "loadHistoryReport");
    const { container } = render(<ListeningReport devices={[]} deviceId={null} onDeviceChange={vi.fn()} onPlayTrack={vi.fn()} onOpenArtistAlbums={vi.fn()} />);
    await screen.findByRole("heading", { name: "Top music" });
    const original = await load.mock.results[0].value;
    const originalLabel = container.querySelector(".report-content")?.getAttribute("aria-label");
    const originalAxis = container.querySelector(".report-hero__chart")?.textContent;
    const pending: Array<(report: history.HistoryReport) => void> = [];
    load.mockImplementation(() => new Promise((resolve) => pending.push(resolve)));

    fireEvent.click(screen.getByRole("button", { name: "30 days" }));
    await waitFor(() => expect(pending).toHaveLength(1));
    expect(screen.getByRole("button", { name: "30 days" })).toHaveAttribute("aria-pressed", "true");
    expect(container.querySelector(".listening-report")).toHaveAttribute("aria-busy", "true");
    expect(container.querySelector(".report-content")).toHaveAttribute("aria-label", originalLabel);
    expect(container.querySelector(".report-hero__chart")?.textContent).toBe(originalAxis);
    expect(screen.getByText(/Updating listening report/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "90 days" }));
    await waitFor(() => expect(pending).toHaveLength(2));
    await act(async () => pending[1]({ ...original, summary: { ...original.summary, plays: 900 } }));
    expect(container.querySelector(".report-hero__copy p")).toHaveTextContent("900 registered plays");
    expect(container.querySelector(".listening-report")).toHaveAttribute("aria-busy", "false");
    expect(container.querySelector(".report-content")?.getAttribute("aria-label")).not.toBe(originalLabel);
    await act(async () => pending[0]({ ...original, summary: { ...original.summary, plays: 300 } }));
    expect(container.querySelector(".report-hero__copy p")).toHaveTextContent("900 registered plays");
  });

  it("keeps the previous report and period controls available after an error, and retries", async () => {
    const load = vi.spyOn(history, "loadHistoryReport");
    render(<ListeningReport devices={[]} deviceId={null} onDeviceChange={vi.fn()} onPlayTrack={vi.fn()} onOpenArtistAlbums={vi.fn()} />);
    await screen.findByRole("heading", { name: "Top music" });
    const original = await load.mock.results[0].value;
    load.mockRejectedValueOnce(new Error("History temporarily unavailable"));
    fireEvent.click(screen.getByRole("button", { name: "30 days" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("History temporarily unavailable");
    expect(screen.getByRole("heading", { name: "Top music" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "90 days" })).toBeEnabled();
    load.mockResolvedValueOnce(original);
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    await waitFor(() => expect(document.querySelector(".listening-report")).toHaveAttribute("aria-busy", "false"));
  });

  it("shows artist portrait slots and album artwork in top track rows", async () => {
    const { container } = render(
      <ListeningReport
        devices={[]}
        deviceId={null}
        onDeviceChange={vi.fn()}
        onPlayTrack={vi.fn()}
        onOpenArtistAlbums={vi.fn()}
      />,
    );

    await screen.findByRole("heading", { name: "Top music" });
    const columns = container.querySelectorAll(".report-top__columns > article");
    expect(columns[0].querySelectorAll(".artist-portrait").length).toBeGreaterThan(0);
    expect(columns[2].querySelectorAll(".artwork").length).toBeGreaterThan(0);
    expect(columns[0].querySelector(".artist-smart-link")?.classList.contains("report-rank__play")).toBe(false);
    expect(columns[2].querySelector(".report-rank > .report-rank__play")).not.toBeNull();
  });

  it("renders up to five played genres with shares and aligned trend charts", async () => {
    const { container } = render(
      <ListeningReport
        devices={[]}
        deviceId={null}
        onDeviceChange={vi.fn()}
        onPlayTrack={vi.fn()}
        onOpenArtistAlbums={vi.fn()}
      />,
    );

    await screen.findByRole("heading", { name: "Genre trends" });
    const rows = container.querySelectorAll(".report-genres__row");
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThanOrEqual(5);
    expect(container.querySelectorAll(".report-genres__sparkline")).toHaveLength(rows.length);
    expect(container.querySelectorAll(".report-genres__share")).toHaveLength(rows.length);
    expect(screen.getByText(/Share is each genre's percentage of all registered plays/)).toBeInTheDocument();
  });
});
