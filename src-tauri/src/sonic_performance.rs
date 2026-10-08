//! Opt-in native benchmark; it writes only the disposable state snapshot.
#[test]
#[ignore = "requires MUSIC_SONIC_BENCH_DIR containing SQLite snapshots and seeds.json"]
fn sonic_performance_proof() {
    use std::{collections::HashMap, time::Instant};
    let dir = std::path::PathBuf::from(std::env::var_os("MUSIC_SONIC_BENCH_DIR").unwrap());
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("seeds.json")).unwrap()).unwrap();
    let stops: Vec<String> = serde_json::from_value(config["stops"].clone()).unwrap();
    let indexed = std::env::var("MUSIC_SONIC_BENCH_MODE").as_deref() != Ok("exact");
    let state_dir = tempfile::tempdir().unwrap();
    let store =
        crate::state_store::StateStore::new(state_dir.path().join("state.sqlite3")).unwrap();
    let mut records = Vec::new();
    for iteration in 0..5 {
        for task in ["tracks", "radio", "albums", "journey"] {
            let start = Instant::now();
            let c = crate::catalog::open_catalog(&dir.join("music-library.sqlite3")).unwrap();
            let value = if task == "tracks" || task == "radio" {
                serde_json::to_value(
                    crate::sonic::run_indexed(
                        &c,
                        &crate::sonic::SonicRequest {
                            seed_key: stops[0].clone(),
                            seed_album_id: None,
                            minimum_coverage: 50,
                            exclude_keys: vec![],
                            limit: 50,
                            minimum_rating: None,
                            same_genre: false,
                            radio: task == "radio",
                        },
                        Some(&store),
                        indexed,
                    )
                    .unwrap(),
                )
                .unwrap()
            } else if task == "albums" {
                serde_json::to_value(
                    crate::sonic_albums::query_indexed(
                        &c,
                        true,
                        &crate::sonic_albums::SonicAlbumRequest {
                            album_id: config["album"].as_str().unwrap().into(),
                            limit: 20,
                            minimum_coverage: 50,
                        },
                        &HashMap::new(),
                        indexed,
                    )
                    .unwrap(),
                )
                .unwrap()
            } else {
                serde_json::to_value(
                    crate::sonic_journey::query_indexed(
                        &c,
                        true,
                        &crate::sonic_journey::JourneyRequest {
                            stop_keys: stops.clone(),
                            connecting_tracks: 3,
                            minimum_rating: None,
                            same_genre: false,
                        },
                        &HashMap::new(),
                        indexed,
                    )
                    .unwrap(),
                )
                .unwrap()
            };
            if task == "journey" {
                assert_eq!(value["complete"], true);
                assert_eq!(
                    value["tracks"].as_array().unwrap().len(),
                    stops.len() + (stops.len() - 1) * 3
                );
            } else {
                assert_eq!(value["seedReady"], true);
                assert!(
                    !value[if task == "albums" { "albums" } else { "tracks" }]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
            }
            let millis = start.elapsed().as_secs_f64() * 1000.;
            println!("{task} iteration={iteration} ms={millis:.2}");
            records.push(
                serde_json::json!({"task":task,"iteration":iteration,"ms":millis,"result":value}),
            );
        }
    }
    std::fs::write(
        std::env::var_os("MUSIC_SONIC_BENCH_OUTPUT").unwrap(),
        serde_json::to_vec_pretty(&serde_json::json!({"indexed":indexed,"indexStats":crate::sonic_index::proof_stats(),"records":records})).unwrap(),
    )
    .unwrap();
}
