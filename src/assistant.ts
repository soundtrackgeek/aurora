import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { formatCount, isTauriRuntime } from "./library";

export type AssistantBackend = "claudeCode" | "codex";

export interface AssistantSettings {
  libraryAccess: boolean;
  /** Lets assistants show views in Aurora and propose tag changes for review. */
  appControl: boolean;
  /** Lets the Assistant in Aurora search and read the web. */
  webAccess: boolean;
  backend: AssistantBackend;
  claudePath: string;
  codexPath: string;
  /** Model ids passed to each CLI; empty uses the CLI's own default. */
  claudeModel: string;
  codexModel: string;
}

export interface AssistantCliStatus { path: string | null; source: "settings" | "detected" | null }

export interface AssistantStatus {
  settings: AssistantSettings;
  claude: AssistantCliStatus;
  codex: AssistantCliStatus;
  /** The command that starts Aurora's MCP server, for example `aurora.exe --mcp`. */
  mcpCommand: string[];
}

export type AssistantToolStatus = "running" | "done" | "failed";

export type AssistantEvent =
  | { kind: "session"; sessionId: string }
  | { kind: "textBlock"; text: string }
  | { kind: "textDelta"; text: string }
  | { kind: "tool"; id: string; name: string; status: AssistantToolStatus }
  | { kind: "done"; sessionId: string | null; costUsd: number | null }
  | { kind: "error"; message: string };

export type AssistantEventEnvelope = AssistantEvent & { runId: string };

export type AssistantPart =
  | { type: "text"; text: string }
  | { type: "tool"; id: string; name: string; status: AssistantToolStatus };

export interface AssistantTurn {
  id: string;
  role: "user" | "assistant";
  backend: AssistantBackend;
  parts: AssistantPart[];
  status: "running" | "done" | "error";
  error?: string;
}

export interface AssistantConversation {
  /** Each CLI keeps its own session, so switching backends starts a new one. */
  sessions: Partial<Record<AssistantBackend, string>>;
  turns: AssistantTurn[];
}

export const emptyConversation: AssistantConversation = { sessions: {}, turns: [] };

export const backendLabels: Record<AssistantBackend, string> = { claudeCode: "Claude Code", codex: "OpenAI Codex" };

const previewStatus: AssistantStatus = {
  settings: { libraryAccess: false, appControl: false, webAccess: true, backend: "claudeCode", claudePath: "", codexPath: "", claudeModel: "claude-haiku-5-5", codexModel: "gpt-6-luna" },
  claude: { path: null, source: null },
  codex: { path: null, source: null },
  mcpCommand: ["aurora.exe", "--mcp"],
};

export async function loadAssistantStatus(): Promise<AssistantStatus> {
  return isTauriRuntime() ? invoke("assistant_status") : previewStatus;
}

export async function saveAssistantSettings(settings: AssistantSettings): Promise<AssistantStatus> {
  if (!isTauriRuntime()) throw new Error("Set up the assistant in the desktop app.");
  return invoke("update_assistant_settings", { settings });
}

export async function testAssistantBackend(backend: AssistantBackend): Promise<string> {
  if (!isTauriRuntime()) throw new Error("Test the assistant in the desktop app.");
  return invoke("test_assistant_backend", { backend });
}

export interface AssistantSendRequest {
  message: string;
  backend: AssistantBackend;
  sessionId: string | null;
  context: string | null;
}

/** Starts a background run and returns its id; events arrive through `listenToAssistant`. */
export async function sendAssistantMessage(request: AssistantSendRequest): Promise<string> {
  if (!isTauriRuntime()) throw new Error("The assistant runs in the desktop app.");
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  return invoke("assistant_send", { request: { ...request, today, utcOffsetMinutes: -now.getTimezoneOffset() } });
}

export async function cancelAssistantRun(runId: string): Promise<void> {
  if (isTauriRuntime()) await invoke("assistant_cancel", { runId });
}

export async function listenToAssistant(handler: (event: AssistantEventEnvelope) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<AssistantEventEnvelope>("assistant://event", (event) => handler(event.payload));
}

export type AssistantExplorerView = "songs" | "albums" | "artists";

/** An assistant asked Aurora to show a search. */
export interface AssistantShowRequest { view: AssistantExplorerView; query: string }

export type GenreProposalStatus = "pending" | "applying" | "done" | "dismissed";
export type GenreChangeStatus = "pending" | "applied" | "skipped" | "failed";

