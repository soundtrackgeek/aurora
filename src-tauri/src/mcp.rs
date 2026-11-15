//! Aurora's MCP server.
//!
//! `aurora.exe --mcp` speaks the Model Context Protocol over stdio, so Claude
//! Code, OpenAI Codex, Gemini CLI, Claude Desktop, and Aurora's own Assistant
//! can ask questions about the library. It runs as a separate process from
//! the Aurora window and never writes the catalog, the MP3 files, or Aurora's
//! state: it opens Music Library's catalog read-only and opens Aurora's state
//! and listening history with side-effect-free reader constructors.
//!
//! A stdio server is reachable only by the process that started it. The
//! control tools (`show_in_aurora`, `propose_genre_changes`, `proposal_status`)
//! ask the running Aurora window through `control`; proposed tag changes wait
//! there for the user to apply them. Every tool keeps the bounded page sizes
//! of the commands it reuses, and results never contain file paths.

use crate::{
    assistant, catalog, connections, control, device_mode, explorer, genre_rule, genres,
    history::HistoryStore, ratings, state_store::StateStore, state_sync, years,
};
use serde_json::{Map, Value, json};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

const SERVER_NAME: &str = "aurora";
const SUPPORTED_PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_RESULT_CHARS: usize = 120_000;
const MAX_ARRAY_ITEMS: usize = 200;
const DEFAULT_PAGE_SIZE: u64 = 25;
const MAX_PAGE_SIZE: u64 = 100;
const SEARCH_GUIDE: &str = include_str!("../../docs/search.md");

const INSTRUCTIONS: &str = "Aurora is a local music library explorer. Its catalog comes from Music Library, \
which imports the user's MP3 collection (about a million tracks). The library tools are read-only. No tool \
writes tags directly: propose_genre_changes sends genre changes to Aurora, where the user reviews and applies \
them; check the outcome with proposal_status. show_in_aurora opens a Songs, Albums or Artists search in the \
running Aurora window so the user can see results; prefer it when the user asks to see or show something. \
The control tools need Aurora running and its \"Let AI assistants control Aurora\" setting. The user's rule is \
usually one genre per album: albums_with_multiple_genres lists albums that break it (a tag holding several \
genres joined by ';', or tracks that disagree) with each album's primaryGenre, and `genres:2..` finds the \
same albums in a search. Ratings are stars from 0.5 to 5 in half steps; loved tracks have loved=true. \
`plays` and listening reports are the user's own plays recorded by Aurora on all their devices. \
Album IDs and track IDs are opaque; pass them back unchanged. Searches use Aurora's query language: call \
search_syntax before writing anything beyond plain words. Join field terms with an explicit AND (or OR, NOT, \
parentheses): `genre:synthpop AND year:1980..1989 AND love:1..` works, but `genre:synthpop year:1980..1989` \
matches nothing. `sort:` and `group:` clauses need no AND, as in `aartist:madonna AND love:1 sort:..year`. \
Genre names are canonical genres from list_genres. Results are paged: pass nextCursor \
back to continue. When you mention albums or tracks to the user, include artist, title, and year.";

/// Runs the server until stdin closes. Returns the process exit code.
pub fn run_stdio() -> i32 {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut server = Server::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            let Ok(text) = serde_json::to_string(&response) else {
                continue;
            };
            if writeln!(stdout, "{text}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break;
            }
        }
    }
    0
}

#[derive(Default)]
struct Server {
    context: Option<Context>,
    /// The MCP client's name from `initialize`, shown with proposals.
    client: String,
}

struct Context {
    state_directory: PathBuf,
    store: StateStore,
    history: HistoryStore,
}

