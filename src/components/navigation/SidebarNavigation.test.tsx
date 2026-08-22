import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SidebarNavigation } from "./SidebarNavigation";

afterEach(cleanup);

type SidebarNavigationProps = Parameters<typeof SidebarNavigation>[0];

function props(overrides: Partial<SidebarNavigationProps> = {}): SidebarNavigationProps {
  return {
    activeDestination: "Universe",
    sidebarMode: "expanded" as const,
    libraryExpanded: false,
    playlistsExpanded: false,
    onLibraryExpandedChange: vi.fn(),
    onPlaylistsExpandedChange: vi.fn(),
    onNavigate: vi.fn(),
    ...overrides,
  };
}

describe("SidebarNavigation", () => {
  it("opens a collapsed Library group and navigates to Songs", () => {
    const value = props();
    render(<SidebarNavigation {...value} />);

    fireEvent.click(screen.getByRole("button", { name: "Library" }));

    expect(value.onLibraryExpandedChange).toHaveBeenCalledWith(true);
    expect(value.onNavigate).toHaveBeenCalledWith("Songs");
  });

  it("collapses an open Library group without changing the active page", () => {
    const value = props({ activeDestination: "Artists", libraryExpanded: true });
    render(<SidebarNavigation {...value} />);

    fireEvent.click(screen.getByRole("button", { name: "Library" }));

    expect(value.onLibraryExpandedChange).toHaveBeenCalledWith(false);
    expect(value.onNavigate).not.toHaveBeenCalled();
  });

  it("opens compact Library and Playlist flyouts in icon-only mode", () => {
    const value = props({ sidebarMode: "icons", activeDestination: "Genres" });
    render(<SidebarNavigation {...value} />);

    fireEvent.click(screen.getByRole("button", { name: "Library" }));
    expect(screen.getByLabelText("Library navigation")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Years" }));
    expect(value.onNavigate).toHaveBeenCalledWith("Years");
    expect(screen.queryByLabelText("Library navigation")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Playlists" }));
    expect(screen.getByLabelText("Pinned playlists")).toBeInTheDocument();
  });

  it("dismisses an icon-only flyout with Escape", () => {
    render(<SidebarNavigation {...props({ sidebarMode: "icons" })} />);
    fireEvent.click(screen.getByRole("button", { name: "Library" }));

    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByLabelText("Library navigation")).not.toBeInTheDocument();
  });

  it("marks the active nested destination as the current page", () => {
    render(<SidebarNavigation {...props({ activeDestination: "Ratings", libraryExpanded: true })} />);
    expect(screen.getByRole("button", { name: "Ratings" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "Library" })).toHaveAttribute("aria-expanded", "true");
  });
});
