import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { HistoryArtistLink } from "./HistoryArtistLink";

afterEach(cleanup);

it("shows each same-name description and opens its specific profile", () => {
  const open = vi.fn();
  render(<>
    <HistoryArtistLink artist="The Kids" artistId="artist:one" artistDisambiguation="First group" onOpen={open} />
    <HistoryArtistLink artist="The Kids" artistId="artist:two" artistDisambiguation="Second group" onOpen={open} />
    <HistoryArtistLink artist="The Kids" artistId="artist:three" artistDisambiguation="Third group" onOpen={open} />
  </>);
  expect(screen.getByText("· Third group")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Open artist page for The Kids (Second group)" }));
  expect(open).toHaveBeenCalledWith("artist:two");
});

it("keeps unresolved plays visible without a misleading artist link", () => {
  render(<HistoryArtistLink artist="Creed" artistDisambiguation="Unresolved artist" onOpen={vi.fn()} />);
  expect(screen.getByText("Creed")).toBeVisible();
  expect(screen.getByText("· Unresolved artist")).toBeVisible();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
});
