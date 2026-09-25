import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TonightsAlbum } from "./TonightsAlbum";
import { defaultTonightRequest, requestTonight, resetTonightFeedback, saveTonightFeedback, type TonightResult } from "../../tonight";
vi.mock("../../tonight", async importOriginal => ({ ...await importOriginal<typeof import("../../tonight")>(), requestTonight: vi.fn(), saveTonightFeedback: vi.fn(), resetTonightFeedback: vi.fn() }));
const result: TonightResult = {
  request: { ...defaultTonightRequest }, source: "local", message: "Using local ranking.", model: null, candidateCount: 12, generatedAtMs: Date.now(),
  suggestions: ["One", "Two", "Three"].map((title, i) => ({ album: { id: String(i), title, artist: `Artist ${i}`, originalYear: 2000, releaseYear: 2000, genre: "Rock", totalTracks: 10, ratedTracks: 8, lovedTracks: 2, durationSeconds: 2400, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.5, albumScore: null }, reasons: ["40:00 · fits your 60 minutes", "8 of 10 tracks rated"], lastListenedAtMs: null, feedback: null })),
};
afterEach(cleanup);
function setup() { const props = { onPlay: vi.fn().mockResolvedValue(undefined), onOpen: vi.fn(), onSettings: vi.fn() }; return { ...render(<TonightsAlbum {...props} />), props }; }
beforeEach(() => { localStorage.clear(); vi.clearAllMocks(); vi.mocked(requestTonight).mockImplementation(async request => ({ ...result, request })); vi.mocked(saveTonightFeedback).mockResolvedValue(); vi.mocked(resetTonightFeedback).mockResolvedValue(); });
describe("Tonight's Album", () => {
  it("requests only on click, renders facts, and plays the whole chosen album", async () => {
    const { props } = setup();
    fireEvent.click(screen.getByRole("button", { name: /Finish something/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Time to listen" }), { target: { value: "45" } });
    expect(requestTonight).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Find three albums" }));
    expect(await screen.findByRole("article", { name: "One by Artist 0" })).toBeVisible();
    expect(requestTonight).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ intention: "finish", minutes: 45 }));
    fireEvent.click(within(screen.getByRole("article", { name: "One by Artist 0" })).getByRole("button", { name: "Play album" }));
    await waitFor(() => expect(props.onPlay).toHaveBeenCalledWith(result.suggestions[0].album, 45));
    expect(await screen.findByText("Playing One.")).toBeVisible();
  });
  it("remembers none-of-these before requesting another set and prevents duplicate requests", async () => {
    setup(); fireEvent.click(screen.getByRole("button", { name: "Find three albums" }));
    await screen.findByText("One");
    let finish!: () => void;
    vi.mocked(saveTonightFeedback).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    fireEvent.click(screen.getByRole("button", { name: /None of these/ }));
    expect(saveTonightFeedback).toHaveBeenCalledWith("comfort", ["0", "1", "2"], "dismiss");
    expect(requestTonight).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: /None of these/ })).toBeDisabled();
    await act(async () => finish());
    await waitFor(() => expect(requestTonight).toHaveBeenCalledTimes(2));
    expect(vi.mocked(requestTonight).mock.calls[1][0].excludeIds).toEqual(["0", "1", "2"]);
  });
  it("retains original context and restores cards after remount without calling Jev", async () => {
    const { unmount } = setup(); fireEvent.click(screen.getByRole("button", { name: "Find three albums" }));
    await screen.findByText("One");
    fireEvent.click(screen.getByRole("button", { name: /Discovery/ }));
    expect(screen.getByText(/Your choices changed/)).toBeVisible();
    fireEvent.click(within(screen.getByRole("article", { name: "One by Artist 0" })).getByRole("button", { name: "Good fit" }));
    await screen.findByText("Good fit remembered for this intention.");
    expect(saveTonightFeedback).toHaveBeenCalledWith("comfort", ["0"], "good");
    unmount(); setup();
    expect(screen.getByText("One")).toBeVisible();
    expect(screen.getByRole("button", { name: /Discovery/ })).toHaveAttribute("aria-pressed", "true");
    expect(requestTonight).toHaveBeenCalledTimes(1);
  });
  it("keeps suggestions when refresh fails and exposes playback failures", async () => {
    const { props } = setup(); fireEvent.click(screen.getByRole("button", { name: "Find three albums" })); await screen.findByText("One");
    vi.mocked(requestTonight).mockRejectedValueOnce(new Error("Catalog unavailable"));
    fireEvent.click(screen.getByRole("button", { name: "Suggest three more" }));
    expect(await screen.findByText("Catalog unavailable")).toBeVisible();
    expect(screen.getByText("One")).toBeVisible();
    props.onPlay.mockRejectedValueOnce(new Error("File unavailable"));
    fireEvent.click(screen.getAllByRole("button", { name: "Play album" })[0]);
    expect(await screen.findByText("File unavailable")).toBeVisible();
    expect(screen.queryByText("Playing One.")).not.toBeInTheDocument();
  });
  it("does not refresh or remove cards when feedback fails", async () => {
    setup(); fireEvent.click(screen.getByRole("button", { name: "Find three albums" })); await screen.findByText("One");
    vi.mocked(saveTonightFeedback).mockRejectedValueOnce(new Error("Feedback could not be saved"));
    fireEvent.click(screen.getByRole("button", { name: /None of these/ }));
    expect(await screen.findByText("Feedback could not be saved")).toBeVisible();
    expect(screen.getAllByRole("article")).toHaveLength(3);
    expect(requestTonight).toHaveBeenCalledTimes(1);
  });
  it("resets feedback without an automatic request", async () => {
    setup(); fireEvent.click(screen.getByRole("button", { name: "Find three albums" })); await screen.findByText("One");
    fireEvent.click(screen.getByRole("button", { name: "Reset album feedback" }));
    await waitFor(() => expect(screen.queryByText("One")).not.toBeInTheDocument());
    expect(resetTonightFeedback).toHaveBeenCalledOnce(); expect(requestTonight).toHaveBeenCalledTimes(1);
  });
});
