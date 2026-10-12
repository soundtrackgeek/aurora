import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { InlineLoveControl, InlineRatingControl } from "./InlineTagControls";
import { ShortcutRatingSaveContext } from "../app/tags/ratingSaveContext";

afterEach(cleanup);

describe("Explore inline tag controls", () => {
  it("shows requested stars only on the saving track and restores confirmed stars on failure", () => {
    const control = <InlineRatingControl trackKey="test-track" title="Annabel" rating={2}
      busy={false} onRatingChange={vi.fn()} />;
    const save = { requestId: 1, trackKey: "test-track", rating: 4, pending: true };
    const { rerender } = render(<ShortcutRatingSaveContext value={{ "test-track": save }}>{control}</ShortcutRatingSaveContext>);
    const group = screen.getByRole("group", { name: "Rate Annabel" });
    expect(group).toHaveClass("is-saving");
    expect(group).toHaveAttribute("aria-busy", "true");
    expect(group).toHaveAttribute("aria-description", "Saving 4 star rating for Annabel…");
    expect(group.querySelectorAll("svg.is-filled")).toHaveLength(4);
    expect(screen.getByRole("button", { name: "Rate Annabel 4.0 stars" })).toBeDisabled();

    rerender(<ShortcutRatingSaveContext value={{ "other-track": save }}>{control}</ShortcutRatingSaveContext>);
    expect(group).not.toHaveClass("is-saving");
    expect(group.querySelectorAll("svg.is-filled")).toHaveLength(2);
    rerender(<ShortcutRatingSaveContext value={{ "test-track": { ...save, pending: false } }}>{control}</ShortcutRatingSaveContext>);
    expect(group).not.toHaveClass("is-saving");
    expect(group.querySelectorAll("svg.is-filled")).toHaveLength(2);
  });

  it("marks clearing a rating as saving while showing empty stars", () => {
    render(<ShortcutRatingSaveContext value={{ "test-track": { requestId: 2, trackKey: "test-track", rating: null, pending: true } }}>
      <InlineRatingControl trackKey="test-track" title="Annabel" rating={4} busy={false} onRatingChange={vi.fn()} />
    </ShortcutRatingSaveContext>);
    const group = screen.getByRole("group", { name: "Rate Annabel" });
    expect(group).toHaveAttribute("data-clearing", "true");
    expect(group).toHaveAttribute("aria-description", "Clearing rating for Annabel…");
    expect(group.querySelectorAll("svg.is-filled")).toHaveLength(0);
  });
  it("offers whole-star hit areas and emits the clicked rating", () => {
    const onRatingChange = vi.fn();
    const onRowDoubleClick = vi.fn();
    render(
      <div onDoubleClick={onRowDoubleClick}>
        <InlineRatingControl trackKey="test-track"
          title="Annabel"
          rating={3}
          busy={false}
          onRatingChange={onRatingChange}
        />
      </div>,
    );

    const target = screen.getByRole("button", { name: "Rate Annabel 4.0 stars" });
    fireEvent.click(target);
    fireEvent.doubleClick(target);
    expect(onRatingChange).toHaveBeenCalledWith(4);
    expect(onRowDoubleClick).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Rate Annabel 3.0 stars" })).toHaveAttribute("aria-pressed", "true");
  });

  it("toggles Love directly and exposes the busy state", () => {
    const onLoveChange = vi.fn();
    const { rerender } = render(
      <InlineLoveControl
        title="Annabel"
        loveState="neutral"
        busy={false}
        onLoveChange={onLoveChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Love Annabel" }));
    expect(onLoveChange).toHaveBeenCalledWith("loved");

    rerender(
      <InlineLoveControl
        title="Annabel"
        loveState="loved"
        busy
        onLoveChange={onLoveChange}
      />,
    );
    expect(screen.getByRole("button", { name: "Remove Love from Annabel" })).toBeDisabled();
  });

  it("can clear the current rating when enabled by the player", () => {
    const onRatingChange = vi.fn();
    render(
      <InlineRatingControl trackKey="test-track"
        title="Midnight City"
        rating={4}
        busy={false}
        allowClear
        onRatingChange={onRatingChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Clear rating for Midnight City" }));
    expect(onRatingChange).toHaveBeenCalledWith(null);
  });
});