export interface GenreProposalAlbum {
  albumId: string;
  artist: string | null;
  title: string | null;
  year: number | null;
  currentTags: { tag: string; tracks: number }[];
  proposedGenre: string;
  appliedGenre: string | null;
  status: GenreChangeStatus;
  error: string | null;
  /** Why Aurora's genre check suggests the genre; assistants send none. */
  evidence?: GenreEvidence;
}

/** A song of the suggested genre that sounds like the album. */
export interface GenreEvidenceTrack {
  trackId: string;
  title: string | null;
  artist: string | null;
  album: string | null;
  year: number | null;
}

/** A genre an album sounds close to. */
export interface NearGenre {
  genre: string;
  /** The share of the genre's albums it is closer to the genre's sound than, 0–100. */
  percentile: number;
}

export interface GenreEvidence {
  /**
   * `retag`: sounds like another genre. `refine`: a specific genre for a broad one.
   * `fits`: a chosen album that keeps its genre.
   */
  kind: "retag" | "refine" | "fits";
  /** Absent for `fits`. */
  confidence?: "high" | "medium";
  summary: string;
  tracks: GenreEvidenceTrack[];
  /** Other genres it sounds closest to, closest first. */
  closest: NearGenre[];
}

export interface GenreProposal {
  id: string;
  createdAtMs: number;
  /** The MCP client that sent it, for example "claude-code". */
  source: string;
  reason: string;
  status: GenreProposalStatus;
  albums: GenreProposalAlbum[];
}

export async function listenToAssistantShow(handler: (request: AssistantShowRequest) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<AssistantShowRequest>("assistant://show", (event) => handler(event.payload));
}

export async function listenToGenreProposals(handler: (proposal: GenreProposal) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<GenreProposal>("assistant://proposal", (event) => handler(event.payload));
}

export async function loadGenreProposals(): Promise<GenreProposal[]> {
  return isTauriRuntime() ? invoke("assistant_proposals") : [];
}

export async function applyGenreProposal(proposalId: string, changes: { albumId: string; genre: string }[]): Promise<GenreProposal> {
  return invoke("apply_assistant_proposal", { proposalId, changes });
}

/** What a genre check by sound found. */
export interface SoundCheck {
  /** The findings, open in Review genre changes; `null` when no album stands out. */
  proposal: GenreProposal | null;
  checked: number;
  /** Checked albums that sound like their genre. */
  fitting: number;
  /** Albums without analyzed tracks or without a genre. */
  unanalyzed: number;
  /** Albums with a score, other non-sound or rare genre, which the check leaves alone. */
  notChecked: number;
}

/** Runs the genre check that listens on the whole library, or on the given albums. */
export async function checkGenresBySound(albumIds?: readonly string[]): Promise<SoundCheck> {
  if (!isTauriRuntime()) throw new Error("Check genres by sound from Aurora desktop.");
  return invoke("check_genres_by_sound", { albumIds: albumIds ? [...albumIds] : null });
}

function albums(count: number): string {
  return `${formatCount(count)} ${count === 1 ? "album" : "albums"}`;
}

/** One line on what a check found; `chosen` when it checked selected albums. */
export function soundCheckSummary(result: SoundCheck, chosen: boolean): string {
  const reviewed = result.proposal?.albums ?? [];
  const flagged = reviewed.filter((album) => album.evidence?.kind !== "fits").length;
  if (!chosen) return flagged ? `${albums(flagged)} to review` : "Every album sounds like its genre";
  const single = result.checked + result.unanalyzed + result.notChecked === 1;
  if (single) {
    if (flagged) return "A different genre fits its sound better. Opened for review.";
    if (result.fitting) {
      const near = reviewed[0]?.evidence?.closest.map((item) => item.genre) ?? [];
      return near.length ? `Sounds like its genre. Next closest: ${near.join(", ")}.` : "Sounds like its genre.";
    }
    if (result.notChecked) return "Not checked: score and other non-sound genres, and genres on fewer than 30 albums, are left alone.";
    return "Not checked: this album has no analyzed tracks or no genre yet.";
  }
  const parts: string[] = [];
  if (flagged) parts.push(`${albums(flagged)} to review`);
  if (result.fitting) parts.push(`${albums(result.fitting)} sound like their genre`);
  if (result.notChecked) parts.push(`${albums(result.notChecked)} with a score or rare genre not checked`);
  if (result.unanalyzed) parts.push(`${albums(result.unanalyzed)} not analyzed yet`);
  return parts.join(" · ") || "Nothing to check";
}

