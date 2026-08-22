use crate::catalog::{default_catalog_path, open_catalog};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::Serialize;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    time::Duration,
};

const MUSICBRAINZ_ROOT: &str = "OneDrive\\_musicbackup";
const CACHE_FILENAME: &str = "musicbrainz_cache.db";
const OVERLAY_FILENAME: &str = "musicbrainz-overlay-sync.sqlite3";
const MAX_ARTIST_CHARS: usize = 256;
const MAX_RELEASES: usize = 100;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicBrainzSource {
    id: &'static str,
    label: &'static str,
    status: &'static str,
    detail: String,
}

impl MusicBrainzSource {
    fn connected(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            status: "connected",
            detail: "Opened locally in read-only mode".to_owned(),
        }
    }

    fn unavailable(id: &'static str, label: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id,
            label,
            status: "unavailable",
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistIdentity {
    mbid: String,
    canonical_name: String,
    match_method: String,
    confidence: Option<f64>,
    provenance: &'static str,
    cache_name_count: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistProfile {
    sort_name: Option<String>,
    artist_type: Option<String>,
    gender: Option<String>,
    life_begin_date: Option<String>,
    life_end_date: Option<String>,
    life_ended: bool,
    area_name: Option<String>,
    begin_area_name: Option<String>,
    end_area_name: Option<String>,
    country_code: Option<String>,
    country_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicBrainzRelease {
    mbid: String,
    title: String,
    year: Option<i64>,
    primary_type: Option<String>,
    secondary_types: Vec<String>,
    status: Option<String>,
    track_count: Option<i64>,
    provenance: &'static str,
    decision: Option<String>,
    local_album_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistIntelligence {
    artist: String,
    match_state: &'static str,
    identity: Option<ArtistIdentity>,
    profile: Option<ArtistProfile>,
    releases: Vec<MusicBrainzRelease>,
    releases_truncated: bool,
    sources: Vec<MusicBrainzSource>,
}

#[derive(Clone, Debug)]
struct OverlayIdentity {
    display_artist: String,
    mbid: Option<String>,
    canonical_name: Option<String>,
    match_method: String,
    confidence: Option<f64>,
    verification_state: String,
    ignored: bool,
    provenance: &'static str,
}

#[derive(Clone, Debug)]
struct CacheIdentity {
    mbid: String,
    name_count: i64,
}

#[derive(Clone, Debug)]
struct CatalogIdentity {
    display_artist: String,
    mbid: String,
}

#[derive(Clone, Debug)]
struct ReleaseDecision {
    decision: String,
    local_album_id: Option<String>,
}

fn default_source_paths() -> Result<(PathBuf, PathBuf), String> {
    let profile = env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "Windows USERPROFILE is unavailable.".to_owned())?;
    let root = profile.join(MUSICBRAINZ_ROOT);
    Ok((root.join(CACHE_FILENAME), root.join(OVERLAY_FILENAME)))
}

fn validate_artist(artist: &str) -> Result<(String, String), String> {
    let artist = artist.trim();
    if artist.is_empty() || artist.chars().count() > MAX_ARTIST_CHARS {
        return Err("Artist identity is invalid.".to_owned());
    }
    let normalized_dashes: String = artist
        .chars()
        .map(|character| match character {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            _ => character,
        })
        .collect();
    let key = normalized_dashes
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    Ok((artist.to_owned(), key))
}

fn open_source(path: &Path, label: &str) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!("{label} was not found."));
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)
        .map_err(|error| format!("Could not open {label} read-only: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(|error| format!("Could not configure {label}: {error}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("Could not enforce read-only access for {label}: {error}"))?;
    Ok(connection)
}

fn overlay_identity(
    connection: &Connection,
    artist_key: &str,
    provenance: &'static str,
) -> rusqlite::Result<Option<OverlayIdentity>> {
    connection
        .query_row(
            "SELECT display_artist, mbid, canonical_name, match_method, confidence, verification_state, ignored
             FROM musicbrainz_artist_links WHERE local_artist_key = ?1",
            [artist_key],
            |row| {
                Ok(OverlayIdentity {
                    display_artist: row.get(0)?,
                    mbid: row.get(1)?,
                    canonical_name: row.get(2)?,
                    match_method: row.get(3)?,
                    confidence: row.get(4)?,
                    verification_state: row.get(5)?,
                    ignored: row.get::<_, i64>(6)? != 0,
                    provenance,
                })
            },
        )
        .optional()
}

fn cache_identity(
    connection: &Connection,
    artist_key: &str,
) -> rusqlite::Result<Option<CacheIdentity>> {
    connection
        .query_row(
            "SELECT a.mbid, (SELECT COUNT(*) FROM artist_cache AS aliases WHERE aliases.mbid = a.mbid)
             FROM artist_cache AS a
             WHERE a.name = ?1 AND a.mbid IS NOT NULL AND trim(a.mbid) <> ''",
            [artist_key],
            |row| {
                Ok(CacheIdentity {
                    mbid: row.get(0)?,
                    name_count: row.get(1)?,
                })
            },
        )
        .optional()
}

fn catalog_identity(
    connection: &Connection,
    artist_key: &str,
) -> rusqlite::Result<Option<CatalogIdentity>> {
    connection
        .query_row(
            "SELECT display_artist, mbid FROM musicbrainz_artist_infos
             WHERE local_artist_key = ?1 AND mbid IS NOT NULL AND trim(mbid) <> ''",
            [artist_key],
            |row| {
                Ok(CatalogIdentity {
                    display_artist: row.get(0)?,
                    mbid: row.get(1)?,
                })
            },
        )
        .optional()
}

fn catalog_profile(
    connection: &Connection,
    artist_key: &str,
    artist_mbid: &str,
) -> rusqlite::Result<Option<ArtistProfile>> {
    connection
        .query_row(
            "SELECT i.sort_name, i.artist_type, i.gender, i.life_begin_date, i.life_end_date,
                    COALESCE(i.life_ended, 0), i.area_name, i.begin_area_name, i.end_area_name,
                    o.country_code, o.country_name
             FROM musicbrainz_artist_infos AS i
             LEFT JOIN musicbrainz_artist_origin_countries AS o
               ON o.local_artist_key = i.local_artist_key AND o.mbid = i.mbid
             WHERE i.local_artist_key = ?1 AND i.mbid = ?2",
            params![artist_key, artist_mbid],
            |row| {
                Ok(ArtistProfile {
                    sort_name: row.get(0)?,
                    artist_type: row.get(1)?,
                    gender: row.get(2)?,
                    life_begin_date: row.get(3)?,
                    life_end_date: row.get(4)?,
                    life_ended: row.get::<_, i64>(5)? != 0,
                    area_name: row.get(6)?,
                    begin_area_name: row.get(7)?,
                    end_area_name: row.get(8)?,
                    country_code: row.get(9)?,
                    country_name: row.get(10)?,
                })
            },
        )
        .optional()
}

fn overlay_decisions(
    connection: &Connection,
    artist_key: &str,
) -> rusqlite::Result<HashMap<String, ReleaseDecision>> {
    let mut statement = connection.prepare(
        "SELECT release_mbid, decision, local_album_id
         FROM musicbrainz_release_decisions WHERE local_artist_key = ?1",
    )?;
    let rows = statement.query_map([artist_key], |row| {
        Ok((
            row.get::<_, String>(0)?,
            ReleaseDecision {
                decision: row.get(1)?,
                local_album_id: row.get(2)?,
            },
        ))
    })?;
    rows.collect()
}

fn cache_releases(
    connection: &Connection,
    artist_mbid: &str,
) -> rusqlite::Result<Vec<MusicBrainzRelease>> {
    let mut statement = connection.prepare(
        "SELECT release_mbid, title, year, type, secondary_types, track_count, status
         FROM release_groups WHERE artist_mbid = ?1
         ORDER BY COALESCE(year, -1) DESC, title COLLATE NOCASE, release_mbid LIMIT 101",
    )?;
    let rows = statement.query_map([artist_mbid], |row| {
        let secondary: Option<String> = row.get(4)?;
        Ok(MusicBrainzRelease {
            mbid: row.get(0)?,
            title: row.get(1)?,
            year: row.get(2)?,
            primary_type: row.get(3)?,
            secondary_types: split_secondary_types(secondary.as_deref()),
            track_count: row.get(5)?,
            status: row.get(6)?,
            provenance: "broadCache",
            decision: None,
            local_album_id: None,
        })
    })?;
    rows.collect()
}

fn overlay_releases(
    connection: &Connection,
    artist_mbid: &str,
) -> rusqlite::Result<Vec<MusicBrainzRelease>> {
    let mut statement = connection.prepare(
        "SELECT release_mbid, title, year, type, secondary_types, status
         FROM musicbrainz_artist_release_groups WHERE artist_mbid = ?1
         ORDER BY COALESCE(year, -1) DESC, title COLLATE NOCASE, release_mbid LIMIT 101",
    )?;
    let rows = statement.query_map([artist_mbid], |row| {
        let secondary: String = row.get(4)?;
        Ok(MusicBrainzRelease {
            mbid: row.get(0)?,
            title: row.get(1)?,
            year: row.get(2)?,
            primary_type: row.get(3)?,
            secondary_types: split_secondary_types(Some(&secondary)),
            track_count: None,
            status: row.get(5)?,
            provenance: "curatedOverlay",
            decision: None,
            local_album_id: None,
        })
    })?;
    rows.collect()
}

fn catalog_releases(
    connection: &Connection,
    artist_mbid: &str,
) -> rusqlite::Result<Vec<MusicBrainzRelease>> {
    let mut statement = connection.prepare(
        "SELECT release_mbid, title, year, type, secondary_types, track_count, status
         FROM musicbrainz_artist_release_groups WHERE artist_mbid = ?1
         ORDER BY COALESCE(year, -1) DESC, title COLLATE NOCASE, release_mbid LIMIT 101",
    )?;
    let rows = statement.query_map([artist_mbid], |row| {
        let secondary: String = row.get(4)?;
        Ok(MusicBrainzRelease {
            mbid: row.get(0)?,
            title: row.get(1)?,
            year: row.get(2)?,
            primary_type: row.get(3)?,
            secondary_types: split_secondary_types(Some(&secondary)),
            track_count: row.get(5)?,
            status: row.get(6)?,
            provenance: "catalogImport",
            decision: None,
            local_album_id: None,
        })
    })?;
    rows.collect()
}

fn split_secondary_types(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(8)
        .map(str::to_owned)
        .collect()
}

fn resolve_identity(
    artist: &str,
    overlay: Option<&OverlayIdentity>,
    catalog: Option<&CatalogIdentity>,
    cache: Option<&CacheIdentity>,
) -> (&'static str, Option<ArtistIdentity>) {
    if overlay.is_some_and(|identity| identity.ignored) {
        return ("ignored", None);
    }
    if let Some(curated) = overlay.filter(|identity| {
        identity.verification_state == "verified"
            && identity
                .mbid
                .as_ref()
                .is_some_and(|mbid| !mbid.trim().is_empty())
    }) {
        let mbid = curated
            .mbid
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_owned();
        let state = if cache.is_some_and(|candidate| candidate.mbid != mbid)
            || catalog.is_some_and(|candidate| candidate.mbid != mbid)
        {
            "conflict"
        } else {
            "verified"
        };
        let cache_name_count = cache
            .filter(|candidate| candidate.mbid == mbid)
            .map(|candidate| candidate.name_count);
        return (
            state,
            Some(ArtistIdentity {
                mbid,
                canonical_name: curated
                    .canonical_name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| curated.display_artist.clone()),
                match_method: curated.match_method.clone(),
                confidence: curated.confidence,
                provenance: curated.provenance,
                cache_name_count,
            }),
        );
    }
    if let Some(imported) = catalog {
        if cache.is_some_and(|candidate| candidate.mbid != imported.mbid) {
            return ("conflict", None);
        }
        return (
            "unconfirmed",
            Some(ArtistIdentity {
                mbid: imported.mbid.clone(),
                canonical_name: imported.display_artist.clone(),
                match_method: "catalog-import".to_owned(),
                confidence: None,
                provenance: "catalogImport",
                cache_name_count: cache.map(|candidate| candidate.name_count),
            }),
        );
    }
    if let Some(exact) = cache {
        return (
            "unconfirmed",
            Some(ArtistIdentity {
                mbid: exact.mbid.clone(),
                canonical_name: artist.to_owned(),
                match_method: "exact-name-cache".to_owned(),
                confidence: None,
                provenance: "cacheExact",
                cache_name_count: Some(exact.name_count),
            }),
        );
    }
    ("unmatched", None)
}

