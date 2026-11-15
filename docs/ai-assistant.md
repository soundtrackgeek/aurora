# AI assistant and MCP server

*Aurora 0.62.0*

Aurora exposes its library to AI assistants through a read-only [Model Context Protocol](https://modelcontextprotocol.io) server, and its **Assistant** panel runs Claude Code or OpenAI Codex against that server. The design and later phases are in [ai-assistant-proposal.md](ai-assistant-proposal.md).

## The server

- **Start:** `aurora.exe --mcp`. `main` checks for the flag before Tauri starts, so no window opens and the single-instance lock is not taken.
- **Transport:** newline-delimited JSON-RPC 2.0 over stdio. The methods are `initialize` (protocol versions 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05), `ping`, `tools/list` and `tools/call`. Notifications get no reply, and any other method returns `-32601`.
- **Access:** `aurora-assistant.json` in Aurora's app data folder holds `libraryAccess`, which defaults to `false`. The server rereads it on every `tools/call` and refuses calls while it is off.
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
| `albums_with_mixed_genres` | Albums whose tracks carry more than one genre |
| `listening_report` | Plays between two dates in the user's time zone |
| `ratings_overview`, `year_overview` | Rating and year distributions |

Search queries need an explicit `AND` between field terms, as in `aartist:madonna AND love:1`. `sort:` and `group:` clauses need none.

## Privacy

The server cleans every result before it leaves:

- It removes keys named like paths, `trackKey`, and strings that look like absolute paths.
- It removes Last.fm's global `playCount` on tracks, so it cannot be mistaken for the user's own plays. It also removes `loveState`.
- It caps lists at 200 items and a result at 120,000 characters.

## The Assistant panel

For each message, Aurora runs the chosen CLI with the prompt on stdin. It parses the CLI's streamed JSON into text, tool-status and done events, which it sends to the panel on `assistant://event`.

- **Claude Code:** `-p --output-format stream-json --verbose --include-partial-messages --mcp-config <generated> --strict-mcp-config --tools "" --allowedTools mcp__aurora --permission-mode dontAsk --setting-sources ""`, with Aurora's system prompt, and `--resume` for follow-ups.
- **Codex:** `exec --json --ignore-user-config --sandbox read-only`, with Aurora's MCP server added through `-c`, `approval_policy="never"`, web search off, and the shell, plugin, app and multi-agent features off. `resume <id>` continues a conversation.

Aurora validates session IDs before passing them to the CLI. A run times out after 10 minutes, and **Stop** kills the process.

## Not yet

- Action tools and tag-change proposals (phase 2)
- Playable result cards
- Saved conversation history
- Gemini CLI and API backends
- A tool that reads the view the user has open
