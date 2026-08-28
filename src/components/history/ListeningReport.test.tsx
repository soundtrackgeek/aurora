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
      />,
    );

    await screen.findByRole("heading", { name: "Top music" });
    const columns = container.querySelectorAll(".report-top__columns > article");
    expect(columns[0].querySelectorAll(".artist-portrait").length).toBeGreaterThan(0);
    expect(columns[2].querySelectorAll(".artwork").length).toBeGreaterThan(0);
  });
});
