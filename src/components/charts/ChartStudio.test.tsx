import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChartStudio } from "./ChartStudio";
import * as charts from "../../charts";

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

function renderStudio() {
  const onSelectionChange = vi.fn();
  const onSelectTrack = vi.fn();
  const onPlayQueue = vi.fn(async () => true);
  const onOpenArtistAlbums = vi.fn();
  render(<ChartStudio onSelectionChange={onSelectionChange} onSelectTrack={onSelectTrack} onPlayQueue={onPlayQueue} onOpenArtistAlbums={onOpenArtistAlbums} />);
  return { onSelectionChange, onSelectTrack, onPlayQueue, onOpenArtistAlbums };
}

describe("ChartStudio", () => {
  it("restores the applied chart and selected entry after a fresh mount", async () => {
    const load = vi.spyOn(charts, "loadChartPage");
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.click(screen.getByRole("tab", { name: "Albums" }));
    await screen.findByRole("heading", { name: "Aurora Album Score · Summer 1985" });
    fireEvent.click(screen.getByRole("tab", { name: "VG Lista" }));
    await screen.findByRole("heading", { name: "VG Lista Albums · Summer 1985" });
    fireEvent.click(screen.getByRole("button", { name: "1985 full year" }));
    await screen.findByRole("heading", { name: "VG Lista Albums · 1985 year chart" });
    const rows = screen.getAllByRole("row");
    fireEvent.click(rows[2]);
    const title = rows[2].querySelector(".chart-row__identity strong")!.textContent!;
    const savedRequest = load.mock.calls[load.mock.calls.length - 1][0];
    cleanup();
    load.mockClear();
    const callbacks = renderStudio();
    await screen.findByRole("heading", { name: "VG Lista Albums · 1985 year chart" });
    expect(load).toHaveBeenCalledWith(savedRequest);
    expect(screen.getByRole("tab", { name: "Albums" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("row", { name: new RegExp(title) })).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(callbacks.onSelectionChange).toHaveBeenCalledWith(expect.objectContaining({ entry: expect.objectContaining({ title }) }), undefined));
  });

  it("retains the previous chart during rapid period changes and only displays the newest result", async () => {
    const load = vi.spyOn(charts, "loadChartPage");
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    const original = await load.mock.results[0].value;
    const pending: Array<{ request: charts.ChartPageRequest; resolve: (page: charts.ChartPage) => void }> = [];
    load.mockImplementation((request) => new Promise((resolve) => pending.push({ request, resolve })));

    fireEvent.click(screen.getByRole("tab", { name: "Period chart" }));
    await waitFor(() => expect(pending).toHaveLength(1));
    expect(screen.getByRole("heading", { name: "Official UK Singles Chart" })).toBeInTheDocument();
    expect(screen.getByText(/Updating chart/)).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Period chart" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("button", { name: "Next period" }));
    await waitFor(() => expect(pending).toHaveLength(2));
    await act(async () => pending[1].resolve({ ...original, request: pending[1].request, chartTitle: "Newest chart" }));
    expect(screen.getByRole("heading", { name: "Newest chart" })).toBeInTheDocument();
    expect(document.querySelector(".chart-studio")).toHaveAttribute("aria-busy", "false");
    await act(async () => pending[0].resolve({ ...original, request: pending[0].request, chartTitle: "Older chart" }));
    expect(screen.queryByRole("heading", { name: "Older chart" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Newest chart" })).toBeInTheDocument();
  });

  it("keeps a failed refresh recoverable without discarding the previous chart", async () => {
    const load = vi.spyOn(charts, "loadChartPage");
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    load.mockRejectedValueOnce(new Error("Chart temporarily unavailable"));
    fireEvent.click(screen.getByRole("tab", { name: "Period chart" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Chart temporarily unavailable");
    expect(screen.getByRole("heading", { name: "Official UK Singles Chart" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Official UK Singles · Summer 1985" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("ignores a selected entry's detail response after leaving Charts", async () => {
    let resolveDetail!: (detail: charts.ChartItemDetail) => void;
    vi.spyOn(charts, "loadChartItemDetail").mockImplementation(() => new Promise((resolve) => { resolveDetail = resolve; }));
    const callbacks = renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    cleanup();
    callbacks.onSelectionChange.mockClear();
    callbacks.onSelectTrack.mockClear();
    await act(async () => resolveDetail({ sourceRanks: [] }));
    expect(callbacks.onSelectionChange).not.toHaveBeenCalled();
    expect(callbacks.onSelectTrack).not.toHaveBeenCalled();
  });

  it("drops pending source details when the new chart is empty and removes weekly controls for an annual source", async () => {
    const load = vi.spyOn(charts, "loadChartPage");
    let resolveDetail!: (detail: charts.ChartItemDetail) => void;
    vi.spyOn(charts, "loadChartItemDetail").mockImplementation(() => new Promise((resolve) => { resolveDetail = resolve; }));
    const callbacks = renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    const original = await load.mock.results[0].value;
    load.mockImplementation(async (request) => ({ ...original, request, chartTitle: "Empty annual chart", entries: [], totalEntries: 0, annualOnly: true }));
    fireEvent.click(screen.getByRole("tab", { name: /Billboard/ }));
    expect(screen.getByRole("tab", { name: "Selected week" })).toBeDisabled();
    expect(document.querySelectorAll(".chart-calendar__weeks button")).toHaveLength(0);
    await screen.findByRole("heading", { name: "Empty annual chart" });
    expect(screen.getByRole("button", { name: "Play this chart" })).toBeDisabled();
    callbacks.onSelectionChange.mockClear();
    callbacks.onSelectTrack.mockClear();
    await act(async () => resolveDetail({ sourceRanks: [] }));
    expect(screen.queryByRole("region", { name: /Across the sources for/ })).not.toBeInTheDocument();
    expect(callbacks.onSelectionChange).not.toHaveBeenCalled();
    expect(callbacks.onSelectTrack).not.toHaveBeenCalled();
  });

  it("opens the selected historical week and matches its leading library track", async () => {
    const callbacks = renderStudio();

    expect(await screen.findByRole("heading", { name: "Official UK Singles Chart" })).toBeInTheDocument();
    expect(screen.getByText("You'll Never Walk Alone")).toBeInTheDocument();
    await waitFor(() => expect(callbacks.onSelectTrack).toHaveBeenCalled());
  });

  it("switches from the exact week into the calculated period chart", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });

    fireEvent.click(screen.getByRole("tab", { name: "Period chart" }));

    expect(await screen.findByRole("heading", { name: "Official UK Singles · Summer 1985" })).toBeInTheDocument();
    expect(screen.getByText(/ranked by position finishes/i)).toBeInTheDocument();
  });

  it("builds an end-of-year chart directly from the year control", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });

    fireEvent.click(screen.getByRole("button", { name: "1985 full year" }));

    expect(await screen.findByRole("heading", { name: "Official UK Singles · 1985 year chart" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Period chart" })).toHaveAttribute("aria-selected", "true");
  });

  it("makes Aurora Score a first-class album chart", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });

    fireEvent.click(screen.getByRole("tab", { name: "Albums" }));

    expect(await screen.findByRole("heading", { name: "Aurora Album Score · Summer 1985" })).toBeInTheDocument();
    expect(screen.getAllByText("Rocky IV").length).toBeGreaterThan(0);
    expect(screen.getByRole("tab", { name: /Aurora Score/i })).toHaveAttribute("aria-selected", "true");
  });

  it("uses Year for Aurora Score by default and offers Release Year", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });

    expect(screen.getByRole("button", { name: "Year" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("Rocky IV")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Release Year" }));

    expect(await screen.findByText("Kind of Blue")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Release Year" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByText("Rocky IV")).not.toBeInTheDocument();
  });

  it("accepts a custom week range in the full row", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.change(screen.getByLabelText("From Year"), { target: { value: "1995" } });
    fireEvent.change(screen.getByLabelText("To Year"), { target: { value: "1995" } });
    fireEvent.change(screen.getByLabelText("From Week"), { target: { value: "7" } });
    fireEvent.change(screen.getByLabelText("To Week"), { target: { value: "13" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply range" }));
    expect(await screen.findByRole("heading", { name: "Official UK Singles · 1995 W7 – 1995 W13" })).toBeInTheDocument();
  });

  it("selects winter across a year boundary from one preset picker", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.click(screen.getByRole("button", { name: /Preset period/ }));
    fireEvent.change(screen.getByLabelText("Preset year"), { target: { value: "1989" } });
    fireEvent.click(screen.getByRole("button", { name: /Winter/ }));
    expect(await screen.findByRole("heading", { name: "Official UK Singles · Winter 1989" })).toBeInTheDocument();
    expect(screen.getByText("1989–1990")).toBeInTheDocument();
  });

  it("rejects a backwards custom range", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.change(screen.getByLabelText("From Year"), { target: { value: "1990" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply range" }));
    expect(screen.getByRole("alert")).toHaveTextContent("chronological");
    expect(screen.getByRole("heading", { name: "Official UK Singles Chart" })).toBeInTheDocument();
  });

  it("preserves the selected entry and inspector mode during a catalog refresh", async () => {
    const onSelectionChange = vi.fn();
    const onSelectTrack = vi.fn();
    const onPlayQueue = vi.fn(async () => true);
    const { rerender } = render(
      <ChartStudio
        catalogRevision={0}
        onSelectionChange={onSelectionChange}
        onSelectTrack={onSelectTrack}
        onPlayQueue={onPlayQueue}
        onOpenArtistAlbums={vi.fn()}
      />,
    );
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });

    const selectedRow = screen.getByText("19").closest('[role="row"]');
    expect(selectedRow).not.toBeNull();
    fireEvent.click(selectedRow!);
    await waitFor(() => expect(onSelectTrack).toHaveBeenCalledWith(
      expect.objectContaining({ title: "19" }),
      undefined,
    ));
    onSelectionChange.mockClear();
    onSelectTrack.mockClear();

    rerender(
      <ChartStudio
        catalogRevision={1}
        onSelectionChange={onSelectionChange}
        onSelectTrack={onSelectTrack}
        onPlayQueue={onPlayQueue}
        onOpenArtistAlbums={vi.fn()}
      />,
    );

    await waitFor(() => expect(onSelectionChange).toHaveBeenCalledWith(
      expect.objectContaining({ entry: expect.objectContaining({ title: "19" }) }),
      { preserveInspector: true },
    ));
    expect(screen.getByText("19").closest('[role="row"]')).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(onSelectTrack).toHaveBeenCalledWith(
      expect.objectContaining({ title: "19" }),
      { preserveInspector: true },
    ));
  });
});
