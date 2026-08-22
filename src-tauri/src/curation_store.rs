use crate::state_store::StateStore;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredArtistDecision {
    pub(crate) local_artist_key: String,
    pub(crate) display_artist: String,
    pub(crate) decision: String,
    pub(crate) artist_mbid: Option<String>,
    pub(crate) canonical_name: Option<String>,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredReleaseDecision {
    pub(crate) local_artist_key: String,
    pub(crate) display_artist: String,
    pub(crate) artist_mbid: String,
    pub(crate) release_mbid: String,
    pub(crate) decision: String,
    pub(crate) local_album_id: Option<String>,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurationUndoResult {
    pub(crate) entity_kind: String,
    pub(crate) local_artist_key: String,
    pub(crate) display_artist: String,
}

impl StateStore {
    pub(crate) fn artist_decision(
        &self,
        local_artist_key: &str,
    ) -> Result<Option<StoredArtistDecision>, String> {
        let connection = self.open()?;
        connection
            .query_row(
                r#"
                SELECT local_artist_key, display_artist, decision, artist_mbid,
                       canonical_name, created_at_ms, updated_at_ms
                FROM musicbrainz_artist_decisions WHERE local_artist_key = ?1
                "#,
                [local_artist_key],
                artist_decision_from_row,
            )
            .optional()
            .map_err(|error| format!("Could not read Aurora's artist decision: {error}"))
    }

    pub(crate) fn all_artist_decisions(&self) -> Result<Vec<StoredArtistDecision>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT local_artist_key, display_artist, decision, artist_mbid,
                       canonical_name, created_at_ms, updated_at_ms
                FROM musicbrainz_artist_decisions ORDER BY local_artist_key
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's artist decisions: {error}"))?;
        statement
            .query_map([], artist_decision_from_row)
            .map_err(|error| format!("Could not read Aurora's artist decisions: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's artist decisions: {error}"))
    }

    pub(crate) fn save_artist_decision(
        &self,
        local_artist_key: &str,
        display_artist: &str,
        decision: &str,
        artist_mbid: Option<&str>,
        canonical_name: Option<&str>,
    ) -> Result<StoredArtistDecision, String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin saving the artist decision: {error}"))?;
        let before = artist_decision_in_transaction(&transaction, local_artist_key)?;
        let timestamp = now_ms();
        transaction
            .execute(
                r#"
                INSERT INTO musicbrainz_artist_decisions(
                  local_artist_key, display_artist, decision, artist_mbid,
                  canonical_name, created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                ON CONFLICT(local_artist_key) DO UPDATE SET
                  display_artist = excluded.display_artist,
                  decision = excluded.decision,
                  artist_mbid = excluded.artist_mbid,
                  canonical_name = excluded.canonical_name,
                  updated_at_ms = excluded.updated_at_ms
                "#,
                params![
                    local_artist_key,
                    display_artist,
                    decision,
                    artist_mbid,
                    canonical_name,
                    timestamp
                ],
            )
            .map_err(|error| format!("Could not save Aurora's artist decision: {error}"))?;
        let after = artist_decision_in_transaction(&transaction, local_artist_key)?
            .ok_or_else(|| "Aurora could not verify the saved artist decision.".to_owned())?;
        record_event(
            &transaction,
            "artist",
            local_artist_key,
            artist_mbid,
            None,
            before.as_ref(),
            Some(&after),
        )?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's artist decision: {error}"))?;
        Ok(after)
    }

    pub(crate) fn clear_artist_decision(&self, local_artist_key: &str) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin clearing the artist decision: {error}"))?;
        let before = artist_decision_in_transaction(&transaction, local_artist_key)?;
        if let Some(before) = before {
            transaction
                .execute(
                    "DELETE FROM musicbrainz_artist_decisions WHERE local_artist_key = ?1",
                    [local_artist_key],
                )
                .map_err(|error| format!("Could not clear Aurora's artist decision: {error}"))?;
            record_event(
                &transaction,
                "artist",
                local_artist_key,
                before.artist_mbid.as_deref(),
                None,
                Some(&before),
                Option::<&StoredArtistDecision>::None,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit the cleared artist decision: {error}"))
    }

    pub(crate) fn release_decisions(
        &self,
        local_artist_key: &str,
        artist_mbid: &str,
    ) -> Result<Vec<StoredReleaseDecision>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT local_artist_key, display_artist, artist_mbid, release_mbid,
                       decision, local_album_id, created_at_ms, updated_at_ms
                FROM musicbrainz_release_decisions
                WHERE local_artist_key = ?1 AND artist_mbid = ?2
                ORDER BY release_mbid
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's release decisions: {error}"))?;
        statement
            .query_map(
                params![local_artist_key, artist_mbid],
                release_decision_from_row,
            )
            .map_err(|error| format!("Could not read Aurora's release decisions: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's release decisions: {error}"))
    }

    pub(crate) fn all_release_decisions(&self) -> Result<Vec<StoredReleaseDecision>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT local_artist_key, display_artist, artist_mbid, release_mbid,
                       decision, local_album_id, created_at_ms, updated_at_ms
                FROM musicbrainz_release_decisions
                ORDER BY local_artist_key, artist_mbid, release_mbid
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's release decisions: {error}"))?;
        statement
            .query_map([], release_decision_from_row)
            .map_err(|error| format!("Could not read Aurora's release decisions: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's release decisions: {error}"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn save_release_decision(
        &self,
        local_artist_key: &str,
        display_artist: &str,
        artist_mbid: &str,
        release_mbid: &str,
        decision: &str,
        local_album_id: Option<&str>,
    ) -> Result<StoredReleaseDecision, String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin saving the release decision: {error}"))?;
        let before = release_decision_in_transaction(
            &transaction,
            local_artist_key,
            artist_mbid,
            release_mbid,
        )?;
        let timestamp = now_ms();
        transaction
            .execute(
                r#"
                INSERT INTO musicbrainz_release_decisions(
                  local_artist_key, display_artist, artist_mbid, release_mbid,
                  decision, local_album_id, created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(local_artist_key, artist_mbid, release_mbid) DO UPDATE SET
                  display_artist = excluded.display_artist,
                  decision = excluded.decision,
                  local_album_id = excluded.local_album_id,
                  updated_at_ms = excluded.updated_at_ms
                "#,
                params![
                    local_artist_key,
                    display_artist,
                    artist_mbid,
                    release_mbid,
                    decision,
                    local_album_id,
                    timestamp
                ],
            )
            .map_err(|error| format!("Could not save Aurora's release decision: {error}"))?;
        let after = release_decision_in_transaction(
            &transaction,
            local_artist_key,
            artist_mbid,
            release_mbid,
        )?
        .ok_or_else(|| "Aurora could not verify the saved release decision.".to_owned())?;
        record_event(
            &transaction,
            "release",
            local_artist_key,
            Some(artist_mbid),
            Some(release_mbid),
            before.as_ref(),
            Some(&after),
        )?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's release decision: {error}"))?;
        Ok(after)
    }

    pub(crate) fn clear_release_decision(
        &self,
        local_artist_key: &str,
        artist_mbid: &str,
        release_mbid: &str,
    ) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin clearing the release decision: {error}"))?;
        let before = release_decision_in_transaction(
            &transaction,
            local_artist_key,
            artist_mbid,
            release_mbid,
        )?;
        if let Some(before) = before {
            transaction
                .execute(
                    r#"
                    DELETE FROM musicbrainz_release_decisions
                    WHERE local_artist_key = ?1 AND artist_mbid = ?2 AND release_mbid = ?3
                    "#,
                    params![local_artist_key, artist_mbid, release_mbid],
                )
                .map_err(|error| format!("Could not clear Aurora's release decision: {error}"))?;
            record_event(
                &transaction,
                "release",
                local_artist_key,
                Some(artist_mbid),
                Some(release_mbid),
                Some(&before),
                Option::<&StoredReleaseDecision>::None,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit the cleared release decision: {error}"))
    }

    pub(crate) fn undo_latest_curation(&self) -> Result<Option<CurationUndoResult>, String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin undoing the curation decision: {error}"))?;
        let event = transaction
            .query_row(
                r#"
                SELECT id, entity_kind, local_artist_key, artist_mbid, release_mbid, before_json
                FROM musicbrainz_curation_events ORDER BY id DESC LIMIT 1
                "#,
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("Could not read Aurora's curation history: {error}"))?;
        let Some((id, entity_kind, local_artist_key, artist_mbid, release_mbid, before_json)) =
            event
        else {
            return Ok(None);
        };

        let display_artist = if entity_kind == "artist" {
            transaction
                .execute(
                    "DELETE FROM musicbrainz_artist_decisions WHERE local_artist_key = ?1",
                    [&local_artist_key],
                )
                .map_err(|error| {
                    format!("Could not reset the artist decision during undo: {error}")
                })?;
            if let Some(json) = before_json {
                let before: StoredArtistDecision =
                    serde_json::from_str(&json).map_err(|error| {
                        format!("Could not decode the prior artist decision: {error}")
                    })?;
                insert_artist_decision(&transaction, &before)?;
                before.display_artist
            } else {
                local_artist_key.clone()
            }
        } else {
            let artist_mbid = artist_mbid
                .as_deref()
                .ok_or_else(|| "Aurora's release history is missing an artist MBID.".to_owned())?;
            let release_mbid = release_mbid
                .as_deref()
                .ok_or_else(|| "Aurora's release history is missing a release MBID.".to_owned())?;
            transaction
                .execute(
                    r#"
                    DELETE FROM musicbrainz_release_decisions
                    WHERE local_artist_key = ?1 AND artist_mbid = ?2 AND release_mbid = ?3
                    "#,
                    params![local_artist_key, artist_mbid, release_mbid],
                )
                .map_err(|error| {
                    format!("Could not reset the release decision during undo: {error}")
                })?;
            if let Some(json) = before_json {
                let before: StoredReleaseDecision =
                    serde_json::from_str(&json).map_err(|error| {
                        format!("Could not decode the prior release decision: {error}")
                    })?;
                insert_release_decision(&transaction, &before)?;
                before.display_artist
            } else {
                local_artist_key.clone()
            }
        };
        transaction
            .execute(
                "DELETE FROM musicbrainz_curation_events WHERE id = ?1",
                [id],
            )
            .map_err(|error| format!("Could not complete Aurora's curation undo: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's curation undo: {error}"))?;
        Ok(Some(CurationUndoResult {
            entity_kind,
            local_artist_key,
            display_artist,
        }))
    }
}

fn artist_decision_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredArtistDecision> {
    Ok(StoredArtistDecision {
        local_artist_key: row.get(0)?,
        display_artist: row.get(1)?,
        decision: row.get(2)?,
        artist_mbid: row.get(3)?,
        canonical_name: row.get(4)?,
        created_at_ms: row.get(5)?,
        updated_at_ms: row.get(6)?,
    })
}

fn release_decision_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredReleaseDecision> {
    Ok(StoredReleaseDecision {
        local_artist_key: row.get(0)?,
        display_artist: row.get(1)?,
        artist_mbid: row.get(2)?,
        release_mbid: row.get(3)?,
        decision: row.get(4)?,
        local_album_id: row.get(5)?,
        created_at_ms: row.get(6)?,
        updated_at_ms: row.get(7)?,
    })
}

fn artist_decision_in_transaction(
    transaction: &Transaction<'_>,
    local_artist_key: &str,
) -> Result<Option<StoredArtistDecision>, String> {
    transaction
        .query_row(
            r#"
            SELECT local_artist_key, display_artist, decision, artist_mbid,
                   canonical_name, created_at_ms, updated_at_ms
            FROM musicbrainz_artist_decisions WHERE local_artist_key = ?1
            "#,
            [local_artist_key],
            artist_decision_from_row,
        )
        .optional()
        .map_err(|error| format!("Could not inspect the prior artist decision: {error}"))
}

fn release_decision_in_transaction(
    transaction: &Transaction<'_>,
    local_artist_key: &str,
    artist_mbid: &str,
    release_mbid: &str,
) -> Result<Option<StoredReleaseDecision>, String> {
    transaction
        .query_row(
            r#"
            SELECT local_artist_key, display_artist, artist_mbid, release_mbid,
                   decision, local_album_id, created_at_ms, updated_at_ms
            FROM musicbrainz_release_decisions
            WHERE local_artist_key = ?1 AND artist_mbid = ?2 AND release_mbid = ?3
            "#,
            params![local_artist_key, artist_mbid, release_mbid],
            release_decision_from_row,
        )
        .optional()
        .map_err(|error| format!("Could not inspect the prior release decision: {error}"))
}

fn record_event<T: Serialize>(
    transaction: &Transaction<'_>,
    entity_kind: &str,
    local_artist_key: &str,
    artist_mbid: Option<&str>,
    release_mbid: Option<&str>,
    before: Option<&T>,
    after: Option<&T>,
) -> Result<(), String> {
    let before_json = before
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| format!("Could not serialize the prior curation decision: {error}"))?;
    let after_json = after
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| format!("Could not serialize the saved curation decision: {error}"))?;
    transaction
        .execute(
            r#"
            INSERT INTO musicbrainz_curation_events(
              entity_kind, local_artist_key, artist_mbid, release_mbid,
              before_json, after_json, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                entity_kind,
                local_artist_key,
                artist_mbid,
                release_mbid,
                before_json,
                after_json,
                now_ms()
            ],
        )
        .map_err(|error| format!("Could not record Aurora's curation history: {error}"))?;
    Ok(())
}

fn insert_artist_decision(
    transaction: &Transaction<'_>,
    decision: &StoredArtistDecision,
) -> Result<(), String> {
    transaction
        .execute(
            r#"
            INSERT INTO musicbrainz_artist_decisions(
              local_artist_key, display_artist, decision, artist_mbid,
              canonical_name, created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                decision.local_artist_key,
                decision.display_artist,
                decision.decision,
                decision.artist_mbid,
                decision.canonical_name,
                decision.created_at_ms,
                decision.updated_at_ms
            ],
        )
        .map_err(|error| format!("Could not restore the artist decision: {error}"))?;
    Ok(())
}

fn insert_release_decision(
    transaction: &Transaction<'_>,
    decision: &StoredReleaseDecision,
) -> Result<(), String> {
    transaction
        .execute(
            r#"
            INSERT INTO musicbrainz_release_decisions(
              local_artist_key, display_artist, artist_mbid, release_mbid,
              decision, local_album_id, created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                decision.local_artist_key,
                decision.display_artist,
                decision.artist_mbid,
                decision.release_mbid,
                decision.decision,
                decision.local_album_id,
                decision.created_at_ms,
                decision.updated_at_ms
            ],
        )
        .map_err(|error| format!("Could not restore the release decision: {error}"))?;
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    fn temporary_state_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aurora-curation-state-{}-{unique}.sqlite3",
            std::process::id()
        ))
    }

    #[test]
    fn decisions_survive_restart_clear_and_undo() {
        let path = temporary_state_path();
        let store = StateStore::new(path.clone()).expect("create state store");
        store
            .save_artist_decision(
                "m83",
                "M83",
                "confirmed",
                Some("6d7b7cd4-254b-4c25-83f6-dd20f98ceacd"),
                Some("M83"),
            )
            .expect("save artist decision");
        drop(store);

        let reopened = StateStore::new(path.clone()).expect("reopen state store");
        assert_eq!(
            reopened
                .artist_decision("m83")
                .expect("read decision")
                .unwrap()
                .decision,
            "confirmed"
        );
        reopened
            .clear_artist_decision("m83")
            .expect("clear decision");
        assert!(
            reopened
                .artist_decision("m83")
                .expect("read cleared")
                .is_none()
        );
        reopened
            .undo_latest_curation()
            .expect("undo clear")
            .expect("undo result");
        assert_eq!(
            reopened
                .artist_decision("m83")
                .expect("read restored")
                .unwrap()
                .decision,
            "confirmed"
        );

        drop(reopened);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn linked_release_is_unique_per_local_album() {
        let path = temporary_state_path();
        let store = StateStore::new(path.clone()).expect("create state store");
        store
            .save_release_decision(
                "m83",
                "M83",
                "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
                "11111111-1111-1111-1111-111111111111",
                "linked",
                Some("album-1"),
            )
            .expect("link release");
        let duplicate = store.save_release_decision(
            "m83",
            "M83",
            "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
            "22222222-2222-2222-2222-222222222222",
            "linked",
            Some("album-1"),
        );
        assert!(duplicate.is_err());
        drop(store);
        let _ = fs::remove_file(path);
    }
}
