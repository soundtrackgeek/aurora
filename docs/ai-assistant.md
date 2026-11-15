# AI assistant and MCP server

*Aurora 0.63.0*

Aurora exposes its library to AI assistants through a local [Model Context Protocol](https://modelcontextprotocol.io) server, and its **Assistant** panel runs Claude Code or OpenAI Codex against that server. Most tools only read. Two tools act on Aurora, and only while **Let AI assistants control Aurora** is on: one opens a search in the running app, and one sends genre changes that the user reviews and applies in Aurora. The design and later phases are in [ai-assistant-proposal.md](ai-assistant-proposal.md).

## The server

- **Start:** `aurora.exe --mcp`. `main` checks for the flag before Tauri starts, so no window opens and the single-instance lock is not taken.
- **Transport:** newline-delimited JSON-RPC 2.0 over stdio. The methods are `initialize` (protocol versions 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05), `ping`, `tools/list` and `tools/call`. Notifications get no reply, and any other method returns `-32601`.
- **Access:** `aurora-assistant.json` in Aurora's app data folder holds `libraryAccess` and `appControl`, which both default to `false`. The server rereads them on every `tools/call` and refuses every call while `libraryAccess` is off, and the control tools while `appControl` is off.
- **Databases:** the server opens the state database only if it is at the current schema version, and never migrates it. It opens history without recovering interrupted sessions or publishing to Tonehavn. It reads the Music Library catalog the same way Aurora does.

## Tools

| Tool | Answers |
|---|---|
| `library_overview` | Counts of tracks, albums, artists, genres and loved tracks |
| `search_syntax` | Aurora's search language ([search.md](search.md)) |
| `search_songs`, `search_albums`, `search_artists` | One page of results for a query, with an opaque cursor for the next page |
| `album_detail`, `artist_detail` | One album's tracks, or one artist's albums and listening |
| `list_genres`, `genre_detail` | Genres with counts, ratings, loves and plays |
| `genre_tag_variants` | Raw genre tags that map to the same canonical genre |
| `albums_with_mixed_genres` | Albums whose tracks carry more than one canonical genre |
| `albums_with_multiple_genres` | Albums that break a one-genre-per-album rule in their tags as written: a tag that holds several genres (`Electro; Dance`) or tracks whose tags differ. Each album lists its tags with track counts, its genres, and the genre listed first on most tracks. Filters: `genre` (canonical genre) and `tagContains` (text in a raw tag) |
| `listening_report` | Plays between two dates in the user's time zone |
| `ratings_overview`, `year_overview` | Rating and year distributions |
| `show_in_aurora` | **Acts.** Opens Songs, Albums or Artists in the running Aurora with a search query, and brings the window forward. The query is checked first, and the result says how many items it found |
| `propose_genre_changes` | **Acts.** Sends up to 500 `{albumId, genre}` changes with a reason. Aurora opens a review dialog; nothing is written until the user applies it there |
| `proposal_status` | Whether a proposal is waiting, applying, done or dismissed, and each album's result |

Search queries need an explicit `AND` between field terms, as in `aartist:madonna AND love:1`. `sort:` and `group:` clauses need none. `genres:2..` finds albums with more than one genre.

The two acting tools have `readOnlyHint: false`; every other tool has `readOnlyHint: true`. All tools have `openWorldHint: false`.

## Controlling Aurora

The running app listens on a random port on `127.0.0.1` and writes the port, a random 256-bit token and its process ID to `aurora-control.json` in its app data folder (readable only by the user on macOS and Linux). The MCP server reads that file and sends one JSON line per request with the token; Aurora compares the token in constant time and rereads both switches on every request. If Aurora is not running, the tools say so and change nothing.

- **`show`** emits `assistant://show`. Aurora opens the view with the query, as if it was typed into the search box, and the top bar notes that an assistant opened it.
- **`proposeGenreChanges`** checks that every album exists and every genre is a single line of at most 256 characters, then keeps the proposal in memory (the 20 newest). It emits `assistant://proposal`, which opens **Review genre changes**.
- **`proposalStatus`** returns the proposal.

In the review dialog, the user sees each album's current tags with track counts (tags that hold several genres are marked), and can untick albums or edit the genre. **Write genre** applies the chosen albums one at a time through the album tag editor's own path: it reads the album's current tags, writes the genre to every track with the same check against edits made in the meantime, verifies the files, queues the Music Library sync, and updates the playing track. It needs music writes to be allowed. A failed album stays open so the user can try again, and **Dismiss** skips the rest. Proposals do not survive a restart.

## Privacy

The server cleans every result before it leaves:

- It removes keys named like paths, `trackKey`, and strings that look like absolute paths.
- It removes Last.fm's global `playCount` on tracks, so it cannot be mistaken for the user's own plays. It also removes `loveState`.
- It caps lists at 200 items and a result at 120,000 characters.

## The Assistant panel

For each message, Aurora runs the chosen CLI with the prompt on stdin. It parses the CLI's streamed JSON into text, tool-status and done events, which it sends to the panel on `assistant://event`.

- **Claude Code:** `-p --output-format stream-json --verbose --include-partial-messages --mcp-config <generated> --strict-mcp-config --tools "" --allowedTools mcp__aurora --permission-mode dontAsk --setting-sources ""`, with Aurora's system prompt, `--model` (default `claude-haiku-5-5`), and `--resume` for follow-ups.
- **Codex:** `exec --json --ignore-user-config --sandbox read-only`, with Aurora's MCP server added through `-c`, `approval_policy="never"`, web search off, and the shell, plugin, app and multi-agent features off. Because `never` declines tools that are not read-only, `show_in_aurora` and `propose_genre_changes` get `approval_mode="approve"`. `-m` picks the model (default `gpt-6-luna`), and `resume <id>` continues a conversation.

The system prompt tells the assistant what it may do: with control on, to call `show_in_aurora` when asked to show something and to propose genre changes instead of claiming to write them; with control off, to point to the switch and give a query to paste. In a terminal, Claude Code and Codex ask before the acting tools unless you allow them.

The models are `claudeModel` and `codexModel` in `aurora-assistant.json`; an empty value leaves out the flag, so the CLI uses its own default. Aurora validates model and session IDs before passing them to the CLI, so neither can be read as a flag. A run times out after 10 minutes, and **Stop** kills the process.

## Not yet

- Proposals for other tags, ratings, playlists and file renames
- Playable result cards
- Saved conversation history
- Gemini CLI and API backends
- A tool that reads the view the user has open
