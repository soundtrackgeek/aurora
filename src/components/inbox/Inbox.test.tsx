import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Inbox } from "./Inbox";

afterEach(cleanup);

describe("Inbox", () => {
  it("keeps staged albums outside the library and opens Auto-Tagger from Ctrl+Shift+T", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByText("1 album outside the library")).toBeInTheDocument();
    expect(screen.getByText("2 issues")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "t", ctrlKey: true, shiftKey: true });

    expect(await screen.findByRole("dialog", { name: "Album Auto-Tagger" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByText("MusicBrainz").length).toBeGreaterThan(0));
    expect(screen.getAllByText("Discogs").length).toBeGreaterThan(0);
    expect(screen.getByLabelText("Release title 1")).toHaveValue("Memories Calling");
  });

  it("requires readiness before previewing a library move", async () => {
    render(<Inbox onOpenMetadataSettings={vi.fn()} onCatalogChanged={vi.fn()} />);

    await screen.findByRole("heading", { name: "Inbox" });
    fireEvent.change(screen.getByRole("combobox", { name: "Library destination" }), { target: { value: "general" } });

    expect(screen.getByRole("button", { name: "Preview move" })).toBeDisabled();
    expect(screen.getByText("Uses the same reviewed, preview-first flow as Add Music.")).toBeInTheDocument();
  });
});