export async function dismissGenreProposal(proposalId: string): Promise<GenreProposal> {
  return invoke("dismiss_assistant_proposal", { proposalId });
}

/** Replaces a proposal in a list by id, or adds it. */
export function upsertProposal(proposals: readonly GenreProposal[], proposal: GenreProposal): GenreProposal[] {
  const index = proposals.findIndex((item) => item.id === proposal.id);
  if (index < 0) return [...proposals, proposal];
  return proposals.map((item, position) => position === index ? proposal : item);
}

/** "Electro; Dance" holds two genres; the rule is one per album. */
export function genreTagHoldsSeveral(tag: string): boolean {
  return tag.split(";").filter((part) => part.trim()).length > 1;
}

/** Shell-quotes an argument for the `claude mcp add` / `codex mcp add` hints. */
function quote(value: string): string {
  return /^[\w./:\\-]+$/.test(value) ? value : `"${value.replace(/"/g, '\\"')}"`;
}

export function claudeAddCommand(mcpCommand: string[]): string {
  return `claude mcp add --scope user aurora -- ${mcpCommand.map(quote).join(" ")}`;
}

export function codexAddCommand(mcpCommand: string[]): string {
  return `codex mcp add aurora -- ${mcpCommand.map(quote).join(" ")}`;
}

/** Applies one run event to the assistant turn it belongs to. */
export function applyAssistantEvent(turn: AssistantTurn, event: AssistantEvent): AssistantTurn {
  switch (event.kind) {
    case "session":
      return turn;
    case "textBlock": {
      const last = turn.parts[turn.parts.length - 1];
      // An empty block followed by deltas reuses an empty trailing paragraph.
      if (last?.type === "text" && !last.text.trim()) {
        return { ...turn, parts: [...turn.parts.slice(0, -1), { type: "text", text: event.text }] };
      }
      return { ...turn, parts: [...turn.parts, { type: "text", text: event.text }] };
    }
    case "textDelta": {
      const last = turn.parts[turn.parts.length - 1];
      if (last?.type === "text") {
        return { ...turn, parts: [...turn.parts.slice(0, -1), { type: "text", text: last.text + event.text }] };
      }
      return { ...turn, parts: [...turn.parts, { type: "text", text: event.text }] };
    }
    case "tool": {
      const index = turn.parts.findIndex((part) => part.type === "tool" && part.id === event.id);
      const part: AssistantPart = { type: "tool", id: event.id, name: event.name || (index >= 0 ? (turn.parts[index] as { name: string }).name : "tool"), status: event.status };
      if (index < 0) return { ...turn, parts: [...turn.parts, part] };
      return { ...turn, parts: turn.parts.map((existing, position) => position === index ? part : existing) };
    }
    case "done":
      return {
        ...turn,
        status: "done",
        parts: turn.parts.map((part) => part.type === "tool" && part.status === "running" ? { ...part, status: "done" } : part),
      };
    case "error":
      return {
        ...turn,
        status: "error",
        error: event.message,
        parts: turn.parts.map((part) => part.type === "tool" && part.status === "running" ? { ...part, status: "failed" } : part),
      };
  }
}

/** Plain-language labels for Aurora's MCP tools, shown while the assistant works. */
export const toolLabels: Record<string, string> = {
  library_overview: "Library overview",
  search_syntax: "Search guide",
  search_songs: "Searching songs",
  search_albums: "Searching albums",
  search_artists: "Searching artists",
  album_detail: "Album details",
  artist_detail: "Artist details",
  list_genres: "Genres",
  genre_detail: "Genre details",
  genre_tag_variants: "Genre tag spellings",
  albums_with_mixed_genres: "Albums with mixed genres",
  listening_report: "Listening history",
  ratings_overview: "Ratings",
  year_overview: "Years",
  albums_with_multiple_genres: "Albums with more than one genre",
  show_in_aurora: "Showing it in Aurora",
  propose_genre_changes: "Sending genre changes for review",
  proposal_status: "Checking the review",
  describe_sound: "Listening to the music",
  tracks_like: "Finding music that sounds alike",
  WebSearch: "Searching the web",
  WebFetch: "Reading a web page",
  web_search: "Searching the web",
};
