//! Lets AI assistants drive the running Aurora window.
//!
//! Aurora's MCP server runs as its own process, started by Claude Code or
//! Codex in a terminal or by the in-app Assistant. To show a view or queue tag
//! changes it connects to this channel: a listener on 127.0.0.1 that the
//! running app opens at startup. The port and a random token are written to
//! `aurora-control.json` in Aurora's app-data folder, which only the user can
//! read, and every request must carry the token.
//!
//! Nothing here writes tags. `proposeGenreChanges` only stores a proposal and
//! shows it; the user reviews and applies it in Aurora, through the same
//! verified tag writer as the tag editor.

use crate::{assistant, catalog, genre_rule};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

pub(crate) const CONTROL_FILE: &str = "aurora-control.json";
pub(crate) const SHOW_EVENT: &str = "assistant://show";
pub(crate) const PROPOSAL_EVENT: &str = "assistant://proposal";
const MAX_REQUEST_BYTES: u64 = 512 * 1024;
const MAX_PROPOSAL_ALBUMS: usize = 500;
const MAX_STORED_PROPOSALS: usize = 20;
const MAX_QUERY_CHARS: usize = 2000;
const MAX_GENRE_CHARS: usize = 256;

pub(crate) const CONTROL_OFF: &str = "Aurora control is off. Turn on Settings → Connections → \
AI assistant → \"Let AI assistants control Aurora\" in Aurora, then try again.";
pub(crate) const NOT_RUNNING: &str =
    "Aurora is not running. Open Aurora, then try again. Read-only library tools still work.";

#[derive(Debug, Deserialize, Serialize)]
struct Endpoint {
    port: u16,
    token: String,
    pid: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    token: String,
    method: String,
    #[serde(default)]
    params: Value,
}

// ---------------------------------------------------------------------------
// Server (the running app)
// ---------------------------------------------------------------------------

/// Opens the control channel and publishes its endpoint. Failing to open it
/// only disables assistant control; Aurora itself keeps running.
pub(crate) fn start(app: AppHandle, directory: PathBuf) -> Result<(), String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("Could not open Aurora's assistant control channel: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the control channel's port: {error}"))?
        .port();
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    write_endpoint(
        &directory,
        &Endpoint {
            port,
            token: token.clone(),
            pid: std::process::id(),
        },
    )?;
    std::thread::Builder::new()
        .name("aurora-control".to_owned())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let app = app.clone();
                let token = token.clone();
                let directory = directory.clone();
                let _ = std::thread::Builder::new()
                    .name("aurora-control-request".to_owned())
                    .spawn(move || serve(&app, stream, &token, &directory));
            }
        })
        .map_err(|error| format!("Could not start the control channel: {error}"))?;
    Ok(())
}

fn write_endpoint(directory: &Path, endpoint: &Endpoint) -> Result<(), String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create Aurora's settings folder: {error}"))?;
    let path = directory.join(CONTROL_FILE);
    let staged = path.with_extension("json.tmp");
    let json = serde_json::to_string(endpoint)
        .map_err(|error| format!("Could not encode the control endpoint: {error}"))?;
    fs::write(&staged, json)
        .map_err(|error| format!("Could not write the control endpoint: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&staged, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&staged, &path)
        .map_err(|error| format!("Could not publish the control endpoint: {error}"))
}

fn serve(app: &AppHandle, stream: TcpStream, token: &str, directory: &Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut line = String::new();
    let read = BufReader::new(stream.take(MAX_REQUEST_BYTES)).read_line(&mut line);
    let response = match read {
        Ok(_) => match serde_json::from_str::<Request>(&line) {
            Ok(request) if tokens_match(&request.token, token) => {
                handle(app, directory, &request.method, &request.params)
            }
            Ok(_) => Err("Aurora refused the request.".to_owned()),
            Err(error) => Err(format!("Aurora could not read the request: {error}")),
        },
        Err(error) => Err(format!("Aurora could not read the request: {error}")),
    };
    let body = match response {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(message) => json!({ "ok": false, "error": message }),
    };
    let _ = writeln!(writer, "{body}").and_then(|()| writer.flush());
}

/// Compares in constant time, so the token cannot be guessed byte by byte.
fn tokens_match(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |difference, (left, right)| difference | (left ^ right))
            == 0
}

