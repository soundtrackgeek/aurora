use super::*;
use crate::catalog::TrackSummary;
use crate::tag_model::LoveState;

const DEVICE: &str = "device-test-export";

fn track(directory: &str) -> TrackSummary {
    TrackSummary {
        id: "1".to_owned(),
        track_key: r"d:\music\kate bush\01.mp3".to_owned(),
        album_id: Some("hounds".to_owned()),
        title: "Running Up That Hill".to_owned(),
        artist: "Kate Bush".to_owned(),
        display_artist: None,
        album: "Hounds of Love".to_owned(),
        release_year: Some(1985),
        original_year: Some(1985),
        publisher: None,
        rating: None,
        loved: false,
        love_state: LoveState::Neutral,
        tag_sync_state: None,
        can_undo_tag_edit: false,
        duration_seconds: Some(300),
        genre: None,
        play_count: None,
        track_number: None,
        track_total: None,
        disc_number: None,
        disc_total: None,
        directory: directory.to_owned(),
        filename: "01.mp3".to_owned(),
        catalog_import_run_id: 1,
    }
}

#[test]
fn payload_matches_the_record_plays_contract() {
    let play = RegisteredPlay {
        device_id: DEVICE.to_owned(),
        started_at_ms: 1_790_000_000_999,
        title: "Running Up That Hill".to_owned(),
        artist: "Kate Bush".to_owned(),
        album: String::new(),
        directory: r"D:\MUSIC\Kate Bush\".to_owned(),
        filename: "01.mp3".to_owned(),
    };
    assert_eq!(
        payload(&[play]),
        json!({ "plays": [{
            "artist": "Kate Bush",
            "title": "Running Up That Hill",
            "album": null,
            "playedAt": 1_790_000_000,
            "filePath": r"D:\MUSIC\Kate Bush\01.mp3",
        }]})
    );
}

#[test]
fn exports_new_plays_once_and_resends_after_a_failure() {
    let directory = tempfile::tempdir().unwrap();
    let history = HistoryStore::for_tests(directory.path(), DEVICE);
    let song = track(r"D:\MUSIC\Kate Bush");
    history.record_test_session(&song, 1_000_000, true);
    history.record_test_session(&song, 2_000_000, false);
    let mut state = PlayExportState::default();
    let mut requests = Vec::new();

    let sent = export_pending(&history, &mut state, 5, |request| {
        requests.push(request);
        Ok(json!({ "inserted": 1, "duplicates": 0, "skipped": 0 }))
    })
    .unwrap();
    assert_eq!(sent, 1, "only registered plays are sent");
    assert_eq!(requests.len(), 1);
    assert_eq!(state.cursors[DEVICE], 1_000_000);

    // Nothing new: no bridge process is started.
    let sent = export_pending(&history, &mut state, 6, |_| panic!("no request expected")).unwrap();
    assert_eq!(sent, 0);

    // A new play is sent together with the overlap window; a failure keeps
    // the cursor so the next run sends it again.
    history.record_test_session(&song, 3_000_000, true);
    let error = export_pending(&history, &mut state, 7, |_| {
        Err("Unknown Aurora bridge operation \"recordPlays\"".to_owned())
    })
    .unwrap_err();
    assert!(error.contains("Update Music Library"));
    assert_eq!(state.cursors[DEVICE], 1_000_000);
    assert_eq!(state.retry_after_ms, Some(7 + RETRY_AFTER_ERROR_MS));

    let mut resent = Vec::new();
    export_pending(&history, &mut state, 8, |request| {
        resent.push(request);
        Ok(json!({}))
    })
    .unwrap();
    assert_eq!(resent[0]["plays"].as_array().unwrap().len(), 2);
    assert_eq!(state.cursors[DEVICE], 3_000_000);
    assert_eq!(state.sent_plays, 2);
    assert_eq!(state.last_error, None);
}

#[test]
fn state_round_trips_and_tolerates_a_missing_or_corrupt_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = state_path(directory.path());
    assert_eq!(load_state(&path), PlayExportState::default());
    let state = PlayExportState {
        cursors: HashMap::from([(DEVICE.to_owned(), 42)]),
        sent_plays: 3,
        last_exported_at_ms: Some(9),
        last_error: None,
        retry_after_ms: None,
    };
    save_state(&path, &state).unwrap();
    assert_eq!(load_state(&path), state);
    fs::write(&path, b"not json").unwrap();
    assert_eq!(load_state(&path), PlayExportState::default());
}
