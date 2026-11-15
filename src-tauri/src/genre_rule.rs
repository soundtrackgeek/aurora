//! Checks an album's genre tags as written, for a one-genre-per-album rule.
//!
//! Music Library stores the raw tag in `tracks.genre` and joins several genres
//! with `;` (`Electro; Dance`). `canonical_genre` keeps only the first, so
//! these checks read the raw tag instead.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagCount {
    pub(crate) tag: String,
    pub(crate) tracks: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreCount {
    pub(crate) genre: String,
    pub(crate) tracks: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumGenreTags {
    pub(crate) album_id: String,
    pub(crate) artist: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) year: Option<i64>,
    /// Each distinct tag value with its track count, most common first.
    pub(crate) tags: Vec<TagCount>,
    /// Each genre after splitting tags on `;`, with the tracks that carry it.
    pub(crate) genres: Vec<GenreCount>,
    /// The genre listed first on most tracks: the usual choice for one genre.
    pub(crate) primary_genre: Option<String>,
    pub(crate) tracks_without_genre: i64,
}

/// Splits a tag into its genres, dropping empty parts.
pub(crate) fn split_genres(tag: &str) -> Vec<String> {
    tag.split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn load_album(
    connection: &Connection,
    album_id: &str,
) -> Result<Option<AlbumGenreTags>, String> {
    let album = connection
        .query_row(
            "SELECT album_artist_display, album, year FROM albums WHERE id = ?1",
            params![album_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("Could not read the album: {error}"))?;
    let Some((artist, title, year)) = album else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare_cached(
            "SELECT TRIM(COALESCE(genre, '')), COUNT(*) FROM tracks WHERE album_id = ?1 \
             GROUP BY 1 ORDER BY 2 DESC, 1",
        )
        .map_err(|error| format!("Could not prepare the album's genre tags: {error}"))?;
    let rows = statement
        .query_map(params![album_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("Could not read the album's genre tags: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read the album's genre tags: {error}"))?;
    Ok(Some(summarize(album_id, artist, title, year, rows)))
}

fn summarize(
    album_id: &str,
    artist: Option<String>,
    title: Option<String>,
    year: Option<i64>,
    rows: Vec<(String, i64)>,
) -> AlbumGenreTags {
    let mut tags = Vec::new();
    let mut genres: Vec<GenreCount> = Vec::new();
    let mut firsts: Vec<GenreCount> = Vec::new();
    let mut tracks_without_genre = 0;
    for (tag, tracks) in rows {
        let parts = split_genres(&tag);
        if parts.is_empty() {
            tracks_without_genre += tracks;
            continue;
        }
        for (index, genre) in parts.iter().enumerate() {
            add(&mut genres, genre, tracks);
            if index == 0 {
                add(&mut firsts, genre, tracks);
            }
        }
        tags.push(TagCount { tag, tracks });
    }
    genres.sort_by_key(|count| std::cmp::Reverse(count.tracks));
    firsts.sort_by_key(|count| std::cmp::Reverse(count.tracks));
    AlbumGenreTags {
        album_id: album_id.to_owned(),
        artist,
        title,
        year,
        tags,
        genres,
        primary_genre: firsts.into_iter().next().map(|first| first.genre),
        tracks_without_genre,
    }
}

fn add(counts: &mut Vec<GenreCount>, genre: &str, tracks: i64) {
    match counts
        .iter_mut()
        .find(|count| count.genre.eq_ignore_ascii_case(genre))
    {
        Some(count) => count.tracks += tracks,
        None => counts.push(GenreCount {
            genre: genre.to_owned(),
            tracks,
        }),
    }
}

/// One page of albums with more than one genre: a tag that holds several
/// genres, or tracks whose tags differ.
pub(crate) fn albums_with_multiple_genres(
    connection: &Connection,
    genre: Option<&str>,
    tag_contains: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<AlbumGenreTags>, i64), String> {
    let mut statement = connection
        .prepare(
            r#"
            WITH per_album AS (
              SELECT album_id,
                     COUNT(DISTINCT lower(TRIM(genre))) AS tags,
                     MAX(1 + LENGTH(genre) - LENGTH(REPLACE(genre, ';', ''))) AS widest
              FROM tracks
              WHERE album_id IS NOT NULL AND NULLIF(TRIM(genre), '') IS NOT NULL
              GROUP BY album_id
            )
            SELECT a.id, COUNT(*) OVER () AS total
            FROM per_album p JOIN albums a ON a.id = p.album_id
            WHERE (p.tags > 1 OR p.widest > 1)
              AND (?1 IS NULL OR TRIM(a.canonical_genre) = ?1 COLLATE NOCASE)
              AND (?2 IS NULL OR EXISTS (
                SELECT 1 FROM tracks t
                WHERE t.album_id = a.id AND instr(lower(t.genre), lower(?2)) > 0))
            ORDER BY a.album_artist_display COLLATE NOCASE, a.album COLLATE NOCASE, a.id
            LIMIT ?3 OFFSET ?4
            "#,
        )
        .map_err(|error| format!("Could not prepare the genre check: {error}"))?;
    let rows = statement
        .query_map(params![genre, tag_contains, limit, offset], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("Could not run the genre check: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read the genre check: {error}"))?;
    let total = rows.first().map_or(0, |(_, total)| *total);
    let mut albums = Vec::with_capacity(rows.len());
    for (album_id, _) in rows {
        if let Some(album) = load_album(connection, &album_id)? {
            albums.push(album);
        }
    }
    Ok((albums, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE albums (id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT,
                   year INTEGER, canonical_genre TEXT);
                 CREATE TABLE tracks (id INTEGER PRIMARY KEY, album_id TEXT, genre TEXT);
                 INSERT INTO albums VALUES
                   ('one', 'Clean', 'A', 1990, 'Rock'),
                   ('multi', 'Two Tags', 'B', 1985, 'Electro'),
                   ('mixed', 'Mixed', 'C', 2001, 'Electro');
                 INSERT INTO tracks (album_id, genre) VALUES
                   ('one', 'Rock'), ('one', 'Rock'),
                   ('multi', 'Electro; Dance'), ('multi', 'Electro; Dance'), ('multi', 'Electro'),
                   ('mixed', 'Pop; Electro'), ('mixed', 'Electro'), ('mixed', 'Electro'), ('mixed', '');",
            )
            .unwrap();
        connection
    }

    #[test]
    fn splits_tags_on_semicolons() {
        assert_eq!(split_genres(" Electro; Dance ;; "), ["Electro", "Dance"]);
        assert_eq!(split_genres("Films/Games"), ["Films/Games"]);
        assert!(split_genres(" ").is_empty());
    }

    #[test]
    fn summarizes_tags_genres_and_the_primary_genre() {
        let album = load_album(&catalog(), "mixed").unwrap().unwrap();
        assert_eq!(
            album.tags[0],
            TagCount {
                tag: "Electro".into(),
                tracks: 2
            }
        );
        assert_eq!(
            album.genres,
            [
                GenreCount {
                    genre: "Electro".into(),
                    tracks: 3
                },
                GenreCount {
                    genre: "Pop".into(),
                    tracks: 1
                },
            ]
        );
        assert_eq!(album.primary_genre.as_deref(), Some("Electro"));
        assert_eq!(album.tracks_without_genre, 1);
        assert!(load_album(&catalog(), "missing").unwrap().is_none());
    }

    #[test]
    fn finds_albums_that_break_the_one_genre_rule() {
        let connection = catalog();
        let (albums, total) = albums_with_multiple_genres(&connection, None, None, 10, 0).unwrap();
        assert_eq!(total, 2);
        assert_eq!(
            albums
                .iter()
                .map(|album| album.album_id.as_str())
                .collect::<Vec<_>>(),
            ["multi", "mixed"]
        );
        let (albums, _) =
            albums_with_multiple_genres(&connection, None, Some("dance"), 10, 0).unwrap();
        assert_eq!(albums.len(), 1);
        let (albums, total) =
            albums_with_multiple_genres(&connection, Some("rock"), None, 10, 0).unwrap();
        assert!(albums.is_empty() && total == 0);
    }
}
