import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { InlineLoveControl, InlineRatingControl } from "./InlineTagControls";

describe("Explore inline tag controls", () => {
  it("offers half-star hit areas and emits the clicked rating", () => {
    const onRatingChange = vi.fn();
    const onRowDoubleClick = vi.fn();
    render(
      <div onDoubleClick={onRowDoubleClick}>
        <InlineRatingControl
          title="Annabel"
          rating={3.5}
          busy={false}
          onRatingChange={onRatingChange}
        />
      </div>,
    );

    const target = screen.getByRole("button", { name: "Rate Annabel 4.5 stars" });
    fireEvent.click(target);
    fireEvent.doubleClick(target);
    expect(onRatingChange).toHaveBeenCalledWith(4.5);
    expect(onRowDoubleClick).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Rate Annabel 3.5 stars" })).toHaveAttribute("aria-pressed", "true");
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
      <InlineRatingControl
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
