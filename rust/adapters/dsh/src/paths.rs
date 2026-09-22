use std::{
    collections::{BTreeMap, HashSet},
    env,
    path::PathBuf,
};

use crate::Result;
use ccusage_adapter_common::collect_files_with_extension;

pub(crate) const DSH_HOME_ENV: &str = "DSH_HOME";

fn roots() -> Vec<PathBuf> {
    let env_candidates = env::var(DSH_HOME_ENV).ok().map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .collect()
    });
    let candidates = env_candidates
        .filter(|paths: &Vec<PathBuf>| !paths.is_empty())
        .unwrap_or_else(|| {
            crate::home::home_dir()
                .map(|home| vec![home.join(".dsh")])
                .unwrap_or_default()
        });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|path| path.is_dir())
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

pub(super) fn discover_session_files() -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for root in roots() {
        let sessions = root.join("sessions");
        if !sessions.is_dir() {
            continue;
        }
        let mut compressed = Vec::new();
        collect_files_with_extension(&sessions, "zstd", &mut compressed);
        files.extend(compressed);

        let mut raw = Vec::new();
        collect_files_with_extension(&sessions, "jsonl", &mut raw);
        files.extend(raw);
    }
    // DSH retains old format generations beside the writable current log.
    // Pick one generation per session, irrespective of archive modification time.
    let mut current = BTreeMap::<PathBuf, ((u64, bool), PathBuf)>::new();
    for path in files {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let compressed = name.ends_with(".zstd");
        let raw = name.strip_suffix(".zstd").unwrap_or(name);
        let version = if raw == "session.jsonl" {
            0
        } else if let Some(version) = raw
            .strip_prefix("session.v")
            .and_then(|name| name.strip_suffix(".jsonl"))
        {
            let Ok(version) = version.parse::<u64>() else {
                continue;
            };
            version
        } else {
            continue;
        };
        let Some(parent) = path.parent() else {
            continue;
        };
        let rank = (version, compressed);
        if current
            .get(parent)
            .is_none_or(|(previous, _)| rank > *previous)
        {
            current.insert(parent.to_path_buf(), (rank, path));
        }
    }
    Ok(current.into_values().map(|(_, path)| path).collect())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use ccusage_test_support::{EnvVarsGuard, fs_fixture};

    use super::*;

    #[test]
    fn discovers_raw_and_compressed_session_logs_only() {
        let fixture = fs_fixture!({
            "sessions/project/raw/session.jsonl": "{}\n",
            "sessions/project/compressed/session.jsonl.zstd": "{}\n",
            "sessions/project/other/events.jsonl": "{}\n",
            "sessions/project/other/session.jsonl.bak": "{}\n",
        });
        let _guard = EnvVarsGuard::set_many([(
            DSH_HOME_ENV,
            Some(OsString::from(fixture.root().as_os_str())),
        )]);

        let files = discover_session_files().unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|path| path.ends_with("session.jsonl")));
        assert!(
            files
                .iter()
                .any(|path| path.ends_with("session.jsonl.zstd"))
        );
    }

    #[test]
    fn prefers_latest_generation_without_counting_archived_logs() {
        let fixture = fs_fixture!({
            "sessions/project/one/session.jsonl.zstd": "{}\n",
            "sessions/project/one/session.v2.jsonl.zstd": "{}\n",
            "sessions/project/one/session.v3.jsonl.zstd": "{}\n",
            "sessions/project/two/session.v3.jsonl": "{}\n",
            "sessions/project/two/session.v9.jsonl.bak": "{}\n",
        });
        let _guard = EnvVarsGuard::set_many([(
            DSH_HOME_ENV,
            Some(OsString::from(fixture.root().as_os_str())),
        )]);
        let files = discover_session_files().unwrap();
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .any(|path| path.ends_with("session.v3.jsonl.zstd"))
        );
        assert!(files.iter().any(|path| path.ends_with("session.v3.jsonl")));
    }

    #[test]
    fn blank_home_falls_back_to_default_home() {
        let fixture = fs_fixture!({
            ".dsh/sessions/project/session/session.jsonl": "{}\n",
        });
        let home = OsString::from(fixture.root().as_os_str());
        let _guard = EnvVarsGuard::set_many([
            (DSH_HOME_ENV, Some(OsString::from("  "))),
            ("HOME", Some(home.clone())),
            ("USERPROFILE", Some(home)),
        ]);

        assert_eq!(discover_session_files().unwrap().len(), 1);
    }
}
