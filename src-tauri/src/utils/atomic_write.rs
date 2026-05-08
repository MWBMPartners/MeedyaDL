// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Atomic JSON file write helper.
// =================================
//
// Codebase audit (#716 finding #8) identified 5+ services that
// independently implement the same "serialize → write to .tmp →
// rename" pattern: settings.json, queue.json, history.json, crash
// reports, manifest.meedyadl, engine config. Each open-codes:
//
//   1. `serde_json::to_string_pretty(&data).map_err(...)`
//   2. `path.with_extension("json.tmp")`
//   3. `std::fs::write(&tmp, &json).map_err(...)`
//   4. `std::fs::rename(&tmp, &path).map_err(...)`
//
// Centralising in a single helper means future durability changes
// (e.g. fsync after write, retry on transient `EBUSY`, lock-file
// support) live in one place and apply uniformly.
//
// Migration is opt-in per callsite (#716 follow-ups). Existing
// patterns keep working until they're individually refactored.

use serde::Serialize;
use std::path::Path;

/// Atomically writes a JSON-serialised value to `path`.
///
/// **Atomicity guarantee.** Writes to `{path}.tmp` first, then renames
/// to `path`. Rename is atomic on all major filesystems (APFS, ext4,
/// NTFS) so a process crash or power loss leaves either the old file
/// intact OR the new file fully written — never a half-written
/// corruption (#230).
///
/// **Pretty output.** Uses `serde_json::to_string_pretty` so the file
/// is human-readable for support / manual diff. Matches every
/// existing call site's behaviour.
///
/// **Error messages.** Returns a `String` error with the same shape
/// existing call sites produce (`"Failed to serialize {context}: {e}"`,
/// `"Failed to write {context}: {e}"`, `"Failed to rename {context}: {e}"`)
/// so call-site error-handling can switch to this helper without
/// regression in user-facing messages.
///
/// `context` is a short label used in the error messages (e.g.
/// `"settings"`, `"queue"`, `"manifest"`) — appears literally in the
/// returned errors.
pub fn atomic_write_json<T: Serialize>(
    path: &Path,
    data: &T,
    context: &str,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize {context}: {e}"))?;
    let tmp = path.with_extension(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("{e}.tmp"))
            .unwrap_or_else(|| "tmp".to_string()),
    );
    std::fs::write(&tmp, &json)
        .map_err(|e| format!("Failed to write {context}: {e}"))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("Failed to rename {context}: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn writes_pretty_json_then_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data.json");
        let sample = Sample {
            name: "MeedyaDL".to_string(),
            count: 42,
        };

        atomic_write_json(&path, &sample, "sample").unwrap();

        // File exists and parses back to the same value.
        let raw = std::fs::read_to_string(&path).unwrap();
        let parsed: Sample = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, sample);

        // Pretty output check — should contain a newline (compact JSON
        // would be a single line).
        assert!(raw.contains('\n'), "atomic_write_json should pretty-print");
    }

    #[test]
    fn overwrites_existing_file_atomically() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data.json");

        atomic_write_json(
            &path,
            &Sample {
                name: "first".to_string(),
                count: 1,
            },
            "sample",
        )
        .unwrap();

        // Second write — should atomically replace the first content.
        atomic_write_json(
            &path,
            &Sample {
                name: "second".to_string(),
                count: 2,
            },
            "sample",
        )
        .unwrap();

        let parsed: Sample = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.name, "second");
        assert_eq!(parsed.count, 2);

        // No leftover .tmp file.
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists(), "temp file should have been renamed");
    }

    #[test]
    fn no_extension_path_still_works() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("dotless");
        atomic_write_json(
            &path,
            &Sample {
                name: "no-ext".to_string(),
                count: 0,
            },
            "sample",
        )
        .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn returns_error_when_target_dir_missing() {
        let path = std::path::PathBuf::from("/definitely/does/not/exist/data.json");
        let result = atomic_write_json(
            &path,
            &Sample {
                name: "x".to_string(),
                count: 0,
            },
            "sample",
        );
        assert!(result.is_err());
        assert!(
            result.as_ref().unwrap_err().contains("sample"),
            "error should mention context label"
        );
    }
}
