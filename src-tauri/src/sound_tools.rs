//! The assistant's ears: `describe_sound` and `tracks_like` for Aurora's MCP
//! server.
//!
//! Both only read data Aurora already has, and never return file paths:
//!
//! - `describe_sound` reports an album's or a track's sound descriptors (from
//!   `sound_descriptors`), and how well it fits a description in words,
//!   ranked against the whole library (from `vibe`).
//! - `tracks_like` lists the tracks or albums that sound most alike by Music
//!   Library's audio analysis, the same matches as Aurora's "Sounds like",
//!   optionally narrowed by an Aurora search such as `cr:0`.
use crate::{
    catalog, sonic,
    sonic_albums::SonicAlbumRequest,
    sound_descriptors::model,
    state_store::StateStore,
    vibe::{self, Fit},
};
use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value as Sql};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub(crate) const MAX_ALBUMS: usize = 25;
pub(crate) const MAX_TRACKS: usize = 50;
pub(crate) const MAX_LIKE: usize = 50;
/// A filtered `tracks_like` reads this many matches before filtering.
const FILTER_POOL: usize = 100;

// ---------------------------------------------------------------------------
// describe_sound
// ---------------------------------------------------------------------------

pub(crate) fn describe(
    album_ids: &[String],
    track_ids: &[String],
    description: Option<&str>,
) -> Result<Value, String> {
    check_ids(album_ids, track_ids)?;
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    describe_with(
        &connection,
        album_ids,
        track_ids,
        description,
        &|text, ids| vibe::album_fits(text, ids),
        &|text, hashes| vibe::track_fits(text, hashes),
    )
}

type Fits<'a> = &'a dyn Fn(&str, &[String]) -> Result<HashMap<String, Fit>, String>;

fn check_ids(album_ids: &[String], track_ids: &[String]) -> Result<(), String> {
    if album_ids.is_empty() && track_ids.is_empty() {
        return Err("Give albumIds or trackIds from a search.".to_owned());
    }
    if album_ids.len() > MAX_ALBUMS || track_ids.len() > MAX_TRACKS {
        return Err(format!(
            "Describe at most {MAX_ALBUMS} albums and {MAX_TRACKS} tracks at a time."
        ));
    }
    Ok(())
}

fn describe_with(
    c: &Connection,
    album_ids: &[String],
    track_ids: &[String],
    description: Option<&str>,
    album_fits: Fits<'_>,
    track_fits: Fits<'_>,
) -> Result<Value, String> {
    let mut albums = Vec::new();
    for id in album_ids {
        albums.push(describe_album(c, id)?);
    }
    let mut tracks = Vec::new();
    for id in track_ids {
        tracks.push(describe_track(c, id)?);
    }
    let mut result = json!({});
    if let Some(text) = description {
        let text = vibe::normalize(text)?;
        result["description"] = json!(text);
        let mut errors = Vec::new();
        if !albums.is_empty() {
            add_fits(
                &mut albums,
                "albumId",
                album_fits(&text, album_ids),
                &mut errors,
            );
        }
        if !tracks.is_empty() {
            let hashes: Vec<String> = tracks
                .iter()
                .filter_map(|track| track["audioHash"].as_str().map(str::to_owned))
                .collect();
            add_fits(
                &mut tracks,
                "audioHash",
                track_fits(&text, &hashes),
                &mut errors,
            );
        }
        if let Some(error) = errors.into_iter().next() {
            result["fitError"] = json!(error);
        }
    }
    for track in &mut tracks {
        if let Some(object) = track.as_object_mut() {
            // Audio hashes identify files; they stay inside Aurora.
            object.remove("audioHash");
        }
    }
    if !albums.is_empty() {
        result["albums"] = json!(albums);
    }
    if !tracks.is_empty() {
        result["tracks"] = json!(tracks);
    }
    result["about"] = json!(ABOUT);
    Ok(result)
}

const ABOUT: &str = "descriptors are what Aurora's sound: search matches; trackShares is the percent of the album's analyzed tracks with each descriptor. fit.percentile is the share of the library's albums (or tracks) this one fits the description better than; fit.matchesVibe says whether vibe: with the description finds it.";

