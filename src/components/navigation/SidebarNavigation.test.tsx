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
    playlists: [],
    selectedPlaylistId: null,
    playlistsLoading: false,
    playlistsError: null,
    onLibraryExpandedChange: vi.fn(),
    onPlaylistsExpandedChange: vi.fn(),
    onNavigate: vi.fn(),
    onSelectPlaylist: vi.fn(),
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
    expect(screen.getByLabelText("Music Library playlists")).toBeInTheDocument();
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

  it("places Publishers inside the Library group", () => {
    const value = props({ activeDestination: "Publishers", libraryExpanded: true });
    render(<SidebarNavigation {...value} />);

    const publishers = screen.getByRole("button", { name: "Publishers" });
    expect(publishers).toHaveAttribute("aria-current", "page");
    fireEvent.click(publishers);
    expect(value.onNavigate).toHaveBeenCalledWith("Publishers");
  });

  it("places Inbox between Universe and Observatory", () => {
    render(<SidebarNavigation {...props({ activeDestination: "Inbox" })} />);
    const universe = screen.getByRole("button", { name: "Universe" });
    const inbox = screen.getByRole("button", { name: "Inbox" });
    const observatory = screen.getByRole("button", { name: "Observatory" });

    expect(universe.compareDocumentPosition(inbox) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(inbox.compareDocumentPosition(observatory) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(inbox).toHaveAttribute("aria-current", "page");
  });

  it("places Charts directly above History as its own destination", () => {
    const value = props({ activeDestination: "Charts" });
    render(<SidebarNavigation {...value} />);
    const charts = screen.getByRole("button", { name: "Charts" });
    const history = screen.getByRole("button", { name: "History" });

    expect(charts).toHaveAttribute("aria-current", "page");
    expect(charts.compareDocumentPosition(history) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("shows real playlist names in both sidebar modes and opens the selected playlist", () => {
    const playlist = { id: 7, name: "All Loved", description: "Saved collection", trackCount: 1560, updatedAt: "2026-09-22" };
    const value = props({ playlists: [playlist], playlistsExpanded: true });
    const view = render(<SidebarNavigation {...value} />);
    fireEvent.click(screen.getByRole("button", { name: /All Loved/ }));
    expect(value.onSelectPlaylist).toHaveBeenCalledWith(7);
    view.rerender(<SidebarNavigation {...value} sidebarMode="icons" />);
    fireEvent.click(screen.getByRole("button", { name: "Playlists" }));
    fireEvent.click(screen.getByRole("button", { name: /All Loved/ }));
    expect(value.onSelectPlaylist).toHaveBeenCalledTimes(2);
  });
});
