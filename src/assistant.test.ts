import { describe, expect, it } from "vitest";
import { applyAssistantEvent, claudeAddCommand, codexAddCommand, type AssistantTurn } from "./assistant";
import { parseBlocks } from "./assistantMarkdown";

const running: AssistantTurn = { id: "t", role: "assistant", backend: "claudeCode", parts: [], status: "running" };

describe("applyAssistantEvent", () => {
  it("joins streamed text and tracks tools by id", () => {
    let turn = applyAssistantEvent(running, { kind: "textBlock", text: "" });
    turn = applyAssistantEvent(turn, { kind: "textDelta", text: "Hel" });
    turn = applyAssistantEvent(turn, { kind: "textDelta", text: "lo" });
    turn = applyAssistantEvent(turn, { kind: "tool", id: "a", name: "list_genres", status: "running" });
    turn = applyAssistantEvent(turn, { kind: "tool", id: "a", name: "", status: "done" });
    turn = applyAssistantEvent(turn, { kind: "textBlock", text: "Done." });
    expect(turn.parts).toEqual([
      { type: "text", text: "Hello" },
      { type: "tool", id: "a", name: "list_genres", status: "done" },
      { type: "text", text: "Done." },
    ]);
  });

  it("resolves running tools when the run ends", () => {
    const busy = applyAssistantEvent(running, { kind: "tool", id: "a", name: "search_songs", status: "running" });
    expect(applyAssistantEvent(busy, { kind: "done", sessionId: "s", costUsd: null })).toMatchObject({ status: "done", parts: [{ status: "done" }] });
    expect(applyAssistantEvent(busy, { kind: "error", message: "Stopped." })).toMatchObject({ status: "error", error: "Stopped.", parts: [{ status: "failed" }] });
  });
});

describe("terminal commands", () => {
  it("quotes paths with spaces", () => {
    const command = ["/Applications/My Aurora/aurora", "--mcp"];
    expect(claudeAddCommand(command)).toBe('claude mcp add --scope user aurora -- "/Applications/My Aurora/aurora" --mcp');
    expect(codexAddCommand(["/usr/bin/aurora", "--mcp"])).toBe("codex mcp add aurora -- /usr/bin/aurora --mcp");
  });
});

describe("parseBlocks", () => {
  it("reads headings, lists, tables and code", () => {
    const blocks = parseBlocks("## Genres\n\n- Synthpop\n- Italo\n\n| Genre | Albums |\n|---|---|\n| Synthpop | 12 |\n\n```\ngenre:synthpop\n```\nPlain line");
    expect(blocks).toEqual([
      { type: "heading", text: "Genres" },
      { type: "list", ordered: false, items: ["Synthpop", "Italo"] },
      { type: "table", rows: [["Genre", "Albums"], ["Synthpop", "12"]] },
      { type: "code", text: "genre:synthpop" },
      { type: "paragraph", lines: ["Plain line"] },
    ]);
  });
});
