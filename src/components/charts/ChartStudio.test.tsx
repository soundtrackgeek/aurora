import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChartInspector, ChartStudio, type ChartSelectionContext } from "./ChartStudio";
import * as charts from "../../charts";
import * as library from "../../library";

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
  async function largeChart(totalEntries = 1105) {
    const original = await charts.loadChartPage({ kind: "singles", source: "officialUk", scope: "period", period: charts.chartPresets[0], selectedYear: 1985, selectedWeek: 23, yearBasis: "year", limit: 100 });
    const entries = Array.from({ length: totalEntries }, (_, index) => ({
      ...original.entries[0], position: index + 1, artist: index === totalEntries - 1 ? "Needle Artist" : "Test Artist", artistKey: `artist-${index + 1}`, titleKey: `song-${index + 1}`,
      title: `Song ${index + 1}`, loved: false, matchedTrackId: null, matchedAlbumId: null, artworkAlbumId: null,
    }));
    vi.spyOn(charts, "loadChartItemDetail").mockResolvedValue({ sourceRanks: [] });
    const load = vi.spyOn(charts, "loadChartPage").mockImplementation(async (request) => {
      const search = request.search?.trim().toLowerCase() ?? "";
      const matching = entries.filter((entry) => !search || entry.title.toLowerCase().includes(search) || entry.artist.toLowerCase().includes(search));
      return { ...original, request, entries: request.limit === 0 ? matching : matching.slice(0, request.limit), totalEntries: matching.length };
    });
    return load;
  }

  it("loads the full chart beyond 1000 while preserving selection", async () => {
    await largeChart();
    renderStudio();
    await screen.findByText("Showing 100 of 1,105 matching entries");
    fireEvent.click(screen.getByText("Song 90").closest('[role="row"]')!);
    fireEvent.click(screen.getByRole("button", { name: "Show full chart" }));
    await screen.findByText("Showing 1,105 of 1,105 matching entries");
    expect(screen.getByText("Song 1105")).toBeInTheDocument();
    expect(screen.getByText("Song 90").closest('[role="row"]')).toHaveAttribute("aria-selected", "true");
    expect(document.querySelectorAll(".chart-row")).toHaveLength(1105);
    expect(document.querySelector(".chart-show-more")).not.toBeInTheDocument();
  });

  it("remembers the full chart and can return to the top 100", async () => {
    const load = await largeChart(201);
    renderStudio();
    await screen.findByText("Showing 100 of 201 matching entries");
    fireEvent.click(screen.getByText("Song 90").closest('[role="row"]')!);
    fireEvent.click(screen.getByRole("button", { name: "Show full chart" }));
    await screen.findByText("Showing 201 of 201 matching entries");
    cleanup();
    renderStudio();
    await screen.findByText("Showing 201 of 201 matching entries");
    expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ limit: 0 }));
    expect(screen.getByText("Song 90").closest('[role="row"]')).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("button", { name: "Show top 100" }));
    await screen.findByText("Showing 100 of 201 matching entries");
    expect(screen.queryByText("Song 201")).not.toBeInTheDocument();
  });

  it("fetches the next 100 and keeps full-chart mode when changing kind or source", async () => {
    const load = await largeChart(621);
    renderStudio();
    await screen.findByText("Showing 100 of 621 matching entries");
    fireEvent.click(screen.getByRole("button", { name: "Show next 100 entries (521 remaining)" }));
    await screen.findByText("Showing 200 of 621 matching entries");
    expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ limit: 200 }));
    expect(screen.getByText("Song 101")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show full chart" }));
    await screen.findByText("Showing 621 of 621 matching entries");
    fireEvent.click(screen.getByRole("tab", { name: "Albums" }));
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "albums", source: "auroraScore", limit: 0 })));
    fireEvent.click(screen.getByRole("tab", { name: "VG Lista" }));
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "albums", source: "vgLista", limit: 0 })));
  });

  it("retains the initial 100 on a failed full-chart load and retries", async () => {
    const load = await largeChart(621);
    renderStudio();
    await screen.findByText("Showing 100 of 621 matching entries");
    load.mockRejectedValueOnce(new Error("Chart temporarily unavailable"));
    fireEvent.click(screen.getByRole("button", { name: "Show full chart" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Chart temporarily unavailable");
    expect(screen.getByText("Showing 100 of 621 matching entries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByText("Showing 621 of 621 matching entries");
  });

  it("live search finds songs and artists below the loaded 100 and restores after clearing", async () => {
    const load = await largeChart(621);
    renderStudio();
    await screen.findByText("Showing 100 of 621 matching entries");
    const search = screen.getByRole("searchbox", { name: "Find in this chart" });
    fireEvent.change(search, { target: { value: " SoNg 621 " } });
    await screen.findByText("Showing 1 of 1 matching entries");
    expect(screen.getByText("Song 621")).toBeInTheDocument();
    expect(document.querySelector(".chart-row__rank")).toHaveTextContent("621");
    expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ search: "SoNg 621", limit: 100 }));
    fireEvent.change(search, { target: { value: "needle artist" } });
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ search: "needle artist" })));
    await waitFor(() => expect(document.querySelector(".chart-studio")).toHaveAttribute("aria-busy", "false"));
    expect(screen.getByText("Song 621")).toBeInTheDocument();
    cleanup();
    renderStudio();
    await screen.findByText("Showing 1 of 1 matching entries");
    expect(screen.getByRole("searchbox", { name: "Find in this chart" })).toHaveValue("needle artist");
    fireEvent.change(screen.getByRole("searchbox", { name: "Find in this chart" }), { target: { value: "Missing song" } });
    await screen.findByText(/No chart entries match/);
    fireEvent.change(screen.getByRole("searchbox", { name: "Find in this chart" }), { target: { value: "" } });
    await screen.findByText("Showing 100 of 621 matching entries");
  });

  it("filters library matches, preserves ranks, and remembers the choice", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    const count = document.querySelectorAll(".chart-row").length;
    const ranks = Array.from(document.querySelectorAll(".chart-row__rank"), (rank) => rank.textContent);
    expect(screen.getByRole("combobox", { name: "Library" })).toHaveValue("");
    fireEvent.change(screen.getByRole("combobox", { name: "Library" }), { target: { value: "notInLibrary" } });
    await waitFor(() => expect(screen.queryByLabelText("In your library")).not.toBeInTheDocument());
    expect(screen.getByText(/No chart entries match/)).toBeInTheDocument();
    cleanup();
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    expect(screen.getByRole("combobox", { name: "Library" })).toHaveValue("notInLibrary");
    fireEvent.change(screen.getByRole("combobox", { name: "Library" }), { target: { value: "inLibrary" } });
    expect((await screen.findAllByLabelText("In your library")).length).toBeGreaterThan(0);
    expect(screen.queryByLabelText("Not matched")).not.toBeInTheDocument();
    expect(Array.from(document.querySelectorAll(".chart-row__rank"), (rank) => rank.textContent)).toEqual(ranks);
    fireEvent.change(screen.getByRole("combobox", { name: "Library" }), { target: { value: "" } });
    await waitFor(() => expect(document.querySelectorAll(".chart-row")).toHaveLength(count));
  });

  it("adds the library checkmark and playback after an unmatched song resolves on catalog refresh", async () => {
    const loadPage = charts.loadChartPage;
    let corrected = false;
    vi.spyOn(charts, "loadChartPage").mockImplementation(async (request) => {
      const page = await loadPage(request);
      const entry = page.entries.find((entry) => entry.matchedTrackId)!;
      return { ...page, entries: [{ ...entry, title: "Stay", artist: "Shakespears Sister",
        artistKey: "shakespears sister", titleKey: "stay",
        matchedTrackId: corrected ? entry.matchedTrackId : null,
        matchedAlbumId: null,
      }], totalEntries: 1 };
    });
    const onSelectionChange = vi.fn();
    const props = { onSelectionChange, onSelectTrack: vi.fn(), onPlayQueue: vi.fn(async () => true), onOpenArtistAlbums: vi.fn() };
    const { rerender } = render(<ChartStudio {...props} catalogRevision={0} />);
    expect(await screen.findByLabelText("Not matched")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Play Stay" })).not.toBeInTheDocument();
    corrected = true;
    rerender(<ChartStudio {...props} catalogRevision={1} />);
    expect(await screen.findByLabelText("In your library")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Play Stay" })).toBeInTheDocument();
    expect(screen.getByText("Stay").closest('[role="row"]')).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(onSelectionChange).toHaveBeenCalledWith(
      expect.objectContaining({ entry: expect.objectContaining({ title: "Stay", matchedTrackId: expect.any(String) }) }),
      { preserveInspector: true },
    ));
  });

  it("explains a missing weekly archive without leaving a loading spinner", async () => {
    vi.spyOn(charts, "loadPublishedChartSeries").mockRejectedValue(new Error("Open Published Charts in Music Library"));
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.click(screen.getByRole("tab", { name: "US weekly" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Open Published Charts in Music Library");
    expect(document.querySelector(".chart-studio")).toHaveAttribute("aria-busy", "false");
    expect(screen.queryByText("Updating chart… Previous results remain visible.")).not.toBeInTheDocument();
  });

  it("chooses a US series, year and exact date and remembers them", async () => {
    const load = vi.spyOn(charts, "loadChartPage");
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.click(screen.getByRole("tab", { name: "US weekly" }));
    await screen.findByRole("heading", { name: "Billboard Hot 100" });
    fireEvent.change(screen.getByRole("combobox", { name: "US chart" }), { target: { value: "Country Singles Chart" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Chart year" }), { target: { value: "1993" } });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Week ending" })).toHaveValue("1993-01-05"));
    fireEvent.click(screen.getByRole("button", { name: "Next published week" }));
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ source: "publishedUs", publishedChart: "Country Singles Chart", selectedYear: 1993, publishedWeek: "1993-01-12" })));
    expect(JSON.parse(localStorage.getItem("aurora:charts:v1")!).request.publishedWeek).toBe("1993-01-12");
    fireEvent.click(screen.getByRole("tab", { name: "Year chart" }));
    expect(await screen.findByRole("heading", { name: "Country Singles Chart · 1993 year chart" })).toBeVisible();
  });

  it("opens the selected track's real album with its year", async () => {
    const callbacks = renderStudio();
    await waitFor(() => expect(callbacks.onSelectTrack).toHaveBeenCalled());
    const selection = callbacks.onSelectionChange.mock.calls[callbacks.onSelectionChange.mock.calls.length - 1][0] as ChartSelectionContext;
    const track = callbacks.onSelectTrack.mock.calls[callbacks.onSelectTrack.mock.calls.length - 1][0] as library.Track;
    const onOpenAlbum = vi.fn();
    render(<ChartInspector selection={selection} track={track} busy={false} onPlay={vi.fn()} onOpenLibrary={vi.fn()} onOpenAlbum={onOpenAlbum} onOpenArtistAlbums={vi.fn()} onRatingChange={vi.fn()} onLoveChange={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: `${track.album} (${track.originalYear ?? track.releaseYear})` }));
    expect(onOpenAlbum).toHaveBeenCalledWith(track);
  });

  it("shows rated-track completeness from the selected library album", async () => {
    const album = (await library.exploreAlbums({})).items[0];
    const loadAlbum = vi.spyOn(library, "loadAlbumDetail").mockResolvedValue({
      album: { ...album, ratedTracks: 3, totalTracks: 4 },
      tracks: [], tracksTruncated: false, popularity: { tracks: [] },
    });
    const callbacks = renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.click(screen.getByRole("tab", { name: "Albums" }));
    await waitFor(() => expect(callbacks.onSelectionChange).toHaveBeenCalledWith(
      expect.objectContaining({ albumRatingProgress: expect.objectContaining({ ratedTracks: 3, totalTracks: 4 }) }), undefined,
    ));
    const selection = callbacks.onSelectionChange.mock.calls[callbacks.onSelectionChange.mock.calls.length - 1][0] as ChartSelectionContext;
    expect(loadAlbum).toHaveBeenCalledWith(selection.entry.matchedAlbumId, { localOnly: true });
    render(<ChartInspector selection={selection} track={null} busy={false} onPlay={vi.fn()} onOpenLibrary={vi.fn()} onOpenArtistAlbums={vi.fn()} onRatingChange={vi.fn()} onLoveChange={vi.fn()} />);
    expect(screen.getByText("Rating Completeness")).toBeVisible();
    expect(screen.getByText("75% (3/4)")).toBeVisible();
  });

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
