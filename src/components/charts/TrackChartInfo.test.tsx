import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadTrackChartPeaks, type TrackChartPeak } from "../../charts";
import { TrackChartInfo } from "./TrackChartInfo";

vi.mock("../../charts", () => ({ loadTrackChartPeaks: vi.fn() }));
const peaks: TrackChartPeak[] = [
  { label: "Official UK", peak: 3, country: "UK" },
  { label: "Ti i Skuddet", peak: 14, country: "NO" },
  { label: "Mainstream Rock Tracks", peak: 53, country: "US" },
  { label: "Radio Songs", peak: 1, country: "US" },
];

describe("TrackChartInfo", () => {
  beforeEach(() => vi.resetAllMocks());
  afterEach(cleanup);
  it("shows full names and peaks and filters each country without reloading", async () => {
    vi.mocked(loadTrackChartPeaks).mockResolvedValue(peaks);
    render(<TrackChartInfo artist="Artist" title="Song" />);
    expect(await screen.findByText("Mainstream Rock Tracks")).toBeInTheDocument();
    expect(screen.getByText("#53")).toBeInTheDocument();
    for (const [country, visible, hidden] of [["US", "Radio Songs", "Official UK"], ["UK", "Official UK", "Radio Songs"], ["NO", "Ti i Skuddet", "Official UK"]]) {
      fireEvent.click(screen.getByRole("button", { name: country }));
      expect(screen.getByText(visible)).toBeInTheDocument();
      expect(screen.queryByText(hidden)).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: country })).toHaveAttribute("aria-pressed", "true");
    }
    fireEvent.click(screen.getByRole("button", { name: "ALL" }));
    expect(screen.getByText("Official UK")).toBeInTheDocument();
    expect(loadTrackChartPeaks).toHaveBeenCalledTimes(1);
  });
  it("discards an older track request that finishes after a new selection", async () => {
    let finishOld!: (value: TrackChartPeak[]) => void;
    vi.mocked(loadTrackChartPeaks).mockImplementationOnce(() => new Promise((resolve) => { finishOld = resolve; })).mockResolvedValueOnce([peaks[3]]);
    const view = render(<TrackChartInfo artist="Artist" title="Old song" />);
    view.rerender(<TrackChartInfo artist="Artist" title="New song" />);
    await screen.findByText("Radio Songs");
    await act(async () => finishOld([peaks[0]]));
    expect(screen.queryByText("Official UK")).not.toBeInTheDocument();
    expect(screen.getByText("Radio Songs")).toBeInTheDocument();
  });
  it("distinguishes empty data from a failed lookup", async () => {
    vi.mocked(loadTrackChartPeaks).mockResolvedValueOnce([]).mockRejectedValueOnce(new Error("offline"));
    const view = render(<TrackChartInfo artist="Artist" title="Missing" />);
    await screen.findByText("No chart appearances found.");
    view.rerender(<TrackChartInfo artist="Artist" title="Unavailable" />);
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Chart info could not be loaded."));
  });
});