impl Server {
    fn handle_line(&mut self, line: &str) -> Option<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(error_response(
                    Value::Null,
                    -32700,
                    &format!("Parse error: {error}"),
                ));
            }
        };
        let Some(object) = message.as_object() else {
            return Some(error_response(
                Value::Null,
                -32600,
                "Expected a JSON-RPC request object.",
            ));
        };
        // Notifications (no id) and responses to our own requests need no reply.
        let id = object.get("id").cloned()?;
        let method = object.get("method").and_then(Value::as_str)?;
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        Some(match self.dispatch(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error_response(id, code, &message),
        })
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                if let Some(name) = params.pointer("/clientInfo/name").and_then(Value::as_str) {
                    self.client = name.chars().take(80).collect();
                }
                Ok(initialize_result(params))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "tools/call needs a tool name.".to_owned()))?;
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                Ok(self.call_tool(name, &arguments))
            }
            _ => Err((
                -32601,
                format!("Aurora's MCP server has no method {method}."),
            )),
        }
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> Value {
        let client = self.client.clone();
        let outcome = self
            .context()
            .and_then(|context| {
                if !assistant::library_access_enabled(&context.state_directory) {
                    return Err(
                        "AI access to the library is off. Turn on Settings → AI assistant → \
                         \"Let AI assistants read my library\" in Aurora, then try again."
                            .to_owned(),
                    );
                }
                run_tool(context, &client, name, arguments)
            })
            .map(|value| {
                let mut value = value;
                sanitize(&mut value);
                value
            })
            .and_then(|value| {
                let text = serde_json::to_string(&value)
                    .map_err(|error| format!("Could not encode the result: {error}"))?;
                if text.chars().count() > MAX_RESULT_CHARS {
                    return Err(
                        "The result is too large. Use a smaller pageSize or a narrower query."
                            .to_owned(),
                    );
                }
                Ok(text)
            });
        match outcome {
            Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
            Err(message) => {
                json!({ "content": [{ "type": "text", "text": message }], "isError": true })
            }
        }
    }

    fn context(&mut self) -> Result<&Context, String> {
        if self.context.is_none() {
            self.context = Some(Context::open()?);
        }
        self.context
            .as_ref()
            .ok_or_else(|| "Aurora's library is unavailable.".to_owned())
    }
}

impl Context {
    fn open() -> Result<Self, String> {
        let state_directory = app_data_dir()?;
        connections::initialize(&state_directory)?;
        device_mode::apply_saved_laptop_mode(&state_directory.join("aurora-device.json"));
        let store = StateStore::open_existing(state_directory.join("aurora-state.sqlite3"))?;
        let remote_directory = state_sync::default_remote_state_path()?
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        let history = HistoryStore::open_reader(
            state_directory.join("aurora-history.sqlite3"),
            remote_directory,
        )?;
        Ok(Self {
            state_directory,
            store,
            history,
        })
    }
}

/// Aurora's Tauri app-data directory, computed without a running Tauri app.
pub(crate) fn app_data_dir() -> Result<PathBuf, String> {
    const IDENTIFIER: &str = "com.soundtrackgeek.aurora";
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "APPDATA is not set.".to_owned())?
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|home| {
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
            })
            .ok_or_else(|| "HOME is not set.".to_owned())?
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
            .ok_or_else(|| "HOME is not set.".to_owned())?
    };
    Ok(base.join(IDENTIFIER))
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(SUPPORTED_PROTOCOLS[0]);
    let protocol = SUPPORTED_PROTOCOLS
        .iter()
        .find(|version| **version == requested)
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOLS[0]);
    json!({
        "protocolVersion": protocol,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": SERVER_NAME,
            "title": "Aurora music library",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": INSTRUCTIONS,
    })
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
        "annotations": {
            "title": title,
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false,
        },
    })
}

/// A tool that asks the running Aurora to do something. It never writes tags.
fn control_tool(
    name: &str,
    title: &str,
    description: &str,
    properties: Value,
    required: &[&str],
) -> Value {
    let mut definition = tool(name, title, description, properties, required);
    definition["annotations"]["readOnlyHint"] = json!(false);
    definition["annotations"]["idempotentHint"] = json!(false);
    definition
}

/// Tools that act on the running Aurora window instead of only reading.
#[cfg(test)]
const CONTROL_TOOLS: [&str; 2] = ["show_in_aurora", "propose_genre_changes"];

fn paging_properties(extra: Value) -> Value {
    let mut properties = json!({
        "query": {
            "type": "string",
            "description": "Aurora search text: plain words and/or fields such as genre:, aartist:, year:, love:, rating:, country:, single:. Join field terms with AND/OR/NOT, e.g. `genre:synthpop AND year:1984`; field terms without AND match nothing. Add sort: and group: clauses without AND. See search_syntax.",
        },
        "pageSize": {
            "type": "integer", "minimum": 1, "maximum": MAX_PAGE_SIZE,
            "description": "Rows per page (default 25, at most 100).",
        },
        "cursor": {
            "type": "object",
            "description": "nextCursor from the previous page, passed back unchanged.",
        },
    });
    if let (Some(base), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
        base.extend(extra.clone());
    }
    properties
}

