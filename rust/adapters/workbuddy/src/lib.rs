use ccusage_core::*;

mod loader;
mod parser;
mod paths;
mod report;

use crate::{
    Result,
    cli::{AgentCommandArgs, AgentReportKind, SharedArgs},
    print_json_or_jq, sort_summaries, wants_json,
};

pub use loader::load_entries;
pub use report::summarize_entries;

pub fn run(args: AgentCommandArgs) -> Result<()> {
    if args.kind != AgentReportKind::Session {
        return Err(crate::cli_error(
            "WorkBuddy exposes only the session report directly; use the unified daily report for ccusage-viz",
        ));
    }
    let shared = args.shared;
    if !wants_json(&shared) {
        return Err(crate::cli_error(
            "WorkBuddy's focused session report is JSON-only; add --json",
        ));
    }
    let entries = load_filtered_entries(&shared)?;
    let mut rows = summarize_entries(&entries, AgentReportKind::Session)?;
    sort_summaries(&mut rows, &shared.order, ccusage_core::summary_period);
    print_json_or_jq(
        report::report_from_rows(&rows, AgentReportKind::Session),
        shared.jq.as_deref(),
        shared.no_cost,
    )
}

pub fn has_data() -> bool {
    paths::discover_usage_files().is_ok_and(|files| !files.is_empty())
}

pub fn load_filtered_entries(shared: &SharedArgs) -> Result<Vec<LoadedEntry>> {
    let mut entries = load_entries(shared)?;
    ccusage_adapter_common::filter_loaded_entries_by_date(&mut entries, shared);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use ccusage_test_support::{EnvVarsGuard, fs_fixture};

    use super::*;
    use crate::cli::{AgentReportKind, SharedArgs};

    fn usage_line(
        session: &str,
        timestamp: i64,
        cwd: &str,
        conversation_request_id: &str,
        model: &str,
        request_model_id: &str,
    ) -> String {
        format!(
            r#"{{"id":"message-{conversation_request_id}","timestamp":{timestamp},"type":"message","role":"assistant","sessionId":"{session}","cwd":"{cwd}","providerData":{{"conversationRequestId":"{conversation_request_id}","messageId":"provider-{conversation_request_id}","model":"{model}","requestModelId":"{request_model_id}","requestModelName":"Auto","rawUsage":{{"prompt_tokens":30746,"completion_tokens":434,"total_tokens":31180,"prompt_cache_hit_tokens":12032,"prompt_tokens_details":{{"cached_tokens":12032}},"completion_thinking_tokens":377,"credit":1.92}},"usage":{{"requests":1,"inputTokens":30746,"outputTokens":434,"totalTokens":31180,"inputTokensDetails":[{{"cached_tokens":12032}}],"outputTokensDetails":[{{"reasoning_tokens":377}}]}}}}}}"#
        )
    }

    fn shared() -> SharedArgs {
        SharedArgs {
            timezone: Some("UTC".to_string()),
            ..SharedArgs::default()
        }
    }

    #[test]
    fn normalizes_cached_input_without_double_counting_reasoning() {
        let fixture = fs_fixture!({
            "projects/workspace/session-1.jsonl": usage_line(
                "session-1",
                1792327339000,
                "/workspace/project",
                "request-1",
                "auto",
                "gpt-5.6-sol",
            ),
        });
        let _guard = EnvVarsGuard::set_many([(
            "WORKBUDDY_HOME",
            Some(OsString::from(fixture.root().as_os_str())),
        )]);

        let entries = load_entries(&shared()).unwrap();
        let rows = summarize_entries(&entries, AgentReportKind::Daily).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].data.message.usage.input_tokens, 18_714);
        assert_eq!(entries[0].data.message.usage.output_tokens, 434);
        assert_eq!(
            entries[0].data.message.usage.cache_read_input_tokens,
            12_032
        );
        assert_eq!(entries[0].extra_total_tokens, 0);
        assert_eq!(entries[0].credits, Some(1.92));
        assert_eq!(entries[0].model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(entries[0].project_path.as_ref(), "/workspace/project");
        assert_eq!(rows[0].total_tokens(), 31_180);
    }

    #[test]
    fn deduplicates_the_same_request_across_workbuddy_editions() {
        let line = usage_line(
            "session-1",
            1792327339000,
            "/workspace/project",
            "request-1",
            "gpt-5.6-sol",
            "auto",
        );
        let fixture = fs_fixture!({
            "cn/projects/workspace/session-1.jsonl": line.clone(),
            "ai/projects/workspace/session-1.jsonl": line,
        });
        let homes = format!(
            "{},{}",
            fixture.path("cn").display(),
            fixture.path("ai").display()
        );
        let _guard = EnvVarsGuard::set_many([("WORKBUDDY_HOME", Some(OsString::from(homes)))]);

        assert_eq!(load_entries(&shared()).unwrap().len(), 1);
    }

    #[test]
    fn session_summary_keeps_project_path_for_ccusage_viz_fallback() {
        let fixture = fs_fixture!({
            "projects/workspace/session-1.jsonl": usage_line(
                "session-1",
                1792327339000,
                "/workspace/project",
                "request-1",
                "gpt-5.6-sol",
                "auto",
            ),
        });
        let _guard = EnvVarsGuard::set_many([(
            "WORKBUDDY_HOME",
            Some(OsString::from(fixture.root().as_os_str())),
        )]);

        let entries = load_entries(&shared()).unwrap();
        let rows = summarize_entries(&entries, AgentReportKind::Session).unwrap();

        assert_eq!(rows[0].session_id.as_deref(), Some("session-1"));
        assert_eq!(rows[0].project_path.as_deref(), Some("/workspace/project"));
    }
}