fn fit_value(fit: Fit) -> Value {
    json!({ "percentile": fit.percentile, "matchesVibe": fit.in_vibe })
}

fn add_fits(
    items: &mut [Value],
    key: &str,
    fits: Result<HashMap<String, Fit>, String>,
    errors: &mut Vec<String>,
) {
    match fits {
        Ok(fits) => {
            for item in items {
                let fit = item[key].as_str().and_then(|id| fits.get(id)).copied();
                item["fit"] = fit.map_or(Value::Null, fit_value);
            }
        }
        Err(error) => errors.push(error),
    }
}

fn track_id(id: &str) -> Result<i64, String> {
    id.trim()
        .parse()
        .map_err(|_| format!("{id} is not a track ID from a search."))
}

fn label_names(mask: u32) -> Vec<&'static str> {
    model()
        .labels
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, label)| label.name.as_str())
        .collect()
}

fn label_name(top: Option<i64>) -> Option<&'static str> {
    top.and_then(|top| usize::try_from(top).ok())
        .and_then(|top| model().labels.get(top))
        .map(|label| label.name.as_str())
}

fn describe_album(c: &Connection, id: &str) -> Result<Value, String> {
    let (title, artist, year, genre, tracks): (
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        i64,
    ) = c
        .query_row(
            "SELECT a.album, a.album_artist_display, COALESCE(a.year, a.release_year), a.canonical_genre, (SELECT count(*) FROM tracks AS t WHERE t.album_id = a.id) FROM albums AS a WHERE a.id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("No album has the ID {id}."))?;
    let mut statement = c
        .prepare_cached(
            "SELECT s.mask FROM tracks AS t JOIN temp.aurora_sound_tracks AS s ON s.file_path = t.file_path AND s.filename = t.filename WHERE t.album_id = ?1",
        )
        .map_err(|error| error.to_string())?;
    let masks = statement
        .query_map([id], |row| row.get::<_, u32>(0))
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(|error| error.to_string())?;
    let rollup: Option<(u32, Option<i64>)> = c
        .query_row(
            "SELECT mask, top FROM temp.aurora_sound_albums WHERE album_id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let analyzed = masks.len();
    let mut shares: Vec<(usize, &str)> = model()
        .labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let count = masks
                .iter()
                .filter(|mask| *mask & (1 << index) != 0)
                .count();
            (count, label.name.as_str())
        })
        .filter(|(count, _)| *count > 0)
        .collect();
    shares.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let mut album = json!({
        "albumId": id,
        "title": title,
        "artist": artist,
        "year": year,
        "genre": genre,
        "tracks": tracks,
        "analyzedTracks": analyzed,
    });
    if analyzed == 0 {
        album["note"] = json!("No track of this album has sound analysis yet.");
        return Ok(album);
    }
    album["descriptors"] = json!(
        rollup
            .map(|(mask, _)| label_names(mask))
            .unwrap_or_default()
    );
    album["mainDescriptor"] = json!(rollup.and_then(|(_, top)| label_name(top)));
    album["trackShares"] = shares
        .into_iter()
        .map(|(count, name)| {
            json!({
                "descriptor": name,
                "percent": (100.0 * count as f64 / analyzed as f64).round(),
            })
        })
        .collect();
    Ok(album)
}