fn handle(
    app: &AppHandle,
    directory: &Path,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    if method == "ping" {
        return Ok(json!({ "version": env!("CARGO_PKG_VERSION") }));
    }
    let settings = assistant::read_settings(directory);
    if !settings.library_access || !settings.app_control {
        return Err(CONTROL_OFF.to_owned());
    }
    let proposals = app.state::<AssistantProposals>();
    match method {
        "show" => {
            let request: ShowRequest = serde_json::from_value(params.clone())
                .map_err(|error| format!("Invalid view request: {error}"))?;
            request.validate()?;
            app.emit(SHOW_EVENT, &request)
                .map_err(|error| format!("Aurora could not change the view: {error}"))?;
            focus_window(app);
            Ok(json!({ "shown": true }))
        }
        "proposeGenreChanges" => {
            let request: ProposalRequest = serde_json::from_value(params.clone())
                .map_err(|error| format!("Invalid genre proposal: {error}"))?;
            let proposal = proposals.create(request)?;
            let _ = app.emit(PROPOSAL_EVENT, &proposal);
            focus_window(app);
            Ok(json!({
                "proposalId": proposal.id,
                "albums": proposal.albums.len(),
                "status": "waitingForReview",
                "note": "The user reviews the changes in Aurora and applies or dismisses them. \
                         Call proposal_status to see the outcome.",
            }))
        }
        "proposalStatus" => {
            let id = params
                .get("proposalId")
                .and_then(Value::as_str)
                .ok_or("proposalStatus needs a proposalId.")?;
            serde_json::to_value(proposals.get(id)?)
                .map_err(|error| format!("Could not encode the proposal: {error}"))
        }
        _ => Err(format!("Aurora's control channel has no method {method}.")),
    }
}

