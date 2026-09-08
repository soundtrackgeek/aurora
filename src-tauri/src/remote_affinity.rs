//! A narrow local-replica update after Tonehavn confirms the authoritative edit.
use crate::{
    catalog::ResolvedTrack,
    tag_model::{LoveState, TagValues},
};
use rusqlite::{Connection, OpenFlags, params};
use std::path::Path;

pub(crate) fn validate_change(expected: &TagValues, desired: &TagValues) -> Result<(), String> {
    expected.validate()?;
    desired.validate()?;
    if desired.release_year != expected.release_year
        || desired.love_state == LoveState::Banned
        || expected.love_state == LoveState::Banned
    {
        return Err("Remote editing currently supports whole-star ratings and Love only. Change other tags on the PC.".into());
    }
    if [expected.rating, desired.rating]
        .into_iter()
        .flatten()
        .any(|r| r.fract() != 0.0)
    {
        return Err("Remote song ratings must be whole stars from 1 to 5, or unrated.".into());
    }
    Ok(())
}

pub(crate) fn update_local_catalog(
    path: &Path,
    resolved: &ResolvedTrack,
    values: &TagValues,
) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|_| "Could not resolve the local catalog path.")?;
    if !cfg!(target_os = "macos") || path.starts_with("/Volumes") {
        return Err("Local catalog mirroring requires a catalog on this Mac.".into());
    }
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| "Could not open the Mac catalog for its confirmed update.")?;
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    apply(
        &mut db,
        &resolved.summary.id,
        &resolved.summary.directory,
        &resolved.summary.filename,
        &resolved.catalog_values,
        values,
    )
}

fn apply(
    db: &mut Connection,
    id: &str,
    directory: &str,
    filename: &str,
    expected: &TagValues,
    values: &TagValues,
) -> Result<(), String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let (album_id, rating, love): (String, Option<f64>, Option<String>) = tx.query_row(
        "SELECT album_id, COALESCE(normalized_rating / 20.0, CAST(NULLIF(rating_raw, '') AS REAL)), love FROM tracks WHERE id=?1 AND file_path=?2 AND filename=?3 AND EXISTS(SELECT 1 FROM albums WHERE albums.id=tracks.album_id)",
        params![id, directory, filename], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).map_err(|_| "The Mac catalog identity changed. Refresh the catalog before mirroring this edit.")?;
    if rating != expected.rating || LoveState::from_catalog(love.as_deref()) != expected.love_state
    {
        return Err(
            "The Mac catalog changed during the edit. Its local values were preserved.".into(),
        );
    }
    let love = match values.love_state {
        LoveState::Loved => "L",
        LoveState::Neutral => "",
        LoveState::Banned => return Err("Remote Ban edits are unsupported.".into()),
    };
    tx.execute(
        "UPDATE tracks SET normalized_rating=?1, rating_raw=?2, love=?3 WHERE id=?4",
        params![
            values.rating.map(|v| (v * 20.0) as i64),
            values.rating.map(|v| format!("{v:.0}")).unwrap_or_default(),
            love,
            id
        ],
    )
    .map_err(|e| e.to_string())?;
    // Recompute only this album using Music Library's existing 0–100 formulas.
    tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS affinity_album_totals(album_id TEXT, total INTEGER, rated INTEGER, rating_sum REAL, loved INTEGER, seconds REAL, five_star_seconds REAL); DELETE FROM affinity_album_totals").map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO affinity_album_totals SELECT album_id, COUNT(*), COUNT(normalized_rating), COALESCE(SUM(normalized_rating),0), SUM(CASE WHEN love='L' THEN 1 ELSE 0 END), COALESCE(SUM(time_seconds),0), COALESCE(SUM(CASE WHEN normalized_rating=100 THEN time_seconds ELSE 0 END),0) FROM tracks WHERE album_id=?1 GROUP BY album_id", [&album_id]).map_err(|e|e.to_string())?;
    tx.execute("UPDATE albums SET rated_tracks=t.rated, loved_tracks=t.loved, rating_completeness=CASE WHEN t.total>0 THEN t.rated*1.0/t.total ELSE 0 END, tmoe_seconds=t.five_star_seconds, ae_ratio=CASE WHEN t.seconds>0 THEN t.five_star_seconds/t.seconds ELSE 0 END, calculated_album_rating=CASE WHEN t.total=t.rated AND t.total>0 THEN round(t.rating_sum/t.total) ELSE NULL END FROM affinity_album_totals t WHERE albums.id=t.album_id", []).map_err(|e|e.to_string())?;
    tx.execute("UPDATE albums SET effective_album_rating=COALESCE(album_rating, calculated_album_rating) WHERE id=?1", [&album_id]).map_err(|e|e.to_string())?;
    tx.execute("UPDATE albums SET album_score=CASE WHEN effective_album_rating IS NULL THEN NULL ELSE (effective_album_rating*0.5 + ae_ratio*100.0 + (tmoe_seconds/60.0)*0.3)/10.0 + loved_tracks*100.0 END WHERE id=?1", [&album_id]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn values(rating: Option<f64>, love_state: LoveState) -> TagValues {
        TagValues {
            rating,
            love_state,
            release_year: None,
        }
    }
    #[test]
    fn rejects_half_stars_and_non_affinity_changes() {
        let before = values(None, LoveState::Neutral);
        assert!(validate_change(&before, &values(Some(4.5), LoveState::Neutral)).is_err());
        assert!(validate_change(&before, &values(Some(4.0), LoveState::Loved)).is_ok());
        let mut other = before.clone();
        other.release_year = Some(2020);
        assert!(validate_change(&before, &other).is_err());
    }
    #[test]
    fn mirror_updates_album_math_and_rolls_back_stale_identity() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE tracks(id INTEGER, album_id TEXT, file_path TEXT, filename TEXT, normalized_rating INTEGER, rating_raw TEXT, love TEXT, time_seconds INTEGER); INSERT INTO tracks VALUES(1,'album','D:\\MUSIC','a.mp3',NULL,'','',120),(2,'album','D:\\MUSIC','b.mp3',100,'5','',180); CREATE TABLE albums(id TEXT, album_rating INTEGER, rated_tracks INTEGER, loved_tracks INTEGER, rating_completeness REAL, tmoe_seconds REAL, ae_ratio REAL, calculated_album_rating REAL, effective_album_rating REAL, album_score REAL); INSERT INTO albums(id) VALUES('album');").unwrap();
        let before = values(None, LoveState::Neutral);
        let after = values(Some(4.0), LoveState::Loved);
        apply(&mut db, "1", "D:\\MUSIC", "a.mp3", &before, &after).unwrap();
        let actual: (i64, i64, f64, f64) = db
            .query_row(
                "SELECT rated_tracks,loved_tracks,calculated_album_rating,album_score FROM albums",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(actual, (2, 1, 90.0, 110.59));
        assert!(
            apply(
                &mut db,
                "1",
                "D:\\MUSIC",
                "a.mp3",
                &before,
                &values(Some(1.0), LoveState::Neutral)
            )
            .is_err()
        );
        assert_eq!(
            db.query_row("SELECT normalized_rating FROM tracks WHERE id=1", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            80
        );
    }
}
