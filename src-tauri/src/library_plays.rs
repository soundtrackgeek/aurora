//! Sends Aurora's registered plays to Music Library's listening history
//! through the `recordPlays` bridge operation.
//!
//! Plays from every history source are sent (this device, other devices'
//! OneDrive snapshots, and Tonehavn), so the home PC's Music Library catalog
//! holds the complete history. Each device has its own cursor because a peer
//! snapshot can arrive long after its plays happened; every request re-reads
//! [`OVERLAP_MS`] behind the cursor, and Music Library ignores plays it
//! already has. Only the home PC sends: Network Mode cannot write to the
//! library, and Laptop Mode works on a copied catalog.

use crate::history::{HistoryStore, RegisteredPlay};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const EXPORT_INTERVAL_SECONDS: u64 = 120;
const BATCH_SIZE: usize = 1_000;
const MAX_BATCHES_PER_RUN: usize = 50;
const OVERLAP_MS: i64 = 6 * 60 * 60 * 1_000;
const RETRY_AFTER_ERROR_MS: i64 = 60 * 60 * 1_000;
const STATE_FILE: &str = "aurora-library-plays.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayExportState {
    #[serde(default)]
    pub(crate) cursors: HashMap<String, i64>,
    #[serde(default)]
    pub(crate) sent_plays: u64,
    #[serde(default)]
    pub(crate) last_exported_at_ms: Option<i64>,
    #[serde(default)]
    pub(crate) last_error: Option<String>,
    #[serde(default)]
    pub(crate) retry_after_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayExportStatus {
    pub(crate) sent_plays: u64,
    pub(crate) last_exported_at_ms: Option<i64>,
    pub(crate) last_error: Option<String>,
}

pub(crate) fn state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(STATE_FILE)
}

pub(crate) fn load_state(path: &Path) -> PlayExportState {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_state(path: &Path, state: &PlayExportState) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("Could not encode the Music Library play export: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|error| format!("Could not save the Music Library play export: {error}"))
}

pub(crate) fn status(state: &PlayExportState) -> PlayExportStatus {
    PlayExportStatus {
        sent_plays: state.sent_plays,
        last_exported_at_ms: state.last_exported_at_ms,
        last_error: state.last_error.clone(),
    }
}

fn is_new(play: &RegisteredPlay, cursors: &HashMap<String, i64>) -> bool {
    cursors
        .get(&play.device_id)
        .is_none_or(|cursor| play.started_at_ms > *cursor)
}

fn file_path(play: &RegisteredPlay) -> Option<String> {
    if play.directory.trim().is_empty() || play.filename.trim().is_empty() {
        return None;
    }
    let separator = if play.directory.contains('\\') {
        '\\'
    } else {
        '/'
    };
    Some(format!(
        "{}{separator}{}",
        play.directory.trim_end_matches(['\\', '/']),
        play.filename
    ))
}

pub(crate) fn payload(plays: &[RegisteredPlay]) -> Value {
    let plays = plays
        .iter()
        .map(|play| {
            json!({
                "artist": play.artist,
                "title": play.title,
                "album": (!play.album.trim().is_empty()).then_some(&play.album),
                "playedAt": play.started_at_ms.div_euclid(1_000),
                "filePath": file_path(play),
            })
        })
        .collect::<Vec<_>>();
    json!({ "plays": plays })
}

fn friendly_error(error: String) -> String {
    if error.contains("Unknown Aurora bridge operation") {
        "Update Music Library to 0.187.0 or later to receive Aurora plays.".to_owned()
    } else {
        error
    }
}

/// Sends every play newer than the device cursors, in batches. Cursors move
/// only after Music Library accepted a batch, so a failure resends it later.
pub(crate) fn export_pending(
    history: &HistoryStore,
    state: &mut PlayExportState,
    now_ms: i64,
    mut send: impl FnMut(Value) -> Result<Value, String>,
) -> Result<u64, String> {
    let mut sent = 0;
    for _ in 0..MAX_BATCHES_PER_RUN {
        let plays = history.registered_plays_after(&state.cursors, OVERLAP_MS, BATCH_SIZE)?;
        let new_plays = plays
            .iter()
            .filter(|play| is_new(play, &state.cursors))
            .count() as u64;
        if new_plays == 0 {
            break;
        }
        if let Err(error) = send(payload(&plays)) {
            let error = friendly_error(error);
            state.last_error = Some(error.clone());
            state.retry_after_ms = Some(now_ms + RETRY_AFTER_ERROR_MS);
            return Err(error);
        }
        for play in &plays {
            let cursor = state
                .cursors
                .entry(play.device_id.clone())
                .or_insert(i64::MIN);
            *cursor = (*cursor).max(play.started_at_ms);
        }
        sent += new_plays;
        state.sent_plays += new_plays;
        state.last_exported_at_ms = Some(now_ms);
        state.last_error = None;
        state.retry_after_ms = None;
    }
    Ok(sent)
}

/// One scheduled pass. Returns without work when this device must not write
/// to Music Library or a recent failure is still backing off.
pub(crate) fn run_scheduled(
    history: &HistoryStore,
    app_data_dir: &Path,
    now_ms: i64,
    send: impl FnMut(Value) -> Result<Value, String>,
) -> Result<(), String> {
    if crate::connections::network_mode() || crate::device_mode::laptop_mode_enabled() {
        return Ok(());
    }
    let path = state_path(app_data_dir);
    let mut state = load_state(&path);
    if state.retry_after_ms.is_some_and(|retry| retry > now_ms) {
        return Ok(());
    }
    let result = export_pending(history, &mut state, now_ms, send);
    save_state(&path, &state)?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests;
