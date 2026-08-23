import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChartStudio } from "./ChartStudio";

afterEach(cleanup);

function renderStudio() {
  const onSelectionChange = vi.fn();
  const onSelectTrack = vi.fn();
  const onPlayQueue = vi.fn(async () => true);
  render(<ChartStudio onSelectionChange={onSelectionChange} onSelectTrack={onSelectTrack} onPlayQueue={onPlayQueue} />);
  return { onSelectionChange, onSelectTrack, onPlayQueue };
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

  it("accepts a custom week range", async () => {
    renderStudio();
    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    fireEvent.change(screen.getByLabelText("Label"), { target: { value: "My 1995 run" } });
    const yearInputs = screen.getAllByLabelText("Year");
    const weekInputs = screen.getAllByLabelText("Week");
    fireEvent.change(yearInputs[0], { target: { value: "1995" } });
    fireEvent.change(yearInputs[1], { target: { value: "1995" } });
    fireEvent.change(weekInputs[0], { target: { value: "7" } });
    fireEvent.change(weekInputs[1], { target: { value: "13" } });
    fireEvent.click(screen.getByRole("button", { name: /Apply period/i }));

    await screen.findByRole("heading", { name: "Official UK Singles Chart" });
    expect(screen.getByRole("button", { name: "My 1995 run" })).toHaveClass("is-active");
    expect(screen.getByText("1995")).toBeInTheDocument();
  });
});
