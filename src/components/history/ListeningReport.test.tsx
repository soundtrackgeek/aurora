import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ListeningReport } from "./ListeningReport";

afterEach(cleanup);

describe("ListeningReport", () => {
  it("shows artist portrait slots and album artwork in top track rows", async () => {
    const { container } = render(
      <ListeningReport
        devices={[]}
        deviceId={null}
        onDeviceChange={vi.fn()}
        onPlayTrack={vi.fn()}
        onOpenArtistAlbums={vi.fn()}
      />,
    );

    await screen.findByRole("heading", { name: "Top music" });
    const columns = container.querySelectorAll(".report-top__columns > article");
    expect(columns[0].querySelectorAll(".artist-portrait").length).toBeGreaterThan(0);
    expect(columns[2].querySelectorAll(".artwork").length).toBeGreaterThan(0);
    expect(columns[0].querySelector(".artist-smart-link")?.classList.contains("report-rank__play")).toBe(false);
    expect(columns[2].querySelector(".report-rank > .report-rank__play")).not.toBeNull();
  });

  it("renders up to five played genres with shares and aligned trend charts", async () => {
    const { container } = render(
      <ListeningReport
        devices={[]}
        deviceId={null}
        onDeviceChange={vi.fn()}
        onPlayTrack={vi.fn()}
        onOpenArtistAlbums={vi.fn()}
      />,
    );

    await screen.findByRole("heading", { name: "Genre trends" });
    const rows = container.querySelectorAll(".report-genres__row");
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThanOrEqual(5);
    expect(container.querySelectorAll(".report-genres__sparkline")).toHaveLength(rows.length);
    expect(container.querySelectorAll(".report-genres__share")).toHaveLength(rows.length);
    expect(screen.getByText(/Share is each genre's percentage of all registered plays/)).toBeInTheDocument();
  });
});
