use super::*;

#[test]
fn m3u8_keeps_utf8_repeats_and_relative_order_and_rejects_streams_and_limits() {
    let base = std::env::temp_dir();
    let paths=m3u_paths("\u{feff}#EXTM3U\r\n#EXTINF:60,Årtist – title\r\nfolder/../first.mp3\nsecond.mp3\nfirst.mp3\n",&base).unwrap();
    assert_eq!(
        paths,
        vec![
            base.join("first.mp3"),
            base.join("second.mp3"),
            base.join("first.mp3")
        ]
    );
    assert!(m3u_paths("https://example.test/stream.mp3", &base).is_err());
    assert!(m3u_paths("#EXTM3U\n", &base).is_err());
    assert!(m3u_paths(&"song.mp3\n".repeat(1001), &base).is_err());
    let file = url::Url::from_file_path(base.join("å song.mp3")).unwrap();
    assert_eq!(
        m3u_paths(file.as_str(), &base).unwrap(),
        vec![base.join("å song.mp3")]
    );
}

#[test]
fn import_matches_exact_catalog_paths_and_rejects_missing_or_ambiguous_entries() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE tracks(id INTEGER,file_path TEXT,filename TEXT)")
        .unwrap();
    let root = std::env::temp_dir();
    let directory = root.to_string_lossy();
    conn.execute(
        "INSERT INTO tracks VALUES(7,?1,'å song.mp3')",
        [directory.as_ref()],
    )
    .unwrap();
    let paths = vec![root.join("å song.mp3"), root.join("å song.mp3")];
    let tracks = resolve_import(&conn, &paths).unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0]["id"], 7);
    assert!(
        resolve_import(&conn, &[root.join("other.mp3")])
            .unwrap_err()
            .contains("No playlist was saved")
    );
    conn.execute(
        "INSERT INTO tracks VALUES(8,?1,'å song.mp3')",
        [directory.as_ref()],
    )
    .unwrap();
    assert!(resolve_import(&conn, &paths).is_err());
}

#[test]
fn export_preserves_unavailable_paths_and_repeats_and_roundtrips_in_order() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE saved_playlists(id INTEGER,playlist_json TEXT,updated_at TEXT)",
    )
    .unwrap();
    let base = std::env::temp_dir();
    let first = serde_json::json!({"filePath":base,"filename":"å song.mp3","title":"First\n#bad","displayArtist":"Årtist","seconds":60});
    let second = serde_json::json!({"filePath":base,"filename":"missing.mp3","title":"Missing","seconds":90});
    conn.execute(
        "INSERT INTO saved_playlists VALUES(1,?1,'rev')",
        [serde_json::json!({"tracks":[first,second,first]}).to_string()],
    )
    .unwrap();
    let text = export_on(&conn, 1, "rev").unwrap();
    assert!(text.contains("Årtist - First #bad"));
    assert_eq!(
        m3u_paths(&text, &base).unwrap(),
        vec![
            base.join("å song.mp3"),
            base.join("missing.mp3"),
            base.join("å song.mp3")
        ]
    );
    assert!(export_on(&conn, 1, "old").is_err());
}

#[test]
fn import_fingerprint_detects_changed_content_and_utf8_and_size_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("songs.m3u8");
    std::fs::write(&path, "a.mp3").unwrap();
    let (_, first) = read_import(&path).unwrap();
    std::fs::write(&path, "b.mp3").unwrap();
    assert_ne!(read_import(&path).unwrap().1, first);
    std::fs::write(&path, [255]).unwrap();
    assert!(read_import(&path).is_err());
    std::fs::write(&path, vec![b'a'; MAX_IMPORT_BYTES as usize + 1]).unwrap();
    assert!(read_import(&path).is_err());
}