fn describe_track(c: &Connection, id: &str) -> Result<Value, String> {
    type Row = (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<u32>,
        Option<i64>,
        Option<String>,
    );
    let row: Row = c
        .query_row(
            "SELECT t.title, COALESCE(t.display_artist, t.album_artist_display), t.album, t.album_id, COALESCE(t.year, t.release_year), t.canonical_genre, s.mask, s.top, (SELECT p.audio_hash FROM temp.aurora_sonic_paths AS p WHERE p.file_path = t.file_path AND p.filename = t.filename LIMIT 1) FROM tracks AS t LEFT JOIN temp.aurora_sound_tracks AS s ON s.file_path = t.file_path AND s.filename = t.filename WHERE t.id = ?1",
            [track_id(id)?],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("No track has the ID {id}."))?;
    let (title, artist, album, album_id, year, genre, mask, top, hash) = row;
    let mut track = json!({
        "trackId": id.trim(),
        "title": title,
        "artist": artist,
        "album": album,
        "albumId": album_id,
        "year": year,
        "genre": genre,
        "audioHash": hash,
    });
    match mask {
        Some(mask) => {
            track["descriptors"] = json!(label_names(mask));
            track["mainDescriptor"] = json!(label_name(top));
        }
        None => track["note"] = json!("This track has no sound analysis yet."),
    }
    Ok(track)
}

// ---------------------------------------------------------------------------
// tracks_like
// ---------------------------------------------------------------------------

pub(crate) struct LikeRequest {
    pub(crate) track_id: Option<String>,
    pub(crate) album_id: Option<String>,
    /// `albums` for album results; tracks otherwise.
    pub(crate) albums: Option<bool>,
    pub(crate) limit: usize,
    pub(crate) query: Option<String>,
    pub(crate) same_genre: bool,
    pub(crate) minimum_rating: Option<f64>,
}

