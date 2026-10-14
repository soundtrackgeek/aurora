//! Project saved plays through Music Library's reviewed artist assignments.
//! Raw device snapshots remain immutable; ambiguous name matches stay unresolved.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct ArtistProfile {
    pub id: Option<String>,
    pub name: String,
    pub disambiguation: Option<String>,
}

impl ArtistProfile {
    pub fn key(&self) -> String {
        self.id.clone().unwrap_or_else(|| {
            let name = artist_identity(&self.name);
            if self.disambiguation.as_deref() == Some("Unresolved artist") {
                format!("history-unresolved:{name}")
            } else {
                name
            }
        })
    }
}

pub(super) struct ArtistProfiles {
    catalog: Option<Connection>,
    by_id: HashMap<String, ArtistProfile>,
    by_name: HashMap<String, Vec<String>>,
    tracks: HashMap<String, Option<(ArtistProfile, ArtistProfile, String)>>,
}

impl ArtistProfiles {
    pub fn open() -> Result<Self, String> {
        let connection = catalog::default_catalog_path()
            .and_then(|path| catalog::open_catalog(&path))
            .ok();
        Self::new(connection)
    }

    pub fn new(connection: Option<Connection>) -> Result<Self, String> {
        if let Some(connection) = connection.as_ref() {
            connection
                .execute_batch("BEGIN DEFERRED TRANSACTION")
                .map_err(|e| e.to_string())?;
        }
        let mut index = Self {
            catalog: connection,
            by_id: HashMap::new(),
            by_name: HashMap::new(),
            tracks: HashMap::new(),
        };
        if let Some(connection) = index.catalog.as_ref() {
            let published = crate::artist_identity::published(connection);
            let sql = if published {
                "SELECT id,display_artist,disambiguation FROM artist_identities".to_owned()
            } else {
                format!(
                    "SELECT {},MIN(album_artist_display),NULL FROM albums GROUP BY {}",
                    catalog::artist_key_sql("album_artist_display"),
                    catalog::artist_key_sql("album_artist_display")
                )
            };
            let mut statement = connection.prepare(&sql).map_err(|e| e.to_string())?;
            for row in statement
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                })
                .map_err(|e| e.to_string())?
            {
                let (id, name, disambiguation) = row.map_err(|e| e.to_string())?;
                index
                    .by_name
                    .entry(artist_identity(&name))
                    .or_default()
                    .push(id.clone());
                index.by_id.insert(
                    id.clone(),
                    ArtistProfile {
                        id: Some(id),
                        name,
                        disambiguation,
                    },
                );
            }
        }
        Ok(index)
    }

    pub fn by_name(&self, name: &str) -> ArtistProfile {
        let ids = self.by_name.get(&artist_identity(name));
        if let Some(ids) = ids
            && ids.len() == 1
        {
            return self.by_id[&ids[0]].clone();
        }
        ArtistProfile {
            id: None,
            name: name.trim().into(),
            disambiguation: ids
                .filter(|ids| ids.len() > 1)
                .map(|_| "Unresolved artist".into()),
        }
    }

    pub fn reference(&self, reference: &str) -> Result<ArtistProfile, String> {
        if let Some(profile) = self.by_id.get(reference) {
            return Ok(profile.clone());
        }
        let profile = self.by_name(reference);
        if profile.disambiguation.as_deref() == Some("Unresolved artist") {
            return Err(
                "Several artists share this name. Choose the separate profile in Artists.".into(),
            );
        }
        Ok(profile)
    }

    fn track_profiles(
        &self,
        owner: &str,
        performer: &str,
        id: &str,
        album_id: String,
    ) -> (ArtistProfile, ArtistProfile, String) {
        let album = self
            .by_id
            .get(id)
            .cloned()
            .unwrap_or_else(|| self.by_name(owner));
        let performer = if performer.trim().is_empty() {
            owner
        } else {
            performer
        };
        let artist = if artist_identity(performer) == artist_identity(owner) {
            album.clone()
        } else {
            self.by_name(performer)
        };
        (artist, album, album_id)
    }

    pub fn project(&mut self, row: &mut HistoryRow) -> Result<(), String> {
        let cache_key = format!(
            "{}\0{}\0{}\0{}",
            row.track_key, row.artist, row.album, row.title
        );
        if !self.tracks.contains_key(&cache_key) {
            let binding = if let Some(connection) = self.catalog.as_ref() {
                let key = crate::artist_identity::key_sql(connection, "t.album_artist_display");
                let columns = format!(
                    "SELECT COALESCE(t.album_artist_display,''),COALESCE(t.display_artist,''),{key},COALESCE(t.album_id,'') FROM tracks t"
                );
                let mut statement = connection
                    .prepare_cached(&format!("{columns} WHERE t.file_path=?1 AND t.filename=?2"))
                    .map_err(|e| e.to_string())?;
                let mut candidates = statement
                    .query_map(params![row.directory, row.filename], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    })
                    .map_err(|e| e.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                if candidates.is_empty() && !row.album.trim().is_empty() {
                    // Exact album/title evidence can rebind a moved file. A name
                    // alone never chooses between separate profiles.
                    let artist_key = catalog::artist_key_sql("a.album_artist_display");
                    let name = catalog::artist_key_sql("?1");
                    let mut by_album=connection.prepare_cached(&format!("{columns} WHERE t.album_id IN (SELECT a.id FROM albums a WHERE {artist_key}={name} AND a.album=?2 COLLATE NOCASE) AND t.title=?3 COLLATE NOCASE")).map_err(|e|e.to_string())?;
                    candidates = by_album
                        .query_map(params![row.artist, row.album, row.title], |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                                r.get::<_, String>(3)?,
                            ))
                        })
                        .map_err(|e| e.to_string())?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| e.to_string())?;
                }
                let mut values = candidates.into_iter().map(|(owner, performer, id, album)| {
                    self.track_profiles(&owner, &performer, &id, album)
                });
                let first = values.next();
                if values.any(|value| {
                    first.as_ref().is_some_and(|first| {
                        first.0.key() != value.0.key() || first.1.key() != value.1.key()
                    })
                }) {
                    None
                } else {
                    first
                }
            } else {
                None
            };
            self.tracks.insert(cache_key.clone(), binding);
        }
        if let Some((artist, album, album_id)) =
            self.tracks.get(&cache_key).and_then(Option::as_ref)
        {
            row.artist_profile = Some(artist.clone());
            row.album_profile = Some(album.clone());
            row.album_identity = (!album_id.is_empty()).then(|| album_id.clone());
        } else {
            row.artist_profile = Some(self.by_name(&row.artist));
            row.album_profile = row.artist_profile.clone();
        }
        Ok(())
    }

    pub fn project_rows(&mut self, rows: &mut [HistoryRow]) -> Result<(), String> {
        for row in rows {
            self.project(row)?;
        }
        Ok(())
    }

    pub fn for_top(&mut self, track: &HistoryTopRow) -> Result<ArtistProfile, String> {
        let mut row = HistoryRow {
            track_key: track.track_key.clone(),
            artist: track.artist.clone(),
            title: track.title.clone(),
            album: track.album.clone(),
            directory: track.directory.clone(),
            filename: track.filename.clone(),
            ..Default::default()
        };
        self.project(&mut row)?;
        Ok(row.artist_profile())
    }

    pub fn top_track(
        &mut self,
        row: HistoryTopRow,
        track: Option<TrackSummary>,
    ) -> Result<HistoryTopTrack, String> {
        let profile = self.for_top(&row)?;
        Ok(HistoryTopTrack {
            artist_id: profile.id,
            artist_disambiguation: profile.disambiguation,
            artist: profile.name,
            track,
            title: row.title,
            track_key: row.track_key,
            album: row.album,
            plays: row.plays,
            listened_seconds: row.listened_seconds,
            last_played_at_ms: row.last_played_at_ms,
        })
    }
}

