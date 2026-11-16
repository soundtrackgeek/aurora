import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import * as assistant from "../assistant";
import { AssistantProposalsDialog } from "./AssistantProposalsDialog";

const album = (albumId: string, title: string, tags: string[], proposedGenre: string): assistant.GenreProposalAlbum => ({
  albumId, artist: "Artist", title, year: 1985, currentTags: tags.map((tag) => ({ tag, tracks: 4 })),
  proposedGenre, appliedGenre: null, status: "pending", error: null,
});

const proposal: assistant.GenreProposal = {
  id: "p1",
  createdAtMs: 1_800_000_000_000,
  source: "claude-code",
  reason: "One genre per album.",
  status: "pending",
  albums: [album("a", "First", ["Electro; Dance"], "Electro"), album("b", "Second", ["Rock", "Pop"], "Rock")],
};

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) { this.open = true; });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

function renderDialog(proposals = [proposal]) {
  const handlers = { onProposalChange: vi.fn(), onClear: vi.fn(), onApplied: vi.fn(), onClose: vi.fn() };
  render(<AssistantProposalsDialog proposals={proposals} {...handlers} />);
  return handlers;
}

describe("AssistantProposalsDialog", () => {
  it("shows where a proposal came from and marks tags that hold several genres", () => {
    renderDialog();
    expect(screen.getByText("Claude Code")).toBeTruthy();
    expect(screen.getByText("One genre per album.")).toBeTruthy();
    expect(screen.getByText("Electro; Dance").closest("[data-several]")).toBeTruthy();
    expect(screen.getByText("Rock").closest("[data-several]")).toBeNull();
    expect(screen.getByRole("button", { name: "Write genre to 2 albums" })).toBeTruthy();
  });

  it("writes only the checked albums, with the genre as edited", async () => {
    const applied: assistant.GenreProposal = {
      ...proposal,
      albums: [{ ...proposal.albums[0], status: "applied", appliedGenre: "Synthpop" }, { ...proposal.albums[1], status: "pending" }],
    };
    const apply = vi.spyOn(assistant, "applyGenreProposal").mockResolvedValue(applied);
    const handlers = renderDialog();
    fireEvent.click(screen.getByRole("checkbox", { name: "Change Artist — Second (1985)" }));
    fireEvent.change(screen.getByRole("textbox", { name: "New genre for Artist — First (1985)" }), { target: { value: " Synthpop " } });
    fireEvent.click(screen.getByRole("button", { name: "Write genre to 1 album" }));
    await waitFor(() => expect(handlers.onApplied).toHaveBeenCalledWith(1));
    expect(apply).toHaveBeenCalledWith("p1", [{ albumId: "a", genre: "Synthpop" }]);
    expect(handlers.onProposalChange).toHaveBeenCalledWith(applied);
  });

  it("will not write an empty genre and shows apply errors", async () => {
    vi.spyOn(assistant, "applyGenreProposal").mockRejectedValue(new Error("Music writes are off."));
    renderDialog();
    const genre = screen.getByRole("textbox", { name: "New genre for Artist — First (1985)" });
    fireEvent.change(genre, { target: { value: "  " } });
    expect((screen.getByRole("button", { name: "Write genre to 2 albums" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(genre, { target: { value: "Electro" } });
    fireEvent.click(screen.getByRole("button", { name: "Write genre to 2 albums" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Music writes are off.");
  });

  it("dismisses a proposal and clears a finished one", async () => {
    const dismissed = { ...proposal, status: "dismissed" as const };
    vi.spyOn(assistant, "dismissGenreProposal").mockResolvedValue(dismissed);
    const handlers = renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(handlers.onProposalChange).toHaveBeenCalledWith(dismissed));
    cleanup();

    const done: assistant.GenreProposal = { ...proposal, status: "done", albums: proposal.albums.map((item) => ({ ...item, status: "applied", appliedGenre: item.proposedGenre })) };
    const next = renderDialog([done]);
    expect(screen.getByText("2 changed")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(next.onClear).toHaveBeenCalledWith("p1");
  });

  it("shows the genre check's evidence and leaves medium-confidence albums unticked", () => {
    const checked: assistant.GenreProposal = {
      ...proposal,
      source: "genre-check",
      albums: [
        { ...album("a", "First", ["Synthwave"], "Hard Rock"), evidence: { kind: "retag", confidence: "high", summary: "Sounds more like Hard Rock than Synthwave.", tracks: [{ trackId: "7", title: "Lucy", artist: "Y&T", album: "Ten", year: 1990 }] } },
        { ...album("b", "Second", ["Rock"], "Hard Rock"), evidence: { kind: "refine", confidence: "medium", summary: "Rock is a broad genre.", tracks: [] } },
      ],
    };
    renderDialog([checked]);
    expect(screen.getByText("Genre check by sound")).toBeTruthy();
    expect(screen.getByText("High confidence · sounds like another genre")).toBeTruthy();
    expect(screen.getByText("Medium confidence · more specific genre")).toBeTruthy();
    expect(screen.getByText("Y&T — Lucy (Ten, 1990)")).toBeTruthy();
    expect((screen.getByRole("checkbox", { name: "Change Artist — First (1985)" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: "Change Artist — Second (1985)" }) as HTMLInputElement).checked).toBe(false);
    expect(screen.getByRole("button", { name: "Write genre to 1 album" })).toBeTruthy();
  });

  it("hides dismissed proposals", () => {
    renderDialog([{ ...proposal, status: "dismissed" }]);
    expect(screen.getByText(/No changes are waiting/)).toBeTruthy();
  });
});

describe("proposal helpers", () => {
  it("replaces a proposal by id or adds it", () => {
    const updated = { ...proposal, status: "done" as const };
    expect(assistant.upsertProposal([proposal], updated)).toEqual([updated]);
    expect(assistant.upsertProposal([proposal], { ...proposal, id: "p2" })).toHaveLength(2);
  });

  it("sums up a genre check by sound", () => {
    const result = (flagged: number, rest: Partial<assistant.SoundCheck> = {}): assistant.SoundCheck => ({
      proposal: flagged ? { ...proposal, albums: proposal.albums.slice(0, flagged) } : null,
      checked: 0, fitting: 0, unanalyzed: 0, notChecked: 0, ...rest,
    });
    expect(assistant.soundCheckSummary(result(2, { checked: 70000, fitting: 69998 }), false)).toBe("2 albums to review");
    expect(assistant.soundCheckSummary(result(0, { checked: 70000, fitting: 70000 }), false)).toBe("Every album sounds like its genre");
    expect(assistant.soundCheckSummary(result(1, { checked: 1 }), true)).toBe("A different genre fits its sound better. Opened for review.");
    expect(assistant.soundCheckSummary(result(0, { checked: 1, fitting: 1 }), true)).toBe("Sounds like its genre.");
    expect(assistant.soundCheckSummary(result(0, { notChecked: 1 }), true)).toMatch(/^Not checked: score/);
    expect(assistant.soundCheckSummary(result(0, { unanalyzed: 1 }), true)).toMatch(/no analyzed tracks/);
    expect(assistant.soundCheckSummary(result(1, { checked: 4, fitting: 3, notChecked: 1, unanalyzed: 2 }), true))
      .toBe("1 album to review · 3 albums sound like their genre · 1 album with a score or rare genre not checked · 2 albums not analyzed yet");
  });

  it("counts genres in a tag by its semicolons", () => {
    expect(assistant.genreTagHoldsSeveral("Electro; Dance")).toBe(true);
    expect(assistant.genreTagHoldsSeveral("Electro;")).toBe(false);
    expect(assistant.genreTagHoldsSeveral("Films/Games")).toBe(false);
  });
});
