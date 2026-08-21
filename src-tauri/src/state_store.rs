use rusqlite::{Connection, TransactionBehavior, params};
use std::{fs, path::PathBuf, time::Duration};

const SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StoredPlaybackState {
    pub(crate) track_ids: Vec<String>,
    pub(crate) current_index: Option<usize>,
    pub(crate) position_seconds: f64,
    pub(crate) volume: f32,
    pub(crate) shuffle: bool,
    pub(crate) repeat_mode: String,
}

impl Default for StoredPlaybackState {
    fn default() -> Self {
        Self {
            track_ids: Vec::new(),
            current_index: None,
            position_seconds: 0.0,
            volume: 0.7,
            shuffle: false,
            repeat_mode: "off".to_owned(),
        }
    }
}

pub(crate) struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub(crate) fn new(path: PathBuf) -> Result<Self, String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Aurora's state path has no parent directory.".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create Aurora's state directory: {error}"))?;
        let store = Self { path };
        store.migrate()?;
        Ok(store)
    }

    fn open(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path)
            .map_err(|error| format!("Could not open Aurora's state database: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(2))
            .map_err(|error| format!("Could not configure Aurora's state database: {error}"))?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| format!("Could not enable state integrity checks: {error}"))?;
        Ok(connection)
    }

    fn migrate(&self) -> Result<(), String> {
        let mut connection = self.open()?;
        let current: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("Could not read Aurora's state schema: {error}"))?;
        if current > SCHEMA_VERSION {
            return Err(format!(
                "Aurora's state database uses unsupported schema version {current}."
            ));
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start Aurora's state migration: {error}"))?;
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS playback_queue (
                  position INTEGER PRIMARY KEY,
                  track_id TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS playback_state (
                  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                  current_index INTEGER,
                  position_seconds REAL NOT NULL DEFAULT 0,
                  volume REAL NOT NULL DEFAULT 0.7 CHECK (volume >= 0 AND volume <= 1),
                  shuffle INTEGER NOT NULL DEFAULT 0 CHECK (shuffle IN (0, 1)),
                  repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK (repeat_mode IN ('off', 'all', 'one'))
                );
                INSERT OR IGNORE INTO playback_state(singleton) VALUES (1);
                "#,
            )
            .map_err(|error| format!("Could not migrate Aurora's state database: {error}"))?;
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|error| {
                format!("Could not mark Aurora's state migration complete: {error}")
            })?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's state migration: {error}"))
    }

    pub(crate) fn load(&self) -> Result<StoredPlaybackState, String> {
        let connection = self.open()?;
        let mut queue_statement = connection
            .prepare("SELECT track_id FROM playback_queue ORDER BY position")
            .map_err(|error| format!("Could not prepare Aurora's queue restore: {error}"))?;
        let track_ids = queue_statement
            .query_map([], |row| row.get(0))
            .map_err(|error| format!("Could not restore Aurora's queue: {error}"))?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|error| format!("Could not decode Aurora's queue: {error}"))?;
        connection
            .query_row(
                r#"
                SELECT current_index, position_seconds, volume, shuffle, repeat_mode
                FROM playback_state WHERE singleton = 1
                "#,
                [],
                |row| {
                    let current_index: Option<i64> = row.get(0)?;
                    Ok(StoredPlaybackState {
                        track_ids,
                        current_index: current_index.and_then(|value| usize::try_from(value).ok()),
                        position_seconds: row.get::<_, f64>(1)?.max(0.0),
                        volume: row.get::<_, f32>(2)?.clamp(0.0, 1.0),
                        shuffle: row.get::<_, i64>(3)? == 1,
                        repeat_mode: row.get(4)?,
                    })
                },
            )
            .map_err(|error| format!("Could not restore Aurora's playback state: {error}"))
    }

    pub(crate) fn save(&self, state: &StoredPlaybackState) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin saving Aurora's queue: {error}"))?;
        transaction
            .execute("DELETE FROM playback_queue", [])
            .map_err(|error| format!("Could not replace Aurora's queue: {error}"))?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO playback_queue(position, track_id) VALUES (?1, ?2)")
                .map_err(|error| format!("Could not prepare Aurora's queue save: {error}"))?;
            for (position, track_id) in state.track_ids.iter().enumerate() {
                insert
                    .execute(params![position as i64, track_id])
                    .map_err(|error| format!("Could not save Aurora's queue: {error}"))?;
            }
        }
        transaction
            .execute(
                r#"
                UPDATE playback_state
                SET current_index = ?1, position_seconds = ?2, volume = ?3,
                    shuffle = ?4, repeat_mode = ?5
                WHERE singleton = 1
                "#,
                params![
                    state.current_index.map(|value| value as i64),
                    state.position_seconds.max(0.0),
                    state.volume.clamp(0.0, 1.0),
                    i64::from(state.shuffle),
                    state.repeat_mode,
                ],
            )
            .map_err(|error| format!("Could not save Aurora's playback settings: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's playback state: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_state_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aurora-state-{}-{unique}.sqlite3",
            std::process::id()
        ))
    }

    #[test]
    fn queue_and_controls_survive_reopen() {
        let path = temporary_state_path();
        let store = StateStore::new(path.clone()).expect("state store");
        let expected = StoredPlaybackState {
            track_ids: vec!["7".to_owned(), "9".to_owned()],
            current_index: Some(1),
            position_seconds: 31.5,
            volume: 0.42,
            shuffle: true,
            repeat_mode: "all".to_owned(),
        };
        store.save(&expected).expect("save state");
        drop(store);

        let reopened = StateStore::new(path.clone()).expect("reopen state");
        assert_eq!(reopened.load().expect("load state"), expected);

        std::fs::remove_file(path).expect("remove test state");
    }
}