fn tool_definitions() -> Vec<Value> {
    let year_range = json!({
        "genre": { "type": "string", "description": "Exact canonical genre name." },
        "artist": { "type": "string", "description": "Exact album artist name." },
        "yearFrom": { "type": "integer", "description": "First original year to include." },
        "yearTo": { "type": "integer", "description": "Last original year to include." },
    });
    vec![
        tool(
            "library_overview",
            "Library overview",
            "Size of the library: tracks, albums, album artists, genres, rated and loved tracks, and the catalog revision. Start here to get a sense of scale.",
            json!({}),
            &[],
        ),
        tool(
            "search_syntax",
            "Search syntax guide",
            "The full guide to Aurora's search language (fields, ranges, AND/OR/NOT, sort: and group:). Read it before building queries for search_songs or search_albums.",
            json!({}),
            &[],
        ),
        tool(
            "search_songs",
            "Search songs",
            "Find tracks with Aurora's search language. Returns title, artist, album, albumId, years, genre, rating, loved, and the user's own plays. Paged.",
            paging_properties(year_range.clone()),
            &[],
        ),
        tool(
            "search_albums",
            "Search albums",
            "Find albums with Aurora's search language. Returns albumId, title, artist, years, genre, publisher, country, track counts, rated and loved counts, and album rating. Paged.",
            paging_properties(year_range),
            &[],
        ),
        tool(
            "search_artists",
            "Search artists",
            "Find album artists by name, optionally within one genre, with their track counts and the user's plays. Paged.",
            paging_properties(json!({
                "genre": { "type": "string", "description": "Exact canonical genre name." },
                "sort": {
                    "type": "string",
                    "enum": ["nameAsc", "nameDesc", "trackCountAsc", "trackCountDesc"],
                },
            })),
            &[],
        ),
        tool(
            "album_detail",
            "Album detail",
            "One album with its full track list (titles, track numbers, ratings, loved, genres).",
            json!({ "albumId": { "type": "string", "description": "albumId from a search result." } }),
            &["albumId"],
        ),
        tool(
            "artist_detail",
            "Artist detail",
            "An album artist's albums in the library and the user's listening history for them.",
            json!({ "artist": { "type": "string", "description": "Album artist name, as in search results." } }),
            &["artist"],
        ),
        tool(
            "list_genres",
            "List genres",
            "Canonical genres with track, album, and artist counts, average rating, loved tracks, year span, and the user's plays. Filter by a name fragment and sort.",
            json!({
                "contains": { "type": "string", "description": "Case-insensitive part of the genre name." },
                "sort": {
                    "type": "string",
                    "enum": ["tracks", "albums", "artists", "rating", "loved", "plays", "name"],
                    "description": "Default: tracks.",
                },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "Default 50." },
            }),
            &[],
        ),
        tool(
            "genre_detail",
            "Genre detail",
            "One genre: its decades, top albums and artists, related genres, and highlight tracks.",
            json!({ "genre": { "type": "string", "description": "Exact canonical genre name, from list_genres." } }),
            &["genre"],
        ),
        tool(
            "genre_tag_variants",
            "Genre tag variants",
            "Raw genre tag spellings in the MP3s behind each canonical genre, with track counts. Use it to find inconsistent genre tags. Without a genre, lists only canonical genres that have more than one spelling. Reflects Music Library's last import.",
            json!({
                "genre": { "type": "string", "description": "Optional canonical genre to inspect." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "Rows, default 100." },
            }),
            &[],
        ),
        tool(
            "albums_with_mixed_genres",
            "Albums with mixed genres",
            "Albums whose tracks carry more than one canonical genre, with the genres and how many tracks have each. Optionally only albums that include one genre. Paged by offset. Reflects Music Library's last import.",
            json!({
                "genre": { "type": "string", "description": "Only albums with at least one track in this canonical genre." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Default 25." },
                "offset": { "type": "integer", "minimum": 0, "description": "Default 0." },
            }),
            &[],
        ),
        tool(
            "albums_with_multiple_genres",
            "Albums with more than one genre",
            "Albums that break a one-genre-per-album rule, read from the genre tags as written: a tag holding several genres joined by ';' (\"Electro; Dance\") or tracks whose tags differ. Each album lists its tag values and track counts, the genres after splitting, and primaryGenre, the genre listed first on most tracks. `total` counts every matching album. Paged by offset. Reflects Music Library's last import plus Aurora's own edits once Music Library imports them.",
            json!({
                "genre": { "type": "string", "description": "Only albums whose canonical album genre is this." },
                "tagContains": { "type": "string", "description": "Only albums with a track whose raw genre tag contains this text, for example \"Dance\"." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Default 25." },
                "offset": { "type": "integer", "minimum": 0, "description": "Default 0." },
            }),
            &[],
        ),
        control_tool(
            "show_in_aurora",
            "Show in Aurora",
            "Opens a search in the running Aurora window: the Songs, Albums or Artists view with this query in the search box. Use Aurora's query language, for example `genres:2..` (albums with more than one genre) or `genre:electro AND genres:2..`. Returns how many results the view shows. Needs Aurora running with assistant control on.",
            json!({
                "view": { "type": "string", "enum": ["songs", "albums", "artists"] },
                "query": { "type": "string", "description": "Aurora search text, with explicit AND between field terms." },
            }),
            &["view", "query"],
        ),
        control_tool(
            "propose_genre_changes",
            "Propose genre changes",
            "Sends genre changes for whole albums to Aurora for the user to review. Nothing is written until the user applies them in Aurora; they can edit or untick albums first. Each change sets every track of the album to one genre, through Aurora's verified tag writer, which keeps an undo journal and updates Music Library. Use album IDs from Aurora's tools unchanged. Up to 500 albums per proposal. Returns a proposalId for proposal_status.",
            json!({
                "changes": {
                    "type": "array", "minItems": 1, "maxItems": 500,
                    "items": {
                        "type": "object",
                        "properties": {
                            "albumId": { "type": "string" },
                            "genre": { "type": "string", "description": "The one genre the album's tracks get, for example its primaryGenre." },
                        },
                        "required": ["albumId", "genre"],
                        "additionalProperties": false,
                    },
                },
                "reason": { "type": "string", "description": "One sentence shown to the user, for example \"Keep only the first genre so each album has one genre.\"" },
            }),
            &["changes"],
        ),
        tool(
            "proposal_status",
            "Proposal status",
            "Whether the user applied, edited, skipped or dismissed a proposal, album by album, with any errors.",
            json!({ "proposalId": { "type": "string" } }),
            &["proposalId"],
        ),
        tool(
            "listening_report",
            "Listening report",
            "The user's listening across all their devices for a date range: totals, top artists, albums, and tracks, genres, decades, and discovery. Dates are inclusive YYYY-MM-DD; omit both for all time.",
            json!({
                "from": { "type": "string", "description": "First day, YYYY-MM-DD." },
                "to": { "type": "string", "description": "Last day, YYYY-MM-DD." },
                "utcOffsetMinutes": {
                    "type": "integer", "minimum": -1440, "maximum": 1440,
                    "description": "The user's UTC offset for day boundaries, for example 60 for CET. Default 0.",
                },
            }),
            &[],
        ),
        tool(
            "ratings_overview",
            "Ratings overview",
            "How the library is rated: tracks and albums per star band, rating completeness, and five-star albums.",
            json!({}),
            &[],
        ),
        tool(
            "year_overview",
            "Year overview",
            "Tracks and albums per year and decade across the library.",
            json!({}),
            &[],
        ),
    ]
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

fn run_tool(
    context: &Context,
    client: &str,
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    let arguments = arguments.as_object().cloned().unwrap_or_default();
    match name {
        "albums_with_multiple_genres" => albums_with_multiple_genres(&arguments),
        "show_in_aurora" => show_in_aurora(context, &arguments),
        "propose_genre_changes" => control::request(
            &context.state_directory,
            "proposeGenreChanges",
            json!({
                "changes": arguments.get("changes").cloned().unwrap_or(Value::Null),
                "reason": optional_text(&arguments, "reason").unwrap_or_default(),
                "source": client,
            }),
        ),
        "proposal_status" => {
            let id = required_text(&arguments, "proposalId")?;
            control::request(
                &context.state_directory,
                "proposalStatus",
                json!({ "proposalId": id }),
            )
        }
        "library_overview" => library_overview(),
        "search_syntax" => Ok(json!({ "guide": SEARCH_GUIDE })),
        "search_songs" => search_songs(context, &arguments),
        "search_albums" => search_albums(context, &arguments),
        "search_artists" => search_artists(context, &arguments),
        "album_detail" => {
            let album_id = required_text(&arguments, "albumId")?;
            to_value(explorer::load_local_album_detail(album_id, &context.store)?)
        }
        "artist_detail" => {
            let artist = required_text(&arguments, "artist")?;
            let detail = explorer::load_artist_detail(artist.clone(), &context.store)?;
            let listening = context.history.artist_listening(&artist).ok();
            Ok(json!({ "detail": to_value(detail)?, "listening": to_value(listening)? }))
        }
        "list_genres" => list_genres(context, &arguments),
        "genre_detail" => {
            let genre = required_text(&arguments, "genre")?;
            to_value(genres::load_genre_detail(
                genre,
                &context.history,
                &context.store,
            )?)
        }
        "genre_tag_variants" => genre_tag_variants(&arguments),
        "albums_with_mixed_genres" => albums_with_mixed_genres(&arguments),
        "listening_report" => listening_report(context, &arguments),
        "ratings_overview" => {
            let mut value = to_value(ratings::load_ratings_overview(&context.store)?)?;
            if let Some(object) = value.as_object_mut() {
                object.remove("initialPage");
            }
            Ok(value)
        }
        "year_overview" => to_value(years::load_year_overview()?),
        _ => Err(format!("Aurora has no tool named {name}.")),
    }
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("Could not encode the result: {error}"))
}

fn required_text(arguments: &Map<String, Value>, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} is required."))
}