pub(crate) fn like(request: LikeRequest, store: &StateStore) -> Result<Value, String> {
    if !(1..=MAX_LIKE).contains(&request.limit) {
        return Err(format!("limit must be from 1 to {MAX_LIKE}."));
    }
    let catalog_path = catalog::default_catalog_path()?;
    let c = catalog::open_catalog(&catalog_path)?;
    catalog::attach_local_aurora_state(&c, store)?;
    let filter = request
        .query
        .as_deref()
        .map(parse_filter)
        .transpose()?
        .flatten();
    let pool = if filter.is_some() {
        FILTER_POOL
    } else {
        request.limit
    };
    let (seed_key, seed_album) = match (&request.track_id, &request.album_id) {
        (Some(id), None) => {
            let (key, album): (String, Option<String>) = c
                .query_row(
                    "SELECT file_path, filename, album_id FROM tracks WHERE id = ?1",
                    [track_id(id)?],
                    |row| {
                        Ok((
                            catalog::normalize_track_key(
                                &row.get::<_, String>(0)?,
                                &row.get::<_, String>(1)?,
                            ),
                            row.get(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("No track has the ID {id}."))?;
            (Some(key), album)
        }
        (None, Some(id)) => (None, Some(id.clone())),
        _ => return Err("Give either trackId or albumId.".to_owned()),
    };
    let albums = request.albums.unwrap_or(seed_key.is_none());
    if albums {
        let album_id = seed_album.ok_or("This track has no album to compare.")?;
        if request.same_genre || request.minimum_rating.is_some() {
            return Err(
                "sameGenre and minimumRating apply to track results; narrow albums with query, like genre:ambient or rating:4...".to_owned(),
            );
        }
        let matches = sonic::album_query(
            SonicAlbumRequest {
                album_id: album_id.clone(),
                limit: pool,
                minimum_coverage: 50,
            },
            store,
        )?;
        if !matches.seed_ready {
            return Err(not_analyzed("album"));
        }
        let mut ids: Vec<String> = matches.albums.iter().map(|a| a.album_id.clone()).collect();
        if let Some(filter) = &filter {
            keep_matching(&c, &mut ids, filter, true)?;
        }
        ids.truncate(request.limit);
        let details = album_details(&c, &ids)?;
        let found: HashMap<&str, &crate::sonic_albums::SonicAlbum> = matches
            .albums
            .iter()
            .map(|album| (album.album_id.as_str(), album))
            .collect();
        let items: Vec<Value> = ids
            .iter()
            .filter_map(|id| {
                let album = found.get(id.as_str())?;
                let mut item = json!({
                    "albumId": album.album_id,
                    "title": album.title,
                    "artist": album.album_artist,
                    "genre": album.genre,
                    "tracks": album.total_tracks,
                    "analyzedTracks": album.analyzed_tracks,
                    "distance": album.distance.map(|d| (d * 1000.0).round() / 1000.0),
                });
                if let (Some(detail), Some(object)) = (details.get(id), item.as_object_mut()) {
                    object.extend(detail.clone());
                }
                Some(item)
            })
            .collect();
        return Ok(json!({
            "seedAlbumId": album_id,
            "albums": items,
            "about": "Albums that sound most like the seed by Music Library's audio analysis, closest first (smaller distance).",
        }));
    }
    let response = sonic::query(
        sonic::SonicRequest {
            seed_key: seed_key.clone().unwrap_or_default(),
            seed_album_id: if seed_key.is_none() { seed_album } else { None },
            minimum_coverage: 50,
            exclude_keys: Vec::new(),
            limit: pool,
            minimum_rating: request.minimum_rating,
            same_genre: request.same_genre,
            radio: true,
        },
        store,
    )?;
    if !response.seed_ready {
        return Err(not_analyzed(if seed_key.is_some() {
            "track"
        } else {
            "album"
        }));
    }
    let mut tracks = response.tracks;
    if let Some(filter) = &filter {
        let mut ids: Vec<String> = tracks.iter().map(|track| track.id.clone()).collect();
        keep_matching(&c, &mut ids, filter, false)?;
        let kept: HashSet<String> = ids.into_iter().collect();
        tracks.retain(|track| kept.contains(&track.id));
    }
    tracks.truncate(request.limit);
    let items: Vec<Value> = tracks
        .into_iter()
        .map(|track| {
            json!({
                "trackId": track.id,
                "title": track.title,
                "artist": track.display_artist.unwrap_or(track.artist),
                "album": track.album,
                "albumId": track.album_id,
                "year": track.original_year.or(track.release_year),
                "genre": track.genre,
                "rating": track.rating,
                "loved": track.loved,
            })
        })
        .collect();
    Ok(json!({
        "seed": request.track_id.map_or_else(|| json!({ "albumId": request.album_id }), |id| json!({ "trackId": id })),
        "tracks": items,
        "about": "Tracks that sound most like the seed by Music Library's audio analysis, closest first, at most three per artist and two per album.",
    }))
}

fn not_analyzed(what: &str) -> String {
    format!(
        "That {what} has no sound analysis yet, so nothing can be compared with it. Music Library analyzes the library's audio in the background."
    )
}

/// An Aurora search for narrowing matches; `None` when it holds nothing.
fn parse_filter(query: &str) -> Result<Option<catalog::CatalogSearch>, String> {
    let lower = query.to_lowercase();
    if lower.contains("sort:") || lower.contains("group:") {
        return Err("query narrows the matches; it cannot use sort: or group:, because the closest matches come first.".to_owned());
    }
    let search = catalog::parse_catalog_search(query)?;
    Ok((!search.is_empty()).then_some(search))
}

/// Keeps the ids, in order, that match the search.
fn keep_matching(
    c: &Connection,
    ids: &mut Vec<String>,
    search: &catalog::CatalogSearch,
    albums: bool,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let mut params: Vec<Sql> = if albums {
        ids.iter().map(|id| Sql::Text(id.clone())).collect()
    } else {
        ids.iter()
            .map(|id| track_id(id).map(Sql::Integer))
            .collect::<Result<_, _>>()?
    };
    let mut sql = if albums {
        format!("SELECT a.id FROM albums AS a WHERE a.id IN ({placeholders})")
    } else {
        format!("SELECT CAST(t.id AS TEXT) FROM tracks AS t WHERE t.id IN ({placeholders})")
    };
    if albums {
        catalog::push_album_search_predicates(&mut sql, &mut params, search);
    } else {
        catalog::push_track_search_predicates(&mut sql, &mut params, search);
    }
    let mut statement = c
        .prepare(&sql)
        .map_err(|error| format!("Could not narrow the matches: {error}"))?;
    let kept: HashSet<String> = statement
        .query_map(params_from_iter(params), |row| row.get(0))
        .and_then(Iterator::collect)
        .map_err(|error| format!("Could not narrow the matches: {error}"))?;
    ids.retain(|id| kept.contains(id));
    Ok(())
}

/// Year and live rating for album results.
fn album_details(
    c: &Connection,
    ids: &[String],
) -> Result<HashMap<String, serde_json::Map<String, Value>>, String> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT a.id, COALESCE(a.year, a.release_year), COALESCE(live.rating, a.effective_album_rating), {rated} FROM albums AS a LEFT JOIN temp.aurora_live_album_metrics AS live ON live.album_id = a.id WHERE a.id IN ({placeholders})",
        rated = catalog::live_album_rated_tracks_sql("a"),
    );
    let mut statement = c.prepare(&sql).map_err(|error| error.to_string())?;
    statement
        .query_map(params_from_iter(ids), |row| {
            let mut detail = serde_json::Map::new();
            detail.insert("year".to_owned(), json!(row.get::<_, Option<i64>>(1)?));
            let rating = row
                .get::<_, Option<f64>>(2)?
                .filter(|rating| *rating > 0.0)
                .map(|rating| (rating / 10.0).round() / 2.0);
            detail.insert("rating".to_owned(), json!(rating));
            detail.insert("ratedTracks".to_owned(), json!(row.get::<_, i64>(3)?));
            Ok((row.get::<_, String>(0)?, detail))
        })
        .and_then(Iterator::collect)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> (tempfile::TempDir, Connection) {
        let (dir, c) = crate::sound_descriptors::tests::search_connection();
        crate::catalog::register_catalog_functions(&c).unwrap();
        c.execute_batch(
            "ALTER TABLE albums ADD COLUMN year INTEGER; ALTER TABLE albums ADD COLUMN release_year INTEGER; ALTER TABLE albums ADD COLUMN canonical_genre TEXT; UPDATE albums SET year = 1982, canonical_genre = 'Soundtrack';",
        )
        .unwrap();
        (dir, c)
    }

    fn fits(pairs: &[(&str, f64, bool)]) -> HashMap<String, Fit> {
        pairs
            .iter()
            .map(|(key, percentile, in_vibe)| {
                (
                    (*key).to_owned(),
                    Fit {
                        percentile: *percentile,
                        in_vibe: *in_vibe,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn describes_albums_by_their_tracks_descriptors() {
        let (_dir, c) = connection();
        let ids = ["A", "B", "C"].map(str::to_owned);
        let no_fits: Fits<'_> = &|_, _| panic!("no description was given");
        let result = describe_with(&c, &ids, &[], None, no_fits, no_fits).unwrap();
        let albums = result["albums"].as_array().unwrap();
        assert_eq!(albums[0]["descriptors"], json!(["Choir"]));
        assert_eq!(albums[0]["mainDescriptor"], "Choir");
        assert_eq!(
            albums[0]["trackShares"],
            json!([{ "descriptor": "Choir", "percent": 100.0 }])
        );
        assert_eq!(albums[0]["year"], 1982);
        assert_eq!(albums[0]["genre"], "Soundtrack");
        // B: two synth tracks and one choir track, and a third is enough.
        assert_eq!(albums[1]["mainDescriptor"], "Synth");
        assert_eq!(
            albums[1]["trackShares"],
            json!([
                { "descriptor": "Synth", "percent": 67.0 },
                { "descriptor": "Choir", "percent": 33.0 },
            ])
        );
        assert_eq!(albums[2]["analyzedTracks"], 0);
        assert!(albums[2].get("descriptors").is_none());
        assert!(describe_with(&c, &["nope".to_owned()], &[], None, no_fits, no_fits).is_err());
    }

    #[test]
    fn describes_tracks_and_ranks_them_against_a_description() {
        let (_dir, c) = connection();
        let tracks = ["4", "7"].map(str::to_owned);
        let albums = ["A".to_owned()];
        let album_fits: Fits<'_> = &|text, ids| {
            assert_eq!(text, "warm analog synth");
            assert_eq!(ids, ["A"]);
            Ok(fits(&[("A", 12.5, false)]))
        };
        let track_fits: Fits<'_> = &|_, hashes| {
            // Track 7 has no analysis, so only track 4 is looked up.
            assert_eq!(hashes, ["hash4"]);
            Ok(fits(&[("hash4", 99.1, true)]))
        };
        let result = describe_with(
            &c,
            &albums,
            &tracks,
            Some("  Warm ANALOG synth"),
            album_fits,
            track_fits,
        )
        .unwrap();
        assert_eq!(result["description"], "warm analog synth");
        assert_eq!(
            result["albums"][0]["fit"],
            json!({ "percentile": 12.5, "matchesVibe": false })
        );
        let tracks = result["tracks"].as_array().unwrap();
        assert_eq!(tracks[0]["descriptors"], json!(["Synth"]));
        assert_eq!(
            tracks[0]["fit"],
            json!({ "percentile": 99.1, "matchesVibe": true })
        );
        assert!(tracks[0].get("audioHash").is_none());
        assert!(tracks[1]["note"].is_string());
        assert!(tracks[1]["fit"].is_null());

        // Without Music Library's worker the descriptors still come back.
        let failing: Fits<'_> = &|_, _| Err("no worker".to_owned());
        let result = describe_with(&c, &albums, &[], Some("x"), failing, failing).unwrap();
        assert_eq!(result["fitError"], "no worker");
        assert_eq!(result["albums"][0]["descriptors"], json!(["Choir"]));
    }

    #[test]
    fn limits_how_much_is_described_at_once() {
        assert!(check_ids(&[], &[]).is_err());
        assert!(check_ids(&vec!["a".to_owned(); MAX_ALBUMS + 1], &[]).is_err());
        assert!(check_ids(&[], &vec!["1".to_owned(); MAX_TRACKS]).is_ok());
    }

    #[test]
    fn narrows_matches_with_a_search_and_keeps_their_order() {
        let (_dir, c) = connection();
        let search = parse_filter("sound:choir").unwrap().unwrap();
        let mut ids = ["6", "4", "2", "7", "1"].map(str::to_owned).to_vec();
        keep_matching(&c, &mut ids, &search, false).unwrap();
        assert_eq!(ids, ["6", "2", "1"]);
        let search = parse_filter("sound:synth").unwrap().unwrap();
        let mut ids = ["C", "B", "A"].map(str::to_owned).to_vec();
        keep_matching(&c, &mut ids, &search, true).unwrap();
        assert_eq!(ids, ["B"]);
        assert!(parse_filter("cr:0 sort:year").is_err());
        assert!(parse_filter("   ").unwrap().is_none());
    }

    /// Runs against the real library and Aurora state, with Aurora's own
    /// descriptor cache in `AURORA_PROOF_SOUND_CACHE`:
    /// `cargo test --lib sound_tools::tests::real_library -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs Music Library's data, Aurora's state and the CLAP worker"]
    fn real_library_proof() {
        let state = crate::mcp::app_data_dir().unwrap();
        let store = StateStore::open_existing(state.join("aurora-state.sqlite3")).unwrap();
        let page = crate::explorer::load_album_page(
            serde_json::from_value(
                json!({ "pageSize": 6, "search": "aartist:vangelis AND year:1980..1989" }),
            )
            .unwrap(),
            &store,
            true,
        )
        .unwrap();
        let ids: Vec<String> = page.items.iter().map(|album| album.id.clone()).collect();
        let started = std::time::Instant::now();
        let described = describe(&ids, &[], Some("orchestral film score")).unwrap();
        println!(
            "describe {:?}\n{}",
            started.elapsed(),
            serde_json::to_string_pretty(&described).unwrap()
        );
        let started = std::time::Instant::now();
        let like = like(
            LikeRequest {
                track_id: None,
                album_id: ids.first().cloned(),
                albums: None,
                limit: 10,
                query: Some("cr:0".to_owned()),
                same_genre: false,
                minimum_rating: None,
            },
            &store,
        )
        .unwrap();
        println!(
            "like {:?}\n{}",
            started.elapsed(),
            serde_json::to_string_pretty(&like).unwrap()
        );
    }
}
