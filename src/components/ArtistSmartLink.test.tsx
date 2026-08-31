import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { albumArtistSearchQuery } from "../artistSearch";
import { ArtistSmartLink } from "./ArtistSmartLink";

describe("ArtistSmartLink", () => {
  it("builds an exact album-artist query and escapes quoted names", () => {
    expect(albumArtistSearchQuery("  Steve Lynch ")).toBe('aartist:"Steve Lynch"');
    expect(albumArtistSearchQuery('"Weird Al" Yankovic')).toBe('aartist:"""Weird Al"" Yankovic"');
  });

  it("opens the clicked artist without activating a parent row", () => {
    const onOpen = vi.fn();
    const onParent = vi.fn();
    render(<div onClick={onParent}><ArtistSmartLink artist="M83" onOpen={onOpen} /></div>);

    fireEvent.click(screen.getByRole("button", { name: "Show albums by M83" }));
    expect(onOpen).toHaveBeenCalledWith("M83");
    expect(onParent).not.toHaveBeenCalled();
  });
});