impl HistoryRow {
    pub(super) fn artist_profile(&self) -> ArtistProfile {
        self.artist_profile
            .clone()
            .unwrap_or_else(|| ArtistProfile {
                id: None,
                name: self.artist.clone(),
                disambiguation: None,
            })
    }
    pub(super) fn artist_key(&self) -> String {
        self.artist_profile().key()
    }
    pub(super) fn album_key(&self) -> String {
        self.album_identity.clone().unwrap_or_else(|| {
            format!(
                "{}\0{}",
                self.album_profile
                    .as_ref()
                    .map_or_else(|| artist_identity(&self.artist), ArtistProfile::key),
                self.album.trim().to_lowercase()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        catalog::register_catalog_functions(&conn).unwrap();
        conn.execute_batch("CREATE TABLE artist_identities(id TEXT PRIMARY KEY,display_artist TEXT,disambiguation TEXT); CREATE TABLE albums(id TEXT,album_artist_display TEXT,album TEXT,artist_identity_key TEXT); CREATE TABLE tracks(album_id TEXT,album_artist_display TEXT,display_artist TEXT,title TEXT,file_path TEXT,filename TEXT,artist_identity_key TEXT);").unwrap();
        for (index, name, description) in [
            (1, "Creed", "First group"),
            (2, "Creed", "Second group"),
            (3, "The Kids", "First group"),
            (4, "The Kids", "Second group"),
            (5, "The Kids", "Third group"),
        ] {
            let id = format!("artist:{index}");
            conn.execute(
                "INSERT INTO artist_identities VALUES(?1,?2,?3)",
                params![id, name, description],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO albums VALUES(?1,?2,?3,?1)",
                params![id, name, format!("Album {index}")],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks VALUES(?1,?2,?2,'Same song',?3,'01.mp3',?1)",
                params![id, name, format!("D:/Music/{index}")],
            )
            .unwrap();
        }
        conn
    }

    fn row(index: i64, name: &str) -> HistoryRow {
        HistoryRow {
            session_id: format!("session:{index}"),
            track_key: format!("track:{index}"),
            artist: name.into(),
            title: "Same song".into(),
            album: format!("Album {index}"),
            directory: format!("D:/Music/{index}"),
            filename: "01.mp3".into(),
            registered_play: true,
            started_at_ms: index * 100_000,
            listened_seconds: 30.0,
            ..Default::default()
        }
    }

    #[test]
    fn two_and_three_same_name_profiles_keep_report_counts_and_discovery_separate() {
        let mut profiles = ArtistProfiles::new(Some(catalog_fixture())).unwrap();
        let mut rows = vec![
            row(1, "Creed"),
            row(2, "Creed"),
            row(3, "The Kids"),
            row(4, "The Kids"),
            row(5, "The Kids"),
        ];
        profiles.project_rows(&mut rows).unwrap();
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row.artist_profile().id,
                Some(format!("artist:{}", index + 1))
            );
            assert!(row.artist_profile().disambiguation.is_some());
        }
        let refs = rows.iter().collect::<Vec<_>>();
        let summary = report_summary(&refs, 0);
        assert_eq!(summary.unique_artists, 5);
        assert_eq!(summary.unique_albums, 5);
        assert_eq!(summary.plays, 5);
        let discovery = report_discovery(&rows, &refs, Some(150_000), None);
        assert_eq!(discovery.total_artists, 5);
        assert_eq!(discovery.new_artists, 4);
        assert!(profiles.reference("Creed").is_err());
        assert_eq!(profiles.reference("artist:2").unwrap().key(), "artist:2");
    }

    #[test]
    fn moved_tracks_use_unique_album_evidence_and_identical_albums_remain_unresolved() {
        let conn = catalog_fixture();
        conn.execute("UPDATE albums SET album='Album 1' WHERE id='artist:2'", [])
            .unwrap();
        let mut profiles = ArtistProfiles::new(Some(conn)).unwrap();
        let mut moved = row(5, "The Kids");
        moved.directory = "D:/Old".into();
        profiles.project(&mut moved).unwrap();
        assert_eq!(moved.artist_profile().id.as_deref(), Some("artist:5"));
        let mut ambiguous = row(1, "Creed");
        ambiguous.directory = "D:/Missing".into();
        profiles.project(&mut ambiguous).unwrap();
        assert_eq!(ambiguous.artist_profile().id, None);
        assert_eq!(
            ambiguous.artist_profile().disambiguation.as_deref(),
            Some("Unresolved artist")
        );
        let mut missing = row(99, "The Kids");
        profiles.project(&mut missing).unwrap();
        assert_eq!(missing.artist_profile().id, None);
        assert_eq!(
            missing.artist_profile().disambiguation.as_deref(),
            Some("Unresolved artist")
        );
        assert_eq!(ambiguous.artist, "Creed"); // Saved source evidence is unchanged.
        assert_eq!(ambiguous.directory, "D:/Missing");
    }

    #[test]
    fn compilation_plays_keep_the_performing_artist_and_album_owner() {
        let conn = catalog_fixture();
        conn.execute(
            "INSERT INTO artist_identities VALUES('kate bush','Kate Bush',NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artist_identities VALUES('various artists','Various Artists',NULL)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO tracks VALUES('compilation','Various Artists','Kate Bush','Song','D:/VA','07.mp3','various artists')",[]).unwrap();
        let mut profiles = ArtistProfiles::new(Some(conn)).unwrap();
        let mut compilation = HistoryRow {
            artist: "Various Artists".into(),
            directory: "D:/VA".into(),
            filename: "07.mp3".into(),
            ..Default::default()
        };
        profiles.project(&mut compilation).unwrap();
        assert_eq!(compilation.artist_profile().key(), "kate bush");
        assert_eq!(compilation.album_profile.unwrap().key(), "various artists");
    }

    #[test]
    fn artist_totals_and_twelve_month_detail_use_the_selected_profile() {
        let directory = tempfile::tempdir().unwrap();
        let store = HistoryStore::new_with_tonehavn_directory(
            directory.path().join("history.sqlite3"),
            directory.path().join("remote"),
            "device-profiles-test".into(),
            "Test".into(),
            None,
        )
        .unwrap();
        let conn = store.open().unwrap();
        for (index, name) in [
            (1, "Creed"),
            (2, "Creed"),
            (3, "The Kids"),
            (4, "The Kids"),
            (5, "The Kids"),
        ] {
            let row = row(index, name);
            conn.execute("INSERT INTO listening_sessions(session_id,track_key,title,artist,album,directory,filename,started_at_ms,listened_seconds,registered_play,threshold_seconds,outcome) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,30,1,30,'completed')",params![row.session_id,row.track_key,row.title,row.artist,row.album,row.directory,row.filename,state_sync::now_ms()-index*1000]).unwrap();
        }
        let mut profiles = ArtistProfiles::new(Some(catalog_fixture())).unwrap();
        let totals = store.artist_insights_with_profiles(&mut profiles).unwrap();
        assert_eq!(totals.len(), 5);
        for index in 1..=5 {
            let id = format!("artist:{index}");
            assert_eq!(totals[&id].plays, 1);
            let detail = store
                .artist_listening_with_profiles(&id, &mut profiles)
                .unwrap();
            assert_eq!(detail.plays, 1);
            assert_eq!(detail.listened_seconds, 30.0);
            assert_eq!(detail.months.values().sum::<i64>(), 1);
            assert_eq!(detail.top_tracks.len(), 1);
        }
        assert!(
            store
                .artist_listening_with_profiles("The Kids", &mut profiles)
                .is_err()
        );
    }
}