fn optional_text(arguments: &Map<String, Value>, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn bounded_integer(
    arguments: &Map<String, Value>,
    key: &str,
    default: u64,
    max: u64,
) -> Result<u64, String> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => value
            .as_u64()
            .filter(|number| (1..=max).contains(number))
            .ok_or_else(|| format!("{key} must be a whole number from 1 to {max}.")),
    }
}

/// Builds an explorer page request from tool arguments, keeping only the
/// fields the request type accepts.
fn page_request<T: serde::de::DeserializeOwned>(
    arguments: &Map<String, Value>,
    allowed: &[(&str, &str)],
) -> Result<T, String> {
    let mut request = Map::new();
    request.insert(
        "pageSize".to_owned(),
        json!(bounded_integer(
            arguments,
            "pageSize",
            DEFAULT_PAGE_SIZE,
            MAX_PAGE_SIZE
        )?),
    );
    if let Some(query) = optional_text(arguments, "query") {
        request.insert("search".to_owned(), json!(query));
    }
    if let Some(cursor) = arguments.get("cursor").filter(|value| value.is_object()) {
        request.insert("cursor".to_owned(), cursor.clone());
    }
    for (argument, field) in allowed {
        if let Some(value) = arguments.get(*argument).filter(|value| !value.is_null()) {
            request.insert((*field).to_owned(), value.clone());
        }
    }
    serde_json::from_value(Value::Object(request))
        .map_err(|error| format!("Invalid search arguments: {error}"))
}

