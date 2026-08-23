import {
  Album,
  CalendarDays,
  Building2,
  ChartColumn,
  ChevronDown,
  ChevronRight,
  Clock3,
  Disc3,
  LibraryBig,
  ListMusic,
  Music2,
  Sparkles,
  Star,
  Tags,
  Telescope,
  UsersRound,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { LeftSidebarMode } from "../../layoutPreferences";

export type SidebarDestination =
  | "Universe"
  | "Observatory"
  | "Songs"
  | "Albums"
  | "Artists"
  | "Publishers"
  | "Genres"
  | "Years"
  | "Ratings"
  | "Tags"
  | "Charts"
  | "History";

type NavigationItem = {
  label: SidebarDestination;
  icon: LucideIcon;
};

type PreviewPlaylist = {
  label: string;
  description: string;
  icon: LucideIcon;
};

type SidebarNavigationProps = {
  activeDestination: SidebarDestination;
  sidebarMode: LeftSidebarMode;
  libraryExpanded: boolean;
  playlistsExpanded: boolean;
  onLibraryExpandedChange: (expanded: boolean) => void;
  onPlaylistsExpandedChange: (expanded: boolean) => void;
  onNavigate: (destination: SidebarDestination) => void;
};

const primaryItems: readonly NavigationItem[] = [
  { label: "Universe", icon: Sparkles },
  { label: "Observatory", icon: Telescope },
];

const libraryItems: readonly NavigationItem[] = [
  { label: "Songs", icon: Music2 },
  { label: "Albums", icon: Album },
  { label: "Artists", icon: UsersRound },
  { label: "Publishers", icon: Building2 },
  { label: "Genres", icon: Disc3 },
  { label: "Years", icon: CalendarDays },
  { label: "Ratings", icon: Star },
  { label: "Tags", icon: Tags },
];

const previewPlaylists: readonly PreviewPlaylist[] = [
  { label: "5 Star Collection", description: "rating view", icon: Star },
  { label: "Night Drive", description: "smart playlist", icon: Music2 },
  { label: "Unplayed", description: "listening queue", icon: Disc3 },
];

function destinationButtonClass(active: boolean, child = false) {
  return `${active ? "is-active" : ""}${child ? " nav-item--child" : ""}`.trim() || undefined;
}

export function SidebarNavigation({
  activeDestination,
  sidebarMode,
  libraryExpanded,
  playlistsExpanded,
  onLibraryExpandedChange,
  onPlaylistsExpandedChange,
  onNavigate,
}: SidebarNavigationProps) {
  const [openFlyout, setOpenFlyout] = useState<"library" | "playlists" | null>(null);
  const navigationRef = useRef<HTMLElement>(null);
  const libraryActive = libraryItems.some(({ label }) => label === activeDestination);

  useEffect(() => {
    if (!openFlyout || sidebarMode !== "icons") return;

    function closeOnOutsidePointer(event: PointerEvent) {
      if (!navigationRef.current?.contains(event.target as Node)) setOpenFlyout(null);
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") setOpenFlyout(null);
    }

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [openFlyout, sidebarMode]);

  function navigate(destination: SidebarDestination) {
    setOpenFlyout(null);
    onNavigate(destination);
  }

  function toggleLibrary() {
    if (sidebarMode === "icons") {
      setOpenFlyout((current) => current === "library" ? null : "library");
      return;
    }
    const expanded = !libraryExpanded;
    onLibraryExpandedChange(expanded);
    if (expanded) navigate("Songs");
  }

  function togglePlaylists() {
    if (sidebarMode === "icons") {
      setOpenFlyout((current) => current === "playlists" ? null : "playlists");
      return;
    }
    onPlaylistsExpandedChange(!playlistsExpanded);
  }

  return (
    <nav ref={navigationRef} className="primary-nav" aria-label="Primary">
      {primaryItems.map(({ label, icon: Icon }) => (
        <button
          type="button"
          key={label}
          className={destinationButtonClass(activeDestination === label)}
          onClick={() => navigate(label)}
          aria-current={activeDestination === label ? "page" : undefined}
          aria-label={sidebarMode === "icons" ? label : undefined}
          title={sidebarMode === "icons" ? label : undefined}
        >
          <Icon aria-hidden="true" />
          <span>{label}</span>
          {activeDestination === label && <ChevronRight className="nav-chevron" aria-hidden="true" />}
        </button>
      ))}

      <div className="nav-group">
        <button
          type="button"
          className={`nav-group__trigger${libraryActive ? " is-current-group" : ""}`}
          onClick={toggleLibrary}
          aria-expanded={sidebarMode === "icons" ? openFlyout === "library" : libraryExpanded}
          aria-controls={sidebarMode === "icons" ? "library-flyout" : "library-navigation"}
          aria-label={sidebarMode === "icons" ? "Library" : undefined}
          title={sidebarMode === "icons" ? "Library" : undefined}
        >
          <LibraryBig aria-hidden="true" />
          <span>Library</span>
          <ChevronDown className="nav-group__chevron" aria-hidden="true" />
        </button>

        {sidebarMode !== "icons" && libraryExpanded && (
          <div id="library-navigation" className="nav-group__children">
            {libraryItems.map(({ label, icon: Icon }) => (
              <button
                type="button"
                key={label}
                className={destinationButtonClass(activeDestination === label, true)}
                onClick={() => navigate(label)}
                aria-current={activeDestination === label ? "page" : undefined}
              >
                <Icon aria-hidden="true" />
                <span>{label}</span>
                {activeDestination === label && <ChevronRight className="nav-chevron" aria-hidden="true" />}
              </button>
            ))}
          </div>
        )}

        {sidebarMode === "icons" && openFlyout === "library" && (
          <div id="library-flyout" className="nav-flyout" aria-label="Library navigation">
            <p><LibraryBig aria-hidden="true" /> Library</p>
            {libraryItems.map(({ label, icon: Icon }) => (
              <button
                type="button"
                key={label}
                className={activeDestination === label ? "is-active" : undefined}
                onClick={() => navigate(label)}
                aria-current={activeDestination === label ? "page" : undefined}
              >
                <Icon aria-hidden="true" />
                <span>{label}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="nav-group nav-group--playlists">
        <button
          type="button"
          className="nav-group__trigger"
          onClick={togglePlaylists}
          aria-expanded={sidebarMode === "icons" ? openFlyout === "playlists" : playlistsExpanded}
          aria-controls={sidebarMode === "icons" ? "playlists-flyout" : "playlist-navigation"}
          aria-label={sidebarMode === "icons" ? "Playlists" : undefined}
          title={sidebarMode === "icons" ? "Playlists" : undefined}
        >
          <ListMusic aria-hidden="true" />
          <span>Playlists</span>
          <ChevronDown className="nav-group__chevron" aria-hidden="true" />
        </button>

        {sidebarMode !== "icons" && playlistsExpanded && (
          <div id="playlist-navigation" className="playlists nav-group__children">
            {previewPlaylists.map(({ label, description, icon: Icon }) => (
              <button type="button" key={label} disabled>
                <Icon aria-hidden="true" />
                <span><strong>{label}</strong><small>{description}</small></span>
              </button>
            ))}
          </div>
        )}

        {sidebarMode === "icons" && openFlyout === "playlists" && (
          <div id="playlists-flyout" className="nav-flyout nav-flyout--playlists" aria-label="Pinned playlists">
            <p><ListMusic aria-hidden="true" /> Pinned playlists</p>
            {previewPlaylists.map(({ label, description, icon: Icon }) => (
              <button type="button" key={label} disabled>
                <Icon aria-hidden="true" />
                <span><strong>{label}</strong><small>{description}</small></span>
              </button>
            ))}
            <small className="nav-flyout__note">Playlist management is coming in a focused release.</small>
          </div>
        )}
      </div>

      <button
        type="button"
        className={destinationButtonClass(activeDestination === "Charts")}
        onClick={() => navigate("Charts")}
        aria-current={activeDestination === "Charts" ? "page" : undefined}
        aria-label={sidebarMode === "icons" ? "Charts" : undefined}
        title={sidebarMode === "icons" ? "Charts" : undefined}
      >
        <ChartColumn aria-hidden="true" />
        <span>Charts</span>
        {activeDestination === "Charts" && <ChevronRight className="nav-chevron" aria-hidden="true" />}
      </button>

      <button
        type="button"
        className={destinationButtonClass(activeDestination === "History")}
        onClick={() => navigate("History")}
        aria-current={activeDestination === "History" ? "page" : undefined}
        aria-label={sidebarMode === "icons" ? "History" : undefined}
        title={sidebarMode === "icons" ? "History" : undefined}
      >
        <Clock3 aria-hidden="true" />
        <span>History</span>
        {activeDestination === "History" && <ChevronRight className="nav-chevron" aria-hidden="true" />}
      </button>
    </nav>
  );
}