fn focus_window(app: &AppHandle) {
    if let Some(window) = app.webview_windows().into_values().next() {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ShowRequest {
    pub(crate) view: String,
    pub(crate) query: String,
}

impl ShowRequest {
    fn validate(&self) -> Result<(), String> {
        if !matches!(self.view.as_str(), "songs" | "albums" | "artists") {
            return Err("view must be songs, albums or artists.".to_owned());
        }
        if self.query.chars().count() > MAX_QUERY_CHARS {
            return Err("The query is too long.".to_owned());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Client (the MCP server process)
// ---------------------------------------------------------------------------

/// Sends one request to the running Aurora and returns its result.
pub(crate) fn request(directory: &Path, method: &str, params: Value) -> Result<Value, String> {
    let endpoint: Endpoint = fs::read_to_string(directory.join(CONTROL_FILE))
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .ok_or(NOT_RUNNING)?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|_| NOT_RUNNING.to_owned())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let body = json!({ "token": endpoint.token, "method": method, "params": params });
    writeln!(stream, "{body}")
        .and_then(|()| stream.flush())
        .map_err(|error| format!("Could not reach Aurora: {error}"))?;
    let mut line = String::new();
    BufReader::new(stream.take(4 * 1024 * 1024))
        .read_line(&mut line)
        .map_err(|error| format!("Aurora did not answer: {error}"))?;
    let reply: Value =
        serde_json::from_str(&line).map_err(|_| "Aurora sent an unreadable answer.".to_owned())?;
    if reply.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(reply.get("result").cloned().unwrap_or(Value::Null))
    } else {
        Err(reply
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Aurora refused the request.")
            .to_owned())
    }
}

// ---------------------------------------------------------------------------
// Genre proposals
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProposedChange {
    pub(crate) album_id: String,
    pub(crate) genre: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProposalRequest {
    pub(crate) changes: Vec<ProposedChange>,
    #[serde(default)]
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProposalStatus {
    Pending,
    Applying,
    Done,
    Dismissed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChangeStatus {
    Pending,
    Applied,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProposalAlbum {
    pub(crate) album_id: String,
    pub(crate) artist: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) year: Option<i64>,
    pub(crate) current_tags: Vec<genre_rule::TagCount>,
    pub(crate) proposed_genre: String,
    /// The genre the user applied, when they edited the proposal.
    pub(crate) applied_genre: Option<String>,
    pub(crate) status: ChangeStatus,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreProposal {
    pub(crate) id: String,
    pub(crate) created_at_ms: u64,
    pub(crate) source: String,
    pub(crate) reason: String,
    pub(crate) status: ProposalStatus,
    pub(crate) albums: Vec<ProposalAlbum>,
}

/// Proposals live in memory: they are short-lived requests for review.
#[derive(Default)]
pub(crate) struct AssistantProposals {
    proposals: Mutex<Vec<GenreProposal>>,
}

pub(crate) fn validate_genre(genre: &str) -> Result<String, String> {
    let genre = genre.trim();
    if genre.is_empty() {
        return Err("A proposed genre is empty.".to_owned());
    }
    if genre.chars().count() > MAX_GENRE_CHARS || genre.chars().any(char::is_control) {
        return Err(format!("\"{genre}\" is not a valid genre tag."));
    }
    Ok(genre.to_owned())
}

impl AssistantProposals {
    fn create(&self, request: ProposalRequest) -> Result<GenreProposal, String> {
        if request.changes.is_empty() {
            return Err("Propose at least one album change.".to_owned());
        }
        if request.changes.len() > MAX_PROPOSAL_ALBUMS {
            return Err(format!(
                "Propose at most {MAX_PROPOSAL_ALBUMS} albums at a time."
            ));
        }
        let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
        let mut albums: Vec<ProposalAlbum> = Vec::with_capacity(request.changes.len());
        let mut unknown = Vec::new();
        for change in request.changes {
            let genre = validate_genre(&change.genre)?;
            if albums.iter().any(|album| album.album_id == change.album_id) {
                return Err("Each album may appear only once in a proposal.".to_owned());
            }
            match genre_rule::load_album(&connection, &change.album_id)? {
                Some(album) => albums.push(ProposalAlbum {
                    album_id: album.album_id,
                    artist: album.artist,
                    title: album.title,
                    year: album.year,
                    current_tags: album.tags,
                    proposed_genre: genre,
                    applied_genre: None,
                    status: ChangeStatus::Pending,
                    error: None,
                }),
                None => unknown.push(change.album_id),
            }
        }
        if !unknown.is_empty() {
            return Err(format!(
                "Aurora has no albums with these IDs: {}. Use IDs from Aurora's tools unchanged.",
                unknown.join(", ")
            ));
        }
        let proposal = GenreProposal {
            id: uuid::Uuid::new_v4().to_string(),
            created_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_millis() as u64),
            source: truncate(request.source.trim(), 80),
            reason: truncate(request.reason.trim(), 2000),
            status: ProposalStatus::Pending,
            albums,
        };
        let mut proposals = self.lock()?;
        proposals.push(proposal.clone());
        while proposals.len() > MAX_STORED_PROPOSALS {
            let oldest_closed = proposals
                .iter()
                .position(|item| {
                    matches!(
                        item.status,
                        ProposalStatus::Done | ProposalStatus::Dismissed
                    )
                })
                .unwrap_or(0);
            proposals.remove(oldest_closed);
        }
        Ok(proposal)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Vec<GenreProposal>>, String> {
        self.proposals
            .lock()
            .map_err(|_| "Aurora's proposal list stopped unexpectedly.".to_owned())
    }

    pub(crate) fn list(&self) -> Result<Vec<GenreProposal>, String> {
        Ok(self.lock()?.clone())
    }

    pub(crate) fn get(&self, id: &str) -> Result<GenreProposal, String> {
        self.lock()?
            .iter()
            .find(|proposal| proposal.id == id)
            .cloned()
            .ok_or_else(|| "Aurora has no proposal with that ID.".to_owned())
    }

    fn update<T>(
        &self,
        id: &str,
        change: impl FnOnce(&mut GenreProposal) -> Result<T, String>,
    ) -> Result<(T, GenreProposal), String> {
        let mut proposals = self.lock()?;
        let proposal = proposals
            .iter_mut()
            .find(|proposal| proposal.id == id)
            .ok_or("Aurora has no proposal with that ID.")?;
        let value = change(proposal)?;
        Ok((value, proposal.clone()))
    }

    /// Checks the approved changes and marks the proposal as being applied.
    pub(crate) fn begin(
        &self,
        id: &str,
        approved: Vec<ProposedChange>,
    ) -> Result<(Vec<ProposedChange>, GenreProposal), String> {
        self.update(id, |proposal| {
            if proposal.status != ProposalStatus::Pending {
                return Err("This proposal is not waiting for review.".to_owned());
            }
            let mut checked = Vec::with_capacity(approved.len());
            for change in approved {
                let album = proposal
                    .albums
                    .iter()
                    .find(|album| album.album_id == change.album_id)
                    .ok_or("An approved album is not part of this proposal.")?;
                if !matches!(album.status, ChangeStatus::Pending | ChangeStatus::Failed) {
                    return Err("An approved album was already handled.".to_owned());
                }
                checked.push(ProposedChange {
                    album_id: change.album_id,
                    genre: validate_genre(&change.genre)?,
                });
            }
            if checked.is_empty() {
                return Err("Choose at least one album to change.".to_owned());
            }
            proposal.status = ProposalStatus::Applying;
            Ok(checked)
        })
    }

    pub(crate) fn record(
        &self,
        id: &str,
        change: &ProposedChange,
        outcome: Result<(), String>,
    ) -> Result<GenreProposal, String> {
        self.update(id, |proposal| {
            if let Some(album) = proposal
                .albums
                .iter_mut()
                .find(|album| album.album_id == change.album_id)
            {
                match outcome {
                    Ok(()) => {
                        album.status = ChangeStatus::Applied;
                        album.error = None;
                        album.applied_genre =
                            (change.genre != album.proposed_genre).then(|| change.genre.clone());
                    }
                    Err(error) => {
                        album.status = ChangeStatus::Failed;
                        album.error = Some(error);
                    }
                }
            }
            Ok(())
        })
        .map(|(_, proposal)| proposal)
    }

    /// Ends an apply run; albums left pending keep the proposal open.
    pub(crate) fn finish(&self, id: &str) -> Result<GenreProposal, String> {
        self.update(id, |proposal| {
            let open = proposal
                .albums
                .iter()
                .any(|album| matches!(album.status, ChangeStatus::Pending | ChangeStatus::Failed));
            proposal.status = if open {
                ProposalStatus::Pending
            } else {
                ProposalStatus::Done
            };
            Ok(())
        })
        .map(|(_, proposal)| proposal)
    }

    pub(crate) fn dismiss(&self, id: &str) -> Result<GenreProposal, String> {
        self.update(id, |proposal| {
            if proposal.status == ProposalStatus::Applying {
                return Err("Wait until the changes finish applying.".to_owned());
            }
            for album in &mut proposal.albums {
                if matches!(album.status, ChangeStatus::Pending | ChangeStatus::Failed) {
                    album.status = ChangeStatus::Skipped;
                }
            }
            if proposal.status == ProposalStatus::Pending {
                proposal.status = ProposalStatus::Dismissed;
            }
            Ok(())
        })
        .map(|(_, proposal)| proposal)
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal(albums: &[&str]) -> GenreProposal {
        GenreProposal {
            id: "p".to_owned(),
            created_at_ms: 0,
            source: String::new(),
            reason: String::new(),
            status: ProposalStatus::Pending,
            albums: albums
                .iter()
                .map(|id| ProposalAlbum {
                    album_id: (*id).to_owned(),
                    artist: None,
                    title: None,
                    year: None,
                    current_tags: Vec::new(),
                    proposed_genre: "Electro".to_owned(),
                    applied_genre: None,
                    status: ChangeStatus::Pending,
                    error: None,
                })
                .collect(),
        }
    }

    fn change(album_id: &str, genre: &str) -> ProposedChange {
        ProposedChange {
            album_id: album_id.to_owned(),
            genre: genre.to_owned(),
        }
    }

    #[test]
    fn tokens_compare_whole_values() {
        assert!(tokens_match("abc123", "abc123"));
        assert!(!tokens_match("abc124", "abc123"));
        assert!(!tokens_match("abc", "abc123"));
    }

    #[test]
    fn proposals_track_each_album_until_reviewed() {
        let store = AssistantProposals::default();
        store.lock().unwrap().push(proposal(&["a", "b", "c"]));
        assert!(store.begin("p", vec![change("x", "Rock")]).is_err());
        let (checked, state) = store
            .begin("p", vec![change("a", " Electro "), change("b", "Dance")])
            .unwrap();
        assert_eq!(state.status, ProposalStatus::Applying);
        assert_eq!(checked[0].genre, "Electro");
        assert!(store.begin("p", vec![change("c", "Rock")]).is_err());
        store.record("p", &checked[0], Ok(())).unwrap();
        let state = store
            .record("p", &checked[1], Err("Locked".into()))
            .unwrap();
        assert_eq!(state.albums[0].status, ChangeStatus::Applied);
        assert_eq!(state.albums[0].applied_genre, None);
        assert_eq!(state.albums[1].status, ChangeStatus::Failed);
        assert_eq!(store.finish("p").unwrap().status, ProposalStatus::Pending);
        let dismissed = store.dismiss("p").unwrap();
        assert_eq!(dismissed.status, ProposalStatus::Dismissed);
        assert_eq!(dismissed.albums[2].status, ChangeStatus::Skipped);
        assert_eq!(dismissed.albums[0].status, ChangeStatus::Applied);
    }

    #[test]
    fn genres_and_views_are_checked() {
        assert!(validate_genre("  ").is_err());
        assert!(validate_genre("Rock\n").is_ok());
        assert!(validate_genre("Ro\u{0}ck").is_err());
        assert_eq!(validate_genre(" Films/Games ").unwrap(), "Films/Games");
        let show = |view: &str| ShowRequest {
            view: view.to_owned(),
            query: "genres:2..".to_owned(),
        };
        assert!(show("albums").validate().is_ok());
        assert!(show("settings").validate().is_err());
    }

    #[test]
    fn a_missing_endpoint_means_aurora_is_not_running() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            request(directory.path(), "ping", Value::Null).unwrap_err(),
            NOT_RUNNING
        );
    }
}