fn finalize_releases(
    mut releases: Vec<MusicBrainzRelease>,
    decisions: &HashMap<String, ReleaseDecision>,
) -> (Vec<MusicBrainzRelease>, bool) {
    for release in &mut releases {
        if let Some(decision) = decisions.get(&release.mbid) {
            release.decision = Some(decision.decision.clone());
            release.local_album_id = decision.local_album_id.clone();
        }
    }
    releases.sort_by(|left, right| {
        right
            .year
            .unwrap_or(-1)
            .cmp(&left.year.unwrap_or(-1))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.mbid.cmp(&right.mbid))
    });
    let truncated = releases.len() > MAX_RELEASES;
    releases.truncate(MAX_RELEASES);
    (releases, truncated)
}

pub(crate) fn load_artist_intelligence(artist: String) -> Result<ArtistIntelligence, String> {
    let (artist, artist_key) = validate_artist(&artist)?;
    let (cache_path, overlay_path) = default_source_paths()?;

    let mut cache_source = MusicBrainzSource::connected("broadCache", "Broad MusicBrainz cache");
    let mut overlay_source =
        MusicBrainzSource::connected("curatedOverlay", "Curated MusicBrainz overlay");
    let mut catalog_source = MusicBrainzSource::connected("catalog", "Music Library catalog");
    let mut catalog = match default_catalog_path().and_then(|path| open_catalog(&path)) {
        Ok(connection) => Some(connection),
        Err(error) => {
            catalog_source =
                MusicBrainzSource::unavailable("catalog", "Music Library catalog", error);
            None
        }
    };
    let mut cache = match open_source(&cache_path, "the broad MusicBrainz cache") {
        Ok(connection) => Some(connection),
        Err(error) => {
            cache_source =
                MusicBrainzSource::unavailable("broadCache", "Broad MusicBrainz cache", error);
            None
        }
    };
    let mut overlay = match open_source(&overlay_path, "the curated MusicBrainz overlay") {
        Ok(connection) => Some(connection),
        Err(error) => {
            overlay_source = MusicBrainzSource::unavailable(
                "curatedOverlay",
                "Curated MusicBrainz overlay",
                error,
            );
            None
        }
    };

    let cached_identity = match cache
        .as_ref()
        .map(|connection| cache_identity(connection, &artist_key))
        .transpose()
    {
        Ok(value) => value.flatten(),
        Err(error) => {
            cache_source = MusicBrainzSource::unavailable(
                "broadCache",
                "Broad MusicBrainz cache",
                format!("The local cache could not be read: {error}"),
            );
            cache = None;
            None
        }
    };
    let curated_identity = match overlay
        .as_ref()
        .map(|connection| overlay_identity(connection, &artist_key, "curatedOverlay"))
        .transpose()
    {
        Ok(value) => value.flatten(),
        Err(error) => {
            overlay_source = MusicBrainzSource::unavailable(
                "curatedOverlay",
                "Curated MusicBrainz overlay",
                format!("The curated overlay could not be read: {error}"),
            );
            overlay = None;
            None
        }
    };
    let imported_identity = match catalog
        .as_ref()
        .map(|connection| catalog_identity(connection, &artist_key))
        .transpose()
    {
        Ok(value) => value.flatten(),
        Err(error) => {
            catalog_source = MusicBrainzSource::unavailable(
                "catalog",
                "Music Library catalog",
                format!("Imported MusicBrainz identity could not be read: {error}"),
            );
            catalog = None;
            None
        }
    };
    let catalog_curated_identity = if curated_identity.is_none() {
        match catalog
            .as_ref()
            .map(|connection| overlay_identity(connection, &artist_key, "catalogOverlay"))
            .transpose()
        {
            Ok(value) => value.flatten(),
            Err(error) => {
                catalog_source = MusicBrainzSource::unavailable(
                    "catalog",
                    "Music Library catalog",
                    format!("Embedded curated identity could not be read: {error}"),
                );
                None
            }
        }
    } else {
        None
    };
    let curated_identity = curated_identity.or(catalog_curated_identity);
    let (match_state, identity) = resolve_identity(
        &artist,
        curated_identity.as_ref(),
        imported_identity.as_ref(),
        cached_identity.as_ref(),
    );

    let mut profile = None;
    let mut cached_releases = None;
    let mut catalog_release_groups = None;
    let mut curated_releases = None;
    let mut decisions = HashMap::new();
    if let Some(identity) = &identity {
        if let Some(connection) = cache.as_ref() {
            match cache_releases(connection, &identity.mbid) {
                Ok(releases) => cached_releases = Some(releases),
                Err(error) => {
                    cache_source = MusicBrainzSource::unavailable(
                        "broadCache",
                        "Broad MusicBrainz cache",
                        format!("Release groups could not be read: {error}"),
                    );
                }
            }
        }
        if let Some(connection) = catalog.as_ref() {
            match catalog_profile(connection, &artist_key, &identity.mbid) {
                Ok(value) => profile = value,
                Err(error) => {
                    catalog_source = MusicBrainzSource::unavailable(
                        "catalog",
                        "Music Library catalog",
                        format!("Artist profile could not be read: {error}"),
                    );
                }
            }
            match catalog_releases(connection, &identity.mbid) {
                Ok(releases) => catalog_release_groups = Some(releases),
                Err(error) => {
                    catalog_source = MusicBrainzSource::unavailable(
                        "catalog",
                        "Music Library catalog",
                        format!("Imported release groups could not be read: {error}"),
                    );
                }
            }
        }
        if let Some(connection) = overlay.as_ref() {
            match overlay_releases(connection, &identity.mbid) {
                Ok(releases) => curated_releases = Some(releases),
                Err(error) => {
                    overlay_source = MusicBrainzSource::unavailable(
                        "curatedOverlay",
                        "Curated MusicBrainz overlay",
                        format!("Curated release groups could not be read: {error}"),
                    );
                }
            }
            match overlay_decisions(connection, &artist_key) {
                Ok(values) => decisions = values,
                Err(error) => {
                    overlay_source = MusicBrainzSource::unavailable(
                        "curatedOverlay",
                        "Curated MusicBrainz overlay",
                        format!("Release decisions could not be read: {error}"),
                    );
                    curated_releases = None;
                    decisions.clear();
                }
            }
        }
    }
    let preferred_releases = if matches!(match_state, "verified" | "conflict") {
        curated_releases
            .filter(|releases| !releases.is_empty())
            .or_else(|| catalog_release_groups.filter(|releases| !releases.is_empty()))
            .or(cached_releases)
    } else {
        catalog_release_groups
            .filter(|releases| !releases.is_empty())
            .or(cached_releases)
    }
    .unwrap_or_default();
    let (releases, releases_truncated) = finalize_releases(preferred_releases, &decisions);

    Ok(ArtistIntelligence {
        artist,
        match_state,
        identity,
        profile,
        releases,
        releases_truncated,
        sources: vec![catalog_source, overlay_source, cache_source],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE artist_cache(name TEXT PRIMARY KEY, mbid TEXT, cached_at TEXT);
                 CREATE TABLE release_groups(
                   artist_mbid TEXT, release_mbid TEXT, title TEXT, year INTEGER, type TEXT,
                   secondary_types TEXT, track_count INTEGER, status TEXT, cached_at TEXT,
                   PRIMARY KEY(artist_mbid, release_mbid));",
            )
            .unwrap();
        connection
    }

    fn overlay_fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE musicbrainz_artist_links(
                   local_artist_key TEXT PRIMARY KEY, display_artist TEXT NOT NULL, mbid TEXT,
                   canonical_name TEXT, match_method TEXT NOT NULL, confidence REAL,
                   verification_state TEXT NOT NULL, ignored INTEGER NOT NULL);
                 CREATE TABLE musicbrainz_release_decisions(
                   local_artist_key TEXT, release_mbid TEXT, decision TEXT, local_album_id TEXT);
                 CREATE TABLE musicbrainz_artist_release_groups(
                   artist_mbid TEXT, release_mbid TEXT, title TEXT, year INTEGER, type TEXT,
                   secondary_types TEXT, status TEXT, source TEXT, fetched_at TEXT,
                   PRIMARY KEY(artist_mbid, release_mbid));",
            )
            .unwrap();
        connection
    }

    #[test]
    fn curated_identity_wins_and_reports_an_exact_cache_conflict() {
        let cache = CacheIdentity {
            mbid: "cache-mbid".to_owned(),
            name_count: 1,
        };
        let overlay = OverlayIdentity {
            display_artist: "M83".to_owned(),
            mbid: Some("curated-mbid".to_owned()),
            canonical_name: Some("M83".to_owned()),
            match_method: "manual-mbid".to_owned(),
            confidence: Some(1.0),
            verification_state: "verified".to_owned(),
            ignored: false,
            provenance: "curatedOverlay",
        };
        let (state, identity) = resolve_identity("M83", Some(&overlay), None, Some(&cache));
        assert_eq!(state, "conflict");
        assert_eq!(identity.unwrap().mbid, "curated-mbid");
    }

    #[test]
    fn exact_cache_match_is_explicitly_unverified() {
        let cache = CacheIdentity {
            mbid: "cache-mbid".to_owned(),
            name_count: 2,
        };
        let (state, identity) = resolve_identity("M83", None, None, Some(&cache));
        assert_eq!(state, "unconfirmed");
        assert_eq!(identity.unwrap().match_method, "exact-name-cache");
    }

    #[test]
    fn ignored_artist_never_uses_the_broad_cache() {
        let overlay = OverlayIdentity {
            display_artist: "Ambiguous".to_owned(),
            mbid: None,
            canonical_name: None,
            match_method: "ignored".to_owned(),
            confidence: None,
            verification_state: "unverified".to_owned(),
            ignored: true,
            provenance: "curatedOverlay",
        };
        let cache = CacheIdentity {
            mbid: "wrong".to_owned(),
            name_count: 1,
        };
        let (state, identity) = resolve_identity("Ambiguous", Some(&overlay), None, Some(&cache));
        assert_eq!(state, "ignored");
        assert!(identity.is_none());
    }

    #[test]
    fn blank_curated_mbid_is_never_verified() {
        let overlay = OverlayIdentity {
            display_artist: "Broken".to_owned(),
            mbid: Some("   ".to_owned()),
            canonical_name: None,
            match_method: "manual-mbid".to_owned(),
            confidence: Some(1.0),
            verification_state: "verified".to_owned(),
            ignored: false,
            provenance: "curatedOverlay",
        };
        let (state, identity) = resolve_identity("Broken", Some(&overlay), None, None);
        assert_eq!(state, "unmatched");
        assert!(identity.is_none());
    }

    #[test]
    fn release_decision_is_annotated_without_mixing_sources() {
        let mut decisions = HashMap::new();
        decisions.insert(
            "release-1".to_owned(),
            ReleaseDecision {
                decision: "included".to_owned(),
                local_album_id: Some("album-1".to_owned()),
            },
        );
        let release = |title: &str, provenance| MusicBrainzRelease {
            mbid: "release-1".to_owned(),
            title: title.to_owned(),
            year: Some(2024),
            primary_type: Some("Album".to_owned()),
            secondary_types: vec![],
            status: Some("Official".to_owned()),
            track_count: None,
            provenance,
            decision: None,
            local_album_id: None,
        };
        let (releases, truncated) =
            finalize_releases(vec![release("Curated", "curatedOverlay")], &decisions);
        assert!(!truncated);
        assert_eq!(releases[0].title, "Curated");
        assert_eq!(releases[0].decision.as_deref(), Some("included"));
        assert_eq!(releases[0].local_album_id.as_deref(), Some("album-1"));
    }

    #[test]
    fn queries_are_bounded_to_one_hundred_releases() {
        let cache = cache_fixture();
        cache
            .execute(
                "INSERT INTO artist_cache(name, mbid) VALUES ('m83', 'artist-1')",
                [],
            )
            .unwrap();
        for index in 0..130 {
            cache
                .execute(
                    "INSERT INTO release_groups(artist_mbid, release_mbid, title, year)
                     VALUES ('artist-1', ?1, ?2, 2024)",
                    params![format!("release-{index:03}"), format!("Release {index:03}")],
                )
                .unwrap();
        }
        let releases = cache_releases(&cache, "artist-1").unwrap();
        let (releases, truncated) = finalize_releases(releases, &HashMap::new());
        assert_eq!(releases.len(), 100);
        assert!(truncated);
    }

    #[test]
    fn fixture_queries_read_curated_rows() {
        let overlay = overlay_fixture();
        overlay
            .execute(
                "INSERT INTO musicbrainz_artist_links VALUES
                 ('m83', 'M83', 'artist-1', 'M83', 'manual-mbid', 1.0, 'verified', 0)",
                [],
            )
            .unwrap();
        assert_eq!(
            overlay_identity(&overlay, "m83", "curatedOverlay")
                .unwrap()
                .unwrap()
                .mbid
                .as_deref(),
            Some("artist-1")
        );
    }

    #[test]
    fn validates_artist_bounds() {
        assert!(validate_artist("  M83  ").is_ok());
        assert_eq!(
            validate_artist("  AC\u{2013}DC   Live ").unwrap().1,
            "ac-dc live"
        );
        assert!(validate_artist("").is_err());
        assert!(validate_artist(&"x".repeat(257)).is_err());
    }

    #[test]
    #[ignore = "requires Jørn's live local music databases"]
    fn live_sources_cover_verified_conflict_and_bounded_release_states() {
        let adele = load_artist_intelligence("Adele".to_owned()).unwrap();
        assert_eq!(adele.match_state, "verified");
        assert!(adele.profile.is_some());
        assert!(!adele.releases.is_empty());

        let kiss = load_artist_intelligence("KISS".to_owned()).unwrap();
        assert_eq!(kiss.match_state, "verified");
        assert_eq!(kiss.releases.len(), MAX_RELEASES);
        assert!(kiss.releases_truncated);

        let conflict = load_artist_intelligence("2-11".to_owned()).unwrap();
        assert_eq!(conflict.match_state, "conflict");
        assert_eq!(
            conflict
                .identity
                .as_ref()
                .map(|identity| identity.mbid.as_str()),
            Some("6c5c9a0e-5033-4460-8a91-4a90aea9f282")
        );
    }
}