fn search_songs(context: &Context, arguments: &Map<String, Value>) -> Result<Value, String> {
    let request = page_request(
        arguments,
        &[
            ("genre", "genre"),
            ("artist", "artist"),
            ("yearFrom", "yearFrom"),
            ("yearTo", "yearTo"),
        ],
    )?;
    let registered_plays = context.history.registered_plays_by_track()?;
    to_value(explorer::load_track_page(
        request,
        &context.store,
        true,
        &registered_plays,
    )?)
}

fn search_albums(context: &Context, arguments: &Map<String, Value>) -> Result<Value, String> {
    let request = page_request(
        arguments,
        &[
            ("genre", "genre"),
            ("artist", "artist"),
            ("yearFrom", "yearFrom"),
            ("yearTo", "yearTo"),
        ],
    )?;
    to_value(explorer::load_album_page(request, &context.store, true)?)
}

fn search_artists(context: &Context, arguments: &Map<String, Value>) -> Result<Value, String> {
    let request = page_request(arguments, &[("genre", "genre"), ("sort", "sort")])?;
    let mut page = explorer::load_artist_page(request, &context.store, true)?;
    let insights = context.history.artist_insights().unwrap_or_default();
    for artist in &mut page.items {
        let insight = insights.get(&crate::history::artist_identity(&artist.id));
        artist.play_count = Some(insight.map_or(0, |insight| insight.plays));
        artist.last_played_at_ms = insight.and_then(|insight| insight.last_played_at_ms);
    }
    to_value(page)
}

