use std::{collections::HashSet, env, path::PathBuf};

use ccusage_adapter_common::collect_usage_files;

use crate::Result;

pub(crate) const WORKBUDDY_HOME_ENV: &str = "WORKBUDDY_HOME";

fn roots() -> Vec<PathBuf> {
    let configured = env::var(WORKBUDDY_HOME_ENV).ok().map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    });
    let candidates = configured
        .filter(|paths| !paths.is_empty())
        .unwrap_or_else(|| {
            crate::home::home_dir()
                .map(|home| vec![home.join(".workbuddy-ai"), home.join(".workbuddy")])
                .unwrap_or_default()
        });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|path| path.is_dir())
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

pub(super) fn discover_usage_files() -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for root in roots() {
        let projects = root.join("projects");
        if projects.is_dir() {
            collect_usage_files(&projects, &mut files);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use ccusage_test_support::{EnvVarsGuard, fs_fixture};

    use super::*;

    #[test]
    fn discovers_nested_project_jsonl_files_from_multiple_roots() {
        let fixture = fs_fixture!({
            "cn/projects/project/session-a.jsonl": "{}\n",
            "ai/projects/project/session-b.jsonl": "{}\n",
            "ai/traces/process/trace.json": "{}\n",
        });
        let homes = format!(
            "{},{}",
            fixture.path("cn").display(),
            fixture.path("ai").display()
        );
        let _guard = EnvVarsGuard::set_many([(WORKBUDDY_HOME_ENV, Some(OsString::from(homes)))]);

        let files = discover_usage_files().unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|path| path.ends_with("session-a.jsonl")));
        assert!(files.iter().any(|path| path.ends_with("session-b.jsonl")));
    }

    #[test]
    fn blank_home_falls_back_to_both_default_editions() {
        let fixture = fs_fixture!({
            ".workbuddy-ai/projects/project/session-a.jsonl": "{}\n",
            ".workbuddy/projects/project/session-b.jsonl": "{}\n",
        });
        let home = OsString::from(fixture.root().as_os_str());
        let _guard = EnvVarsGuard::set_many([
            (WORKBUDDY_HOME_ENV, Some(OsString::from("  "))),
            ("HOME", Some(home.clone())),
            ("USERPROFILE", Some(home)),
        ]);

        assert_eq!(discover_usage_files().unwrap().len(), 2);
    }
}
