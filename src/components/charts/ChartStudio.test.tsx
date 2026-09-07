import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChartStudio } from "./ChartStudio";

afterEach(cleanup);

function renderStudio() {
  const onSelectionChange = vi.fn();
  const onSelectTrack = vi.fn();
  const onPlayQueue = vi.fn(async () => true);
  const onOpenArtistAlbums = vi.fn();
  render(<ChartStudio onSelectionChange={onSelectionChange} onSelectTrack={onSelectTrack} onPlayQueue={onPlayQueue} onOpenArtistAlbums={onOpenArtistAlbums} />);
  return { onSelectionChange, onSelectTrack, onPlayQueue, onOpenArtistAlbums };
}

describe("ChartStudio", () => {
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