fn library_overview() -> Result<Value, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    let count = |sql: &str| -> Result<i64, String> {
        connection
            .query_row(sql, [], |row| row.get(0))
            .map_err(|error| format!("Could not count the catalog: {error}"))
    };
    Ok(json!({
        "tracks": count("SELECT COUNT(*) FROM tracks")?,
        "albums": count("SELECT COUNT(*) FROM albums")?,
        "albumArtists": count("SELECT COUNT(DISTINCT album_artist_display) FROM albums")?,
        "genres": count("SELECT COUNT(DISTINCT canonical_genre) FROM albums WHERE NULLIF(TRIM(canonical_genre), '') IS NOT NULL")?,
        "ratedTracks": count("SELECT COUNT(*) FROM tracks WHERE normalized_rating IS NOT NULL")?,
        "lovedTracks": count("SELECT COUNT(*) FROM tracks WHERE love = 'L'")?,
        "catalogRevision": catalog::completed_import_revision()?,
        "auroraVersion": env!("CARGO_PKG_VERSION"),
    }))
}

fn list_genres(context: &Context, arguments: &Map<String, Value>) -> Result<Value, String> {
    let limit = bounded_integer(arguments, "limit", 50, 200)? as usize;
    let contains = optional_text(arguments, "contains").map(|value| value.to_lowercase());
    let sort = optional_text(arguments, "sort").unwrap_or_else(|| "tracks".to_owned());
    let mut items = genres::load_genre_index(&context.history, &context.store)?;
    let total_genres = items.len();
    if let Some(fragment) = &contains {
        items.retain(|genre| genre.name.to_lowercase().contains(fragment));
    }
    let matching = items.len();
    match sort.as_str() {
        "tracks" => items.sort_by_key(|genre| std::cmp::Reverse(genre.track_count)),
        "albums" => items.sort_by_key(|genre| std::cmp::Reverse(genre.album_count)),
        "artists" => items.sort_by_key(|genre| std::cmp::Reverse(genre.artist_count)),
        "loved" => items.sort_by_key(|genre| std::cmp::Reverse(genre.loved_tracks)),
        "plays" => items.sort_by_key(|genre| std::cmp::Reverse(genre.plays)),
        "rating" => items.sort_by(|left, right| {
            right
                .average_rating
                .unwrap_or(0.0)
                .total_cmp(&left.average_rating.unwrap_or(0.0))
        }),
        "name" => items.sort_by_key(|genre| genre.name.to_lowercase()),
        other => return Err(format!("Unknown genre sort {other}.")),
    }
    items.truncate(limit);
    Ok(json!({
        "totalGenres": total_genres,
        "matchingGenres": matching,
        "genres": to_value(items)?,
    }))
}

fn genre_tag_variants(arguments: &Map<String, Value>) -> Result<Value, String> {
    let limit = bounded_integer(arguments, "limit", 100, 200)? as i64;
    let genre = optional_text(arguments, "genre");
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    let mut statement = connection
        .prepare(
            r#"
            WITH variants AS (
              SELECT TRIM(canonical_genre) AS canonical, TRIM(genre) AS raw, COUNT(*) AS tracks
              FROM tracks
              WHERE NULLIF(TRIM(canonical_genre), '') IS NOT NULL
                AND NULLIF(TRIM(genre), '') IS NOT NULL
                AND (?1 IS NULL OR TRIM(canonical_genre) = ?1 COLLATE NOCASE)
              GROUP BY 1, 2
            )
            SELECT canonical, raw, tracks FROM variants
            WHERE ?1 IS NOT NULL
               OR canonical IN (SELECT canonical FROM variants GROUP BY canonical HAVING COUNT(*) > 1)
            ORDER BY canonical COLLATE NOCASE, tracks DESC
            LIMIT ?2
            "#,
        )
        .map_err(|error| format!("Could not prepare the genre check: {error}"))?;
    let rows = statement
        .query_map(rusqlite::params![genre, limit], |row| {
            Ok(json!({
                "canonicalGenre": row.get::<_, String>(0)?,
                "tagValue": row.get::<_, String>(1)?,
                "tracks": row.get::<_, i64>(2)?,
            }))
        })
        .map_err(|error| format!("Could not run the genre check: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read the genre check: {error}"))?;
    Ok(json!({
        "variants": rows,
        "limitReached": rows.len() as i64 == limit,
    }))
}

