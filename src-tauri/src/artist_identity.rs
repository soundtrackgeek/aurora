//! Read Music Library's artist identities without mutating its catalog.
use crate::catalog::artist_key_sql;
use rusqlite::{Connection, OptionalExtension};

fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name=?2)",
        [table, column],
        |row| row.get(0),
    )
    .unwrap_or(false)
}
pub(crate) fn published(conn: &Connection) -> bool {
    has_column(conn, "albums", "artist_identity_key")
}
pub(crate) fn album_reference(album_id: &str) -> Result<String, String> {
    let connection = crate::catalog::open_catalog(&crate::catalog::default_catalog_path()?)?;
    let reference = reference_sql(&connection, "album_artist_display");
    connection
        .query_row(
            &format!("SELECT {reference} FROM albums WHERE id=?1"),
            [album_id],
            |row| row.get(0),
        )
        .map_err(|_| "This album is no longer available in the catalog.".into())
}
pub(crate) fn key_sql(conn: &Connection, field: &str) -> String {
    if published(conn) {
        format!(
            "COALESCE(NULLIF({}, ''), {})",
            field.replace("album_artist_display", "artist_identity_key"),
            artist_key_sql(field)
        )
    } else {
        artist_key_sql(field)
    }
}
pub(crate) fn reference_sql(conn: &Connection, field: &str) -> String {
    if published(conn) {
        key_sql(conn, field)
    } else {
        format!("COALESCE(NULLIF(TRIM({field}),''),'Unknown Artist')")
    }
}
pub(crate) fn description_sql(conn: &Connection, key: &str) -> String {
    if has_column(conn, "artist_identities", "disambiguation") {
        format!("(SELECT disambiguation FROM artist_identities WHERE id={key})")
    } else {
        "NULL".into()
    }
}
pub(crate) fn resolve(
    conn: &Connection,
    reference: &str,
) -> Result<(String, String, bool), String> {
    let (name, normalized) = crate::musicbrainz::validate_artist(reference)?;
    if !published(conn) {
        return Ok((normalized, name, false));
    }
    if let Some((id, name)) = conn
        .query_row(
            "SELECT id,display_artist FROM artist_identities WHERE id=?1",
            [reference],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        let ambiguous = conn.query_row("SELECT COUNT(*)>1 FROM artist_identities WHERE name_key=(SELECT name_key FROM artist_identities WHERE id=?1)",[&id],|row|row.get(0)).map_err(|error|error.to_string())?;
        return Ok((id, name, ambiguous));
    }
    let key = key_sql(conn, "a.album_artist_display");
    let exact=conn.query_row(&format!("SELECT {key}, MIN(a.album_artist_display) FROM albums a WHERE {key}=?1 HAVING COUNT(*)>0"),[reference],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).optional().map_err(|error|error.to_string())?;
    let (id, name) = if let Some(value) = exact {
        value
    } else {
        let mut stmt=conn.prepare(&format!("SELECT {key},MIN(a.album_artist_display) FROM albums a WHERE {}=?1 GROUP BY {key} LIMIT 2",artist_key_sql("a.album_artist_display"))).map_err(|error|error.to_string())?;
        let matches = stmt
            .query_map([normalized], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        if matches.len() > 1 {
            return Err(
                "Several artists share this name. Choose the separate profile in Artists.".into(),
            );
        }
        matches
            .into_iter()
            .next()
            .unwrap_or((normalized_name(&name), name))
    };
    let ambiguous=conn.query_row("SELECT COUNT(*)>1 FROM artist_identities WHERE name_key=(SELECT name_key FROM artist_identities WHERE id=?1)",[&id],|row|row.get::<_,bool>(0)).unwrap_or(false);
    Ok((id, name, ambiguous))
}
fn normalized_name(name: &str) -> String {
    crate::musicbrainz::validate_artist(name)
        .map(|(_, key)| key)
        .unwrap_or_default()
}

pub(crate) fn project_sql(conn: &Connection, sql: &str) -> String {
    if !published(conn) {
        return sql.into();
    }
    let mut sql = sql.to_owned();
    for alias in ["t", "a"] {
        let field = format!("{alias}.album_artist_display");
        let effective = key_sql(conn, &field);
        let marker = format!("__aurora_artist_identity_{alias}__");
        sql = sql
            .replace(&effective, &marker)
            .replace(&artist_key_sql(&field), &effective)
            .replace(&marker, &effective);
    }
    for alias in ["t", "p"] {
        sql = sql.replace(&format!("l.artist_key = lower(trim({alias}.album_artist_display))"),
            &format!("l.artist_key = (SELECT linked.artist_identity_key FROM tracks linked WHERE linked.id={alias}.id)"));
    }
    sql
}

pub(crate) fn attach_tracks(
    conn: &Connection,
    tracks: &mut [crate::catalog::TrackSummary],
) -> Result<(), String> {
    if !published(conn) || tracks.is_empty() {
        return Ok(());
    }
    let ids = tracks
        .iter()
        .filter_map(|track| track.album_id.as_deref())
        .collect::<std::collections::HashSet<_>>();
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let key = key_sql(conn, "a.album_artist_display");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT a.id,{key} FROM albums a WHERE a.id IN ({placeholders})"
        ))
        .map_err(|error| error.to_string())?;
    let values = stmt
        .query_map(rusqlite::params_from_iter(ids), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()
        .map_err(|error| error.to_string())?;
    for track in tracks {
        track.artist_id = track
            .album_id
            .as_ref()
            .and_then(|id| values.get(id).cloned());
    }
    Ok(())
}
