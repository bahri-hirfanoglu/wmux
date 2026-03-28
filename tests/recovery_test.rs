use std::fs;
use std::path::PathBuf;

use wmux::daemon::recovery::{self, PersistedSession, PersistedState};

/// Helper to create a unique temp directory for each test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wmux_test_{}_{}", std::process::id(), name));
    // Clean up any leftover from previous runs
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Clean up a test directory.
fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

fn sample_state() -> PersistedState {
    use wmux::daemon::recovery::PersistedPane;
    PersistedState {
        version: 1,
        sessions: vec![
            PersistedSession {
                id: "1".to_string(),
                name: Some("main".to_string()),
                pid: 1234,
                created_at: "2026-03-28T12:00:00Z".to_string(),
                shell: "powershell.exe".to_string(),
                cols: 120,
                rows: 30,
                panes: vec![PersistedPane {
                    id: 0,
                    pid: 1234,
                    shell: "powershell.exe".to_string(),
                    cols: 120,
                    rows: 30,
                }],
            },
            PersistedSession {
                id: "2".to_string(),
                name: None,
                pid: 5678,
                created_at: "2026-03-28T12:01:00Z".to_string(),
                shell: "cmd.exe".to_string(),
                cols: 80,
                rows: 24,
                panes: vec![PersistedPane {
                    id: 0,
                    pid: 5678,
                    shell: "cmd.exe".to_string(),
                    cols: 80,
                    rows: 24,
                }],
            },
        ],
        next_id: 3,
        saved_at: "2026-03-28T12:01:00Z".to_string(),
    }
}

#[test]
fn test_serialize_roundtrip() {
    let state = sample_state();
    let json = serde_json::to_string_pretty(&state).unwrap();
    let deserialized: PersistedState = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.version, 1);
    assert_eq!(deserialized.sessions.len(), 2);
    assert_eq!(deserialized.sessions[0].id, "1");
    assert_eq!(deserialized.sessions[0].name, Some("main".to_string()));
    assert_eq!(deserialized.sessions[0].pid, 1234);
    assert_eq!(deserialized.sessions[1].id, "2");
    assert_eq!(deserialized.sessions[1].name, None);
    assert_eq!(deserialized.sessions[1].pid, 5678);
    assert_eq!(deserialized.next_id, 3);
}

#[test]
fn test_save_and_load_state() {
    let dir = test_dir("save_and_load");
    let state = sample_state();

    recovery::save_state_to(&state, &dir).unwrap();

    // Verify file exists
    assert!(dir.join("state.json").exists());

    // Load it back
    let loaded = recovery::load_state_from(&dir).unwrap();
    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.sessions.len(), 2);
    assert_eq!(loaded.next_id, 3);
    assert_eq!(loaded.sessions[0].id, "1");
    assert_eq!(loaded.sessions[1].shell, "cmd.exe");

    cleanup(&dir);
}

#[test]
fn test_load_state_missing_file() {
    let dir = test_dir("missing_file");
    // Don't create any state file — load should return empty state
    let loaded = recovery::load_state_from(&dir).unwrap();
    assert_eq!(loaded.version, 1);
    assert!(loaded.sessions.is_empty());
    assert_eq!(loaded.next_id, 1);

    cleanup(&dir);
}

#[test]
fn test_load_state_corrupted_file() {
    let dir = test_dir("corrupted");
    let state_path = dir.join("state.json");
    fs::write(&state_path, "this is not valid json!!!").unwrap();

    let loaded = recovery::load_state_from(&dir).unwrap();
    assert_eq!(loaded.version, 1);
    assert!(loaded.sessions.is_empty());

    // Corrupted file should be backed up
    assert!(dir.join("state.json.bak").exists());

    cleanup(&dir);
}

#[test]
fn test_save_state_atomic_write() {
    let dir = test_dir("atomic");
    let state = sample_state();

    recovery::save_state_to(&state, &dir).unwrap();

    // No temp file should remain
    assert!(!dir.join("state.json.tmp").exists());
    // Real file should exist
    assert!(dir.join("state.json").exists());

    // Content should be valid JSON
    let content = fs::read_to_string(dir.join("state.json")).unwrap();
    let parsed: PersistedState = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.sessions.len(), 2);

    cleanup(&dir);
}
