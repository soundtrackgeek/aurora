use crate::{
    catalog::{default_catalog_path, open_catalog},
    musicbrainz::{
        ArtistIntelligence, default_source_paths, load_artist_intelligence_with_store, valid_mbid,
        validate_artist,
    },
    state_store::StateStore,
};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const MAX_ALBUM_ID_CHARS: usize = 512;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ArtistDecisionRequest {
    Confirm { artist: String, mbid: String },
    Ignore { artist: String },
    Clear { artist: String },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ReleaseDecisionRequest {
    Link {
        artist: String,
        artist_mbid: String,
        release_mbid: String,
        local_album_id: String,
    },
    NotInScope {
        artist: String,
        artist_mbid: String,
        release_mbid: String,
    },
    Ignore {
        artist: String,
        artist_mbid: String,
        release_mbid: String,
    },
    Clear {
        artist: String,
        artist_mbid: String,
        release_mbid: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurationExportResult {
    path: String,
    artist_decisions: usize,
    release_decisions: usize,
    source_rows_preserved: bool,
}

pub(crate) fn update_artist_decision(
    store: &StateStore,
    request: ArtistDecisionRequest,
) -> Result<ArtistIntelligence, String> {
    match request {
        ArtistDecisionRequest::Confirm { artist, mbid } => {
            let (display_artist, artist_key) = validate_artist(&artist)?;
            let mbid = mbid.trim().to_lowercase();
            if !valid_mbid(&mbid) {
                return Err("Choose a valid MusicBrainz artist candidate.".to_owned());
            }
            let current = load_artist_intelligence_with_store(display_artist.clone(), store)?;
            let candidate = current
                .candidates
                .iter()
                .find(|candidate| candidate.mbid.eq_ignore_ascii_case(&mbid))
                .ok_or_else(|| {
                    "That MusicBrainz candidate is no longer available. Reload the artist before confirming it."
                        .to_owned()
                })?;
            if let Some(existing_mbid) = current
                .decision
                .as_ref()
                .and_then(|decision| decision.artist_mbid.as_deref())
                .filter(|existing| !existing.eq_ignore_ascii_case(&mbid))
                && !store
                    .release_decisions(&artist_key, existing_mbid)?
                    .is_empty()
            {
                return Err(
                    "Clear this artist's Aurora release decisions before changing its confirmed MBID."
                        .to_owned(),
                );
            }
            store.save_artist_decision(
                &artist_key,
                &display_artist,
                "confirmed",
                Some(&mbid),
                Some(&candidate.canonical_name),
            )?;
            load_artist_intelligence_with_store(display_artist, store)
        }
        ArtistDecisionRequest::Ignore { artist } => {
            let (display_artist, artist_key) = validate_artist(&artist)?;
            store.save_artist_decision(&artist_key, &display_artist, "ignored", None, None)?;
            load_artist_intelligence_with_store(display_artist, store)
        }
        ArtistDecisionRequest::Clear { artist } => {
            let (display_artist, artist_key) = validate_artist(&artist)?;
            let current = load_artist_intelligence_with_store(display_artist.clone(), store)?;
            if let Some(local_mbid) = current
                .decision
                .as_ref()
                .and_then(|decision| decision.artist_mbid.as_deref())
            {
                let has_external_authority = current.candidates.iter().any(|candidate| {
                    candidate.verified_source && candidate.mbid.eq_ignore_ascii_case(local_mbid)
                });
                if !has_external_authority
                    && !store.release_decisions(&artist_key, local_mbid)?.is_empty()
                {
                    return Err(
                        "Clear this artist's Aurora release decisions before removing its identity override."
                            .to_owned(),
                    );
                }
            }
            store.clear_artist_decision(&artist_key)?;
            load_artist_intelligence_with_store(display_artist, store)
        }
    }
}

pub(crate) fn update_release_decision(
    store: &StateStore,
    request: ReleaseDecisionRequest,
) -> Result<ArtistIntelligence, String> {
    let (artist, artist_mbid, release_mbid, action, local_album_id) = match request {
        ReleaseDecisionRequest::Link {
            artist,
            artist_mbid,
            release_mbid,
            local_album_id,
        } => (
            artist,
            artist_mbid,
            release_mbid,
            "linked",
            Some(local_album_id),
        ),
        ReleaseDecisionRequest::NotInScope {
            artist,
            artist_mbid,
            release_mbid,
        } => (artist, artist_mbid, release_mbid, "not-in-scope", None),
        ReleaseDecisionRequest::Ignore {
            artist,
            artist_mbid,
            release_mbid,
        } => (artist, artist_mbid, release_mbid, "ignored", None),
        ReleaseDecisionRequest::Clear {
            artist,
            artist_mbid,
            release_mbid,
        } => (artist, artist_mbid, release_mbid, "clear", None),
    };
    let (display_artist, artist_key) = validate_artist(&artist)?;
    let artist_mbid = artist_mbid.trim().to_lowercase();
    let release_mbid = release_mbid.trim().to_lowercase();
    if !valid_mbid(&artist_mbid) || !valid_mbid(&release_mbid) {
        return Err("The MusicBrainz artist or release identity is invalid.".to_owned());
    }
    let current = load_artist_intelligence_with_store(display_artist.clone(), store)?;
    let resolved = current
        .identity
        .as_ref()
        .filter(|identity| {
            identity.mbid.eq_ignore_ascii_case(&artist_mbid)
                && matches!(
                    identity.provenance,
                    "auroraState" | "curatedOverlay" | "catalogOverlay"
                )
        })
        .ok_or_else(|| {
            "Confirm the artist identity before curating its release groups.".to_owned()
        })?;
    if !current
        .releases
        .iter()
        .any(|release| release.mbid.eq_ignore_ascii_case(&release_mbid))
    {
        return Err(
            "That release group is no longer in the selected artist's local MusicBrainz snapshot."
                .to_owned(),
        );
    }

    if action == "clear" {
        store.clear_release_decision(&artist_key, &resolved.mbid, &release_mbid)?;
    } else {
        let local_album_id = local_album_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if action == "linked" {
            let album_id =
                local_album_id.ok_or_else(|| "Choose a local album to link.".to_owned())?;
            validate_local_album(&artist_key, album_id)?;
        }
        store.save_release_decision(
            &artist_key,
            &display_artist,
            &resolved.mbid,
            &release_mbid,
            action,
            local_album_id,
        )?;
    }
    load_artist_intelligence_with_store(display_artist, store)
}

pub(crate) fn undo_latest(store: &StateStore) -> Result<Option<ArtistIntelligence>, String> {
    let Some(result) = store.undo_latest_curation()? else {
        return Ok(None);
    };
    let intelligence = load_artist_intelligence_with_store(result.display_artist.clone(), store)?;
    Ok(Some(intelligence))
}

pub(crate) fn export_overlay_snapshot(
    app: &AppHandle,
    store: &StateStore,
) -> Result<CurationExportResult, String> {
    let export_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate Aurora's app-data directory: {error}"))?
        .join("exports");
    fs::create_dir_all(&export_directory)
        .map_err(|error| format!("Could not create Aurora's export directory: {error}"))?;
    let path = export_directory.join(format!("aurora-musicbrainz-overlay-{}.sqlite3", now_ms()));
    let (_, overlay_path) = default_source_paths()?;
    let source_rows_preserved = overlay_path.is_file();

    let create_result = if source_rows_preserved {
        create_consistent_copy(&overlay_path, &path)
    } else {
        Connection::open(&path)
            .map(|_| ())
            .map_err(|error| format!("Could not create Aurora's overlay export: {error}"))
    };
    if let Err(error) = create_result {
        let _ = fs::remove_file(&path);
        return Err(error);
    }

    let export_result = apply_export_decisions(&path, store);
    if let Err(error) = export_result {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    let (artist_decisions, release_decisions) = export_result?;
    Ok(CurationExportResult {
        path: path.to_string_lossy().into_owned(),
        artist_decisions,
        release_decisions,
        source_rows_preserved,
    })
}

fn validate_local_album(artist_key: &str, album_id: &str) -> Result<(), String> {
    if album_id.chars().count() > MAX_ALBUM_ID_CHARS {
        return Err("The local album identity is invalid.".to_owned());
    }
    let connection = open_catalog(&default_catalog_path()?)?;
    let album_artist = connection
        .query_row(
            "SELECT album_artist_display FROM albums WHERE id = ?1",
            [album_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|_| "That local album is no longer available in the catalog.".to_owned())?;
    let (_, album_artist_key) = validate_artist(&album_artist)?;
    if album_artist_key != artist_key {
        return Err("The selected local album belongs to another artist.".to_owned());
    }
    Ok(())
}

fn create_consistent_copy(source: &Path, destination: &Path) -> Result<(), String> {
    let connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Could not open the curated overlay for export: {error}"))?;
    connection
        .execute("VACUUM INTO ?1", [destination.to_string_lossy().as_ref()])
        .map_err(|error| format!("Could not create a consistent overlay snapshot: {error}"))?;
    Ok(())
}

fn apply_export_decisions(path: &Path, store: &StateStore) -> Result<(usize, usize), String> {
    let artists = store.all_artist_decisions()?;
    let releases = store.all_release_decisions()?;
    let mut connection = Connection::open(path)
        .map_err(|error| format!("Could not open Aurora's overlay export: {error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(15))
        .map_err(|error| format!("Could not configure Aurora's overlay export: {error}"))?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|error| format!("Could not enable overlay integrity checks: {error}"))?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|error| format!("Could not configure durable overlay export writes: {error}"))?;
    ensure_overlay_schema(&connection)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not begin the overlay export transaction: {error}"))?;

    for artist in &artists {
        let ignored = artist.decision == "ignored";
        transaction
            .execute(
                r#"
                INSERT INTO musicbrainz_artist_links(
                  local_artist_key, display_artist, mbid, canonical_name, match_method,
                  confidence, verification_state, ignored, created_at, updated_at
                ) VALUES (
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now')
                )
                ON CONFLICT(local_artist_key) DO UPDATE SET
                  display_artist = excluded.display_artist,
                  mbid = excluded.mbid,
                  canonical_name = excluded.canonical_name,
                  match_method = excluded.match_method,
                  confidence = excluded.confidence,
                  verification_state = excluded.verification_state,
                  ignored = excluded.ignored,
                  updated_at = excluded.updated_at
                "#,
                params![
                    artist.local_artist_key,
                    artist.display_artist,
                    artist.artist_mbid,
                    artist.canonical_name,
                    if ignored { "ignored" } else { "manual-mbid" },
                    if ignored { None } else { Some(1.0_f64) },
                    if ignored { "ignored" } else { "verified" },
                    i64::from(ignored),
                ],
            )
            .map_err(|error| format!("Could not export an artist decision: {error}"))?;
        transaction
            .execute(
                "DELETE FROM musicbrainz_artist_link_tombstones WHERE local_artist_key = ?1",
                [&artist.local_artist_key],
            )
            .map_err(|error| format!("Could not clear an obsolete artist tombstone: {error}"))?;
    }

    for release in &releases {
        transaction
            .execute(
                r#"
                INSERT OR IGNORE INTO musicbrainz_artist_links(
                  local_artist_key, display_artist, mbid, canonical_name, match_method,
                  confidence, verification_state, ignored, created_at, updated_at
                ) VALUES (
                  ?1, ?2, ?3, NULL, 'release-decision', NULL, 'unverified', 0,
                  datetime('now'), datetime('now')
                )
                "#,
                params![
                    release.local_artist_key,
                    release.display_artist,
                    release.artist_mbid,
                ],
            )
            .map_err(|error| format!("Could not export the release artist identity: {error}"))?;
        transaction
            .execute(
                r#"
                INSERT INTO musicbrainz_release_decisions(
                  local_artist_key, release_mbid, decision, local_album_id,
                  created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))
                ON CONFLICT(local_artist_key, release_mbid) DO UPDATE SET
                  decision = excluded.decision,
                  local_album_id = excluded.local_album_id,
                  updated_at = excluded.updated_at
                "#,
                params![
                    release.local_artist_key,
                    release.release_mbid,
                    if release.decision == "linked" {
                        "include"
                    } else {
                        &release.decision
                    },
                    release.local_album_id,
                ],
            )
            .map_err(|error| format!("Could not export a release decision: {error}"))?;
        transaction
            .execute(
                r#"
                DELETE FROM musicbrainz_release_decision_tombstones
                WHERE local_artist_key = ?1 AND release_mbid = ?2
                "#,
                params![release.local_artist_key, release.release_mbid],
            )
            .map_err(|error| format!("Could not clear an obsolete release tombstone: {error}"))?;
    }
    transaction
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS aurora_export_manifest(
              version TEXT NOT NULL,
              exported_at TEXT NOT NULL,
              artist_decisions INTEGER NOT NULL,
              release_decisions INTEGER NOT NULL
            );
            DELETE FROM aurora_export_manifest;
            "#,
        )
        .map_err(|error| format!("Could not prepare Aurora's export manifest: {error}"))?;
    transaction
        .execute(
            r#"
            INSERT INTO aurora_export_manifest(
              version, exported_at, artist_decisions, release_decisions
            ) VALUES ('0.6.0', datetime('now'), ?1, ?2)
            "#,
            params![artists.len() as i64, releases.len() as i64],
        )
        .map_err(|error| format!("Could not record Aurora's export manifest: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit Aurora's overlay export: {error}"))?;
    Ok((artists.len(), releases.len()))
}

fn ensure_overlay_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS musicbrainz_artist_links (
              local_artist_key TEXT PRIMARY KEY,
              display_artist TEXT NOT NULL,
              mbid TEXT,
              canonical_name TEXT,
              match_method TEXT NOT NULL DEFAULT 'unverified',
              confidence REAL,
              verification_state TEXT NOT NULL DEFAULT 'unverified',
              ignored INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_musicbrainz_artist_links_mbid
              ON musicbrainz_artist_links(mbid);
            CREATE TABLE IF NOT EXISTS musicbrainz_artist_link_tombstones (
              local_artist_key TEXT PRIMARY KEY,
              display_artist TEXT,
              updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS musicbrainz_release_decisions (
              local_artist_key TEXT NOT NULL,
              release_mbid TEXT NOT NULL,
              decision TEXT NOT NULL,
              local_album_id TEXT,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (local_artist_key, release_mbid),
              FOREIGN KEY(local_artist_key) REFERENCES musicbrainz_artist_links(local_artist_key)
                ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_musicbrainz_release_decisions_decision
              ON musicbrainz_release_decisions(decision);
            CREATE TABLE IF NOT EXISTS musicbrainz_release_decision_tombstones (
              local_artist_key TEXT NOT NULL,
              release_mbid TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (local_artist_key, release_mbid)
            );
            CREATE TABLE IF NOT EXISTS musicbrainz_artist_release_groups (
              artist_mbid TEXT NOT NULL,
              release_mbid TEXT NOT NULL,
              title TEXT NOT NULL,
              year INTEGER,
              type TEXT,
              secondary_types TEXT NOT NULL DEFAULT '',
              track_count INTEGER,
              status TEXT NOT NULL DEFAULT 'Official',
              source TEXT NOT NULL DEFAULT 'musicbrainz-live',
              fetched_at TEXT NOT NULL,
              PRIMARY KEY (artist_mbid, release_mbid)
            );
            CREATE INDEX IF NOT EXISTS idx_musicbrainz_artist_release_groups_artist
              ON musicbrainz_artist_release_groups(artist_mbid);
            CREATE TABLE IF NOT EXISTS musicbrainz_release_status_cache (
              artist_mbid TEXT NOT NULL,
              release_mbid TEXT NOT NULL,
              has_official_release INTEGER NOT NULL,
              checked_at TEXT NOT NULL,
              PRIMARY KEY (artist_mbid, release_mbid)
            );
            "#,
        )
        .map_err(|error| {
            format!("Could not ensure the overlay-compatible export schema: {error}")
        })?;
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temporary_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aurora-{label}-{}-{}.sqlite3",
            std::process::id(),
            now_ms()
        ))
    }

    #[test]
    fn overlay_export_uses_music_library_contract_without_touching_source_data() {
        let state_path = temporary_path("curation-state");
        let export_path = temporary_path("curation-export");
        let store = StateStore::new(state_path.clone()).expect("state store");
        store
            .save_artist_decision(
                "m83",
                "M83",
                "confirmed",
                Some("11111111-1111-1111-1111-111111111111"),
                Some("M83"),
            )
            .expect("artist decision");
        store
            .save_release_decision(
                "m83",
                "M83",
                "11111111-1111-1111-1111-111111111111",
                "22222222-2222-2222-2222-222222222222",
                "linked",
                Some("mb:33333333-3333-3333-3333-333333333333"),
            )
            .expect("release decision");

        let connection = Connection::open(&export_path).expect("export database");
        ensure_overlay_schema(&connection).expect("overlay schema");
        connection
            .execute(
                "INSERT INTO musicbrainz_artist_links(
                   local_artist_key, display_artist, match_method, verification_state,
                   ignored, created_at, updated_at
                 ) VALUES ('preserved', 'Preserved', 'manual-mbid', 'verified', 0,
                           datetime('now'), datetime('now'))",
                [],
            )
            .expect("source-compatible row");
        drop(connection);

        assert_eq!(
            apply_export_decisions(&export_path, &store).expect("apply decisions"),
            (1, 1)
        );

        let exported = Connection::open(&export_path).expect("reopen export");
        assert_eq!(
            exported
                .query_row(
                    "SELECT COUNT(*) FROM musicbrainz_artist_links WHERE local_artist_key = 'preserved'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("preserved count"),
            1
        );
        assert_eq!(
            exported
                .query_row(
                    "SELECT decision FROM musicbrainz_release_decisions
                     WHERE local_artist_key = 'm83'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("exported release decision"),
            "include"
        );
        assert_eq!(
            exported
                .query_row("SELECT version FROM aurora_export_manifest", [], |row| {
                    row.get::<_, String>(0)
                },)
                .expect("manifest version"),
            "0.6.0"
        );

        drop(exported);
        drop(store);
        let _ = fs::remove_file(export_path);
        let _ = fs::remove_file(state_path);
    }
}