fn albums_with_mixed_genres(arguments: &Map<String, Value>) -> Result<Value, String> {
    let limit = bounded_integer(arguments, "limit", 25, 100)? as i64;
    let offset = arguments
        .get("offset")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .unwrap_or(0);
    let genre = optional_text(arguments, "genre");
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    let mut statement = connection
        .prepare(
            r#"
            WITH per_album AS (
              SELECT album_id, TRIM(canonical_genre) AS genre, COUNT(*) AS tracks
              FROM tracks
              WHERE album_id IS NOT NULL AND NULLIF(TRIM(canonical_genre), '') IS NOT NULL
              GROUP BY album_id, TRIM(canonical_genre)
            ),
            mixed AS (
              SELECT album_id,
                     json_group_array(json_object('genre', genre, 'tracks', tracks)) AS genres
              FROM per_album
              GROUP BY album_id
              HAVING COUNT(*) > 1
                 AND (?1 IS NULL OR SUM(genre = ?1 COLLATE NOCASE) > 0)
            )
            SELECT a.id, a.album, a.album_artist_display, a.year, a.canonical_genre, m.genres
            FROM mixed m JOIN albums a ON a.id = m.album_id
            ORDER BY a.album_artist_display COLLATE NOCASE, a.album COLLATE NOCASE, a.id
            LIMIT ?2 OFFSET ?3
            "#,
        )
        .map_err(|error| format!("Could not prepare the mixed-genre check: {error}"))?;
    let rows = statement
        .query_map(rusqlite::params![genre, limit, offset], |row| {
            let genres: String = row.get(5)?;
            Ok(json!({
                "albumId": row.get::<_, String>(0)?,
                "title": row.get::<_, Option<String>>(1)?,
                "artist": row.get::<_, Option<String>>(2)?,
                "year": row.get::<_, Option<i64>>(3)?,
                "albumGenre": row.get::<_, Option<String>>(4)?,
                "trackGenres": serde_json::from_str::<Value>(&genres).unwrap_or(Value::Null),
            }))
        })
        .map_err(|error| format!("Could not run the mixed-genre check: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read the mixed-genre check: {error}"))?;
    let next_offset = (rows.len() as i64 == limit).then_some(offset + limit);
    Ok(json!({ "albums": rows, "nextOffset": next_offset }))
}

fn albums_with_multiple_genres(arguments: &Map<String, Value>) -> Result<Value, String> {
    let limit = bounded_integer(arguments, "limit", 25, 100)? as i64;
    let offset = arguments
        .get("offset")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .unwrap_or(0);
    let genre = optional_text(arguments, "genre");
    let tag = optional_text(arguments, "tagContains");
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    let (albums, total) = genre_rule::albums_with_multiple_genres(
        &connection,
        genre.as_deref(),
        tag.as_deref(),
        limit,
        offset,
    )?;
    let next_offset = (offset + (albums.len() as i64) < total).then_some(offset + limit);
    Ok(json!({ "albums": to_value(albums)?, "total": total, "nextOffset": next_offset }))
}

fn show_in_aurora(context: &Context, arguments: &Map<String, Value>) -> Result<Value, String> {
    let view = required_text(arguments, "view")?;
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    // Run the search first: a query Aurora cannot parse fails here, where the
    // assistant can fix it, instead of only in the user's search box.
    let mut probe = Map::new();
    probe.insert("query".to_owned(), json!(query));
    probe.insert("pageSize".to_owned(), json!(1));
    let page = match view.as_str() {
        "songs" => search_songs(context, &probe)?,
        "albums" => search_albums(context, &probe)?,
        "artists" => search_artists(context, &probe)?,
        _ => return Err("view must be songs, albums or artists.".to_owned()),
    };
    control::request(
        &context.state_directory,
        "show",
        json!({ "view": view, "query": query }),
    )?;
    Ok(json!({
        "shown": true,
        "view": view,
        "query": query,
        "totalCount": page.get("totalCount").or_else(|| page.get("total")).cloned(),
    }))
}

fn listening_report(context: &Context, arguments: &Map<String, Value>) -> Result<Value, String> {
    let offset = arguments
        .get("utcOffsetMinutes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if !(-1_440..=1_440).contains(&offset) {
        return Err("utcOffsetMinutes must be between -1440 and 1440.".to_owned());
    }
    let offset_ms = offset * 60_000;
    let from = optional_text(arguments, "from")
        .map(|day| parse_day(&day).map(|ms| ms - offset_ms))
        .transpose()?;
    let to = optional_text(arguments, "to")
        .map(|day| parse_day(&day).map(|ms| ms + 86_400_000 - 1 - offset_ms))
        .transpose()?;
    let request = serde_json::from_value(json!({
        "startedAfterMs": from,
        "startedBeforeMs": to,
        "timezoneOffsetMinutes": offset,
    }))
    .map_err(|error| format!("Invalid report range: {error}"))?;
    let mut value = to_value(context.history.report(request, &context.store)?)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("previousSummary");
        object.remove("previousDaily");
    }
    Ok(value)
}

