# AI library assistant: potential improvement

*Proposal · 14 November 2026 · phases 1 and 3 implemented in Aurora 0.62.0, plus the Codex backend from phase 4; phase 2 started in 0.63.0 with app control and reviewed genre-change proposals; see [ai-assistant.md](ai-assistant.md)*

**What shipped differs from this proposal in four ways.** The server uses stdio only, so it needs no localhost token: only the process that started it can talk to it. It is a small hand-written JSON-RPC server instead of `rmcp`, because it is synchronous and handles only four methods. The Assistant is a panel opened from the top bar, not a tab. Conversations stay in memory for now and are not saved in the state database.

This proposal describes an assistant that answers questions about the library, builds playlists, and suggests tag fixes. It works with the AI subscriptions the user already pays for (Claude Code, OpenAI Codex, Gemini CLI) as well as with API keys. It builds on two existing roadmap items: [Feature 7, natural-language search](improvement-roadmap.md#7-natural-language-search) and [Feature 20, a local read-only MCP server](improvement-roadmap.md#20-local-read-only-mcp-server).

## Background: what Helium ships

[Helium Music Manager](https://www.imploded.com/helium) added an AI assistant in October 2026 ([release post](https://implodedsoftware.wordpress.com/)):

- An **Assistant** tab works with the user's own library, not general music knowledge. It can search, play, build regular and smart playlists, fix tags, start BPM/key analysis, and find duplicate albums. The open view is used as context.
- **Tag changes are proposals.** They appear in a review dialog and nothing is written until the user approves.
- **Providers:** Claude Code (the user's Claude plan), OpenAI Codex (the user's ChatGPT plan), or an API key for Anthropic, OpenAI, or Google Gemini, with a connection test.
- An **MCP server** lets Claude Code, Claude Desktop, and other MCP clients reach the library. Only the local machine can connect, each connection needs a secret token, and changes need approval.

Aurora uses an LLM only in Tonight's Album, through OpenRouter (`src-tauri/src/tonight.rs`).

## Can it use existing subscriptions?

Yes. Two designs make this possible. They depend on each other, and both use the same MCP server.

### Option A: Aurora exposes an MCP server and the user's CLI connects to it

The user stays in Claude Code, Codex, or Gemini CLI and registers Aurora once as an MCP server (for example `claude mcp add aurora …`, or the matching entry in Codex's or Gemini's configuration).

- **Billing:** whatever the CLI is signed in with, usually the subscription. Aurora never handles AI credentials.
- **Effort:** medium. This is Feature 20, and Aurora's bounded commands map onto MCP tools directly.
- **Limit:** the conversation happens in a terminal, not inside Aurora.

### Option B: an Assistant tab in Aurora that runs the CLI in the background

Aurora starts the installed CLI in non-interactive mode, gives it Aurora's own MCP server, and parses the streamed JSON output into the tab.

| CLI | Headless call (approximate) | Limiting it to Aurora's tools |
|---|---|---|
| Claude Code | `claude -p "<prompt>" --output-format stream-json --mcp-config aurora.json --strict-mcp-config` | `--allowedTools "mcp__aurora__*"`; built-in file and shell tools disabled |
| OpenAI Codex | `codex exec --json "<prompt>"` with Aurora configured as an MCP server | read-only sandbox; no shell command approvals |
| Gemini CLI | `gemini -p "<prompt>" --output-format json` | `--allowed-mcp-server-names aurora` |

CLI flags change between releases. Check each one against the installed version when implementing, and detect missing support at runtime rather than assuming it.

Each CLI uses its own sign-in, so the subscription pays for the requests. An **API backend** (the existing OpenRouter key in the OS credential vault, or a direct Anthropic, OpenAI, or Gemini key) covers users who have no CLI installed. In that case Aurora runs the tool loop itself, against the same tool definitions.

## Design

### One MCP server, several "brains"

The MCP server is the only path between a model and the library. The CLI or API is a replaceable backend. Building the server first makes Option A available immediately and reduces Option B to process management and a chat UI.

Use the official Rust SDK ([`rmcp`](https://github.com/modelcontextprotocol/rust-sdk)) inside the Tauri backend, as Feature 20 already proposes. Transport: stdio for a CLI that Aurora launches itself, and localhost Streamable HTTP for a CLI the user runs on their own.

### Tools

Read tools reuse existing bounded commands and keep their limits (at most 100 rows per page, keyset cursors):

| Tool | Backed by |
|---|---|
| `search_catalog` | the catalog query language ([search guide](search.md)), including `love:`, `single:`, `country:` and chart fields |
| `album_detail`, `artist_overview` | album inspector and artist page data |
| `listening_report` | listening history across devices |
| `ratings_overview` | Ratings Studio, rating missions |
| `chart_info` | chart peaks and the singles data |
| `similar_sound` | sonic "More like this" and sonic smart playlists |
| `current_view` | what the user has open (search, selected album), so they can say "these albums" |

Action tools, in increasing order of risk:

| Tool | Effect |
|---|---|
| `queue_tracks`, `play` | change playback only |
| `create_playlist`, `create_smart_playlist` | write through the existing Music Library playlist bridge |
| `propose_tag_changes` | **creates a proposal only**, see below |

### Proposals, never direct writes

`propose_tag_changes` stores a proposal in Aurora's writable state database: the tracks, the old values read from the files, the new values, and the model's reason. It writes no MP3.

The Assistant tab (or a notification when Option A is used) shows each proposal as a card with a field-by-field comparison. **Approve** sends it through the existing verified tag pipeline ([tag-editing contract](tag-editing-contract.md)): working copy, audio SHA-256 check, atomic replace, album rollback, then the Music Library sync queue. **Reject** discards it.

If a file changed after the proposal was made, the proposal is marked stale and must be rebuilt. The rule that the MP3 is the source of truth stays the same, no matter what a model gets wrong.

### Guardrails

- **Off by default.** It is turned on in Settings with an explicit toggle, and read-only mode is the default.
- **Local only.** The HTTP transport binds to `127.0.0.1` and needs a per-session secret token, as Helium does.
- **No file paths or credentials** in tool results. Use album and track identifiers, as Aurora's queue and overlays already do.
- **Restricted CLI.** In Option B, Aurora launches the CLI with only Aurora's MCP tools allowed and file, shell, and web tools disabled.
- **Bounded output.** Every tool keeps the existing page and payload limits, so a model cannot pull the whole 1.1M-track catalog.
- **Audit trail.** Approved and rejected proposals are kept in the state database alongside the tag-operation journal.

### Settings → Connections → AI assistant

- Backend: Claude Code, OpenAI Codex, Gemini CLI, OpenRouter, or a direct API key.
- **Executable path** for each CLI, detected automatically and editable. On this machine Node comes from fnm, so npm-installed CLIs (Codex, Gemini CLI) are usually not on the `PATH` that a GUI app inherits. Detection must check the usual install locations, and the user must be able to point Aurora at the executable.
- **Test connection**, which runs a trivial prompt and reports the CLI version, the sign-in state, and whether MCP tools were reachable.
- **Allow actions:** read only / playback and playlists / tag proposals.

## Phases

1. **Read-only MCP server (Option A).** Read tools, a Settings toggle, local transport with a token, and documentation for registering it in each CLI. This is Feature 20 as written.
2. **Action tools and proposals.** Playback, playlists, and `propose_tag_changes`, with a proposals list in Aurora and approval through the verified pipeline.
3. **Assistant tab (Option B) with Claude Code first.** Run the CLI in the background, parse the streamed output, render track and album results as playable cards and proposals as approval cards, and keep a conversation history in the state database.
4. **More backends.** Codex, Gemini CLI, then the API backends (OpenRouter first, since its key and connection test already exist).
5. **Natural-language search (Feature 7)** becomes a mode of the same assistant: the model proposes a query, and the user edits and runs it.

## Risks and open questions

- **Provider terms.** Running your own CLI on your own machine is ordinary use of that CLI. Distributing Aurora with subscription sign-in built in is different: Anthropic, for example, says third-party products should not offer claude.ai sign-in or subscription limits without its approval and should use API keys. Check each provider's current terms before releasing this to anyone else.
- **Latency.** Each headless call starts a process and connects MCP, which takes a few seconds before the first token. That suits "build me a playlist" or "find inconsistent genres", not live search suggestions. Keeping one CLI session alive between messages (where the CLI supports resuming) could reduce it.
- **Shared limits.** Assistant requests use the same subscription limits as the user's coding sessions.
- **Output formats.** The CLIs' JSON streams are not a stable contract across versions. The parser should tolerate unknown events and report a clear error when the format changes.
- **Privacy.** Tool results (artist, album, and track names, ratings, play counts) go to the chosen provider. Settings should say so, as Tonight's Album already does for its bounded request.
- **Open question:** should Option A proposals be approved in Aurora only, or could the CLI approve them as well? Approving in Aurora only is safer and matches Helium.

## Sources

- [Helium Music Manager](https://www.imploded.com/helium)
- [Imploded Software blog: New versions with integrated AI support (10 October 2026)](https://implodedsoftware.wordpress.com/)
- [Model Context Protocol Rust SDK (`rmcp`)](https://github.com/modelcontextprotocol/rust-sdk)