/// Milliseconds since the Unix epoch at UTC midnight of a `YYYY-MM-DD` day.
fn parse_day(value: &str) -> Result<i64, String> {
    let invalid = || format!("{value} is not a date in YYYY-MM-DD form.");
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid());
    };
    let year: i64 = year.parse().map_err(|_| invalid())?;
    let month: i64 = month.parse().map_err(|_| invalid())?;
    let day: i64 = day.parse().map_err(|_| invalid())?;
    if !(1970..=9999).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(invalid());
    }
    // Howard Hinnant's days_from_civil.
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = shifted_year.div_euclid(400);
    let year_of_era = shifted_year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Ok(days * 86_400_000)
}

// ---------------------------------------------------------------------------
// Result hygiene
// ---------------------------------------------------------------------------

/// Keys that hold file-system locations, path-derived keys, or UI-only state.
const HIDDEN_KEYS: [&str; 14] = [
    "path",
    "filePath",
    "directory",
    "filename",
    "folder",
    "albumPath",
    "coverPath",
    "cachePath",
    "sourcePath",
    "trackKey",
    "trackKeys",
    "canUndoTagEdit",
    "tagSyncState",
    "naming",
];

/// Removes file paths, path-derived keys, nulls, and Last.fm's global play
/// count (which must never be read as the user's plays), and caps arrays.
fn sanitize(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, item| {
                !HIDDEN_KEYS.contains(&key.as_str()) && !item.is_null() && !looks_like_path(item)
            });
            // A track (it has loveState) carries Last.fm's worldwide scrobbles
            // as playCount; its own plays are registeredPlays.
            if object.contains_key("loveState") {
                object.remove("playCount");
                object.remove("loveState");
            }
            for item in object.values_mut() {
                sanitize(item);
            }
        }
        Value::Array(items) => {
            if items.len() > MAX_ARRAY_ITEMS {
                items.truncate(MAX_ARRAY_ITEMS);
            }
            for item in items.iter_mut() {
                sanitize(item);
            }
        }
        _ => {}
    }
}

fn looks_like_path(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let bytes = text.as_bytes();
    let drive = bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    drive
        || text.starts_with("\\\\")
        || text.starts_with("/Users/")
        || text.starts_with("/Volumes/")
        || text.starts_with("/home/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_convert_to_utc_midnight() {
        assert_eq!(parse_day("1970-01-01").unwrap(), 0);
        assert_eq!(parse_day("2026-08-01").unwrap(), 1_785_542_400_000);
        assert!(parse_day("2026-13-01").is_err());
        assert!(parse_day("yesterday").is_err());
    }

    #[test]
    fn sanitize_removes_paths_track_keys_and_global_plays() {
        let mut value = json!({
            "items": [{
                "id": "42",
                "title": "Song",
                "trackKey": "d:\\music\\a\\01.mp3",
                "loveState": "none",
                "playCount": 900_000,
                "registeredPlays": 3,
                "genre": null,
                "note": "D:\\MUSIC\\Album",
            }],
            "nextCursor": null,
        });
        sanitize(&mut value);
        assert_eq!(
            value,
            json!({ "items": [{ "id": "42", "title": "Song", "registeredPlays": 3 }] })
        );
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_fail() {
        let mut server = Server::default();
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
        let reply = server
            .handle_line(r#"{"jsonrpc":"2.0","id":7,"method":"resources/list"}"#)
            .unwrap();
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["error"]["code"], -32601);
    }

    #[test]
    fn initialize_negotiates_a_supported_protocol() {
        let mut server = Server::default();
        let reply = server
            .handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#)
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(reply["result"]["serverInfo"]["name"], "aurora");
        let reply = server
            .handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#)
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], SUPPORTED_PROTOCOLS[0]);
    }

    #[test]
    fn only_control_tools_act_and_every_tool_is_closed() {
        for tool in tool_definitions() {
            let name = tool["name"].as_str().unwrap();
            assert_eq!(
                tool["annotations"]["readOnlyHint"],
                !CONTROL_TOOLS.contains(&name),
                "{name}"
            );
            assert_eq!(tool["annotations"]["destructiveHint"], false, "{name}");
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        }
    }
}
