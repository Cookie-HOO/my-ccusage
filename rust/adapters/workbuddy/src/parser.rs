use std::{fs, path::Path, sync::Arc};

use jiff::tz::TimeZone as JiffTimeZone;
use serde::Deserialize;

use ccusage_adapter_common::jsonl;

use crate::{
    LoadedEntry, Result, TimestampMs, TokenUsageRaw, UsageEntry, UsageMessage, cli::SharedArgs,
    fast::LinePrefilter, format_date_tz, format_rfc3339_millis, parse_tz,
};

const UNKNOWN_MODEL: &str = "unknown";
const CUSTOM_LOCAL_PREFIX: &str = "custom-local:";
const MODEL_PLACEHOLDERS: &[&str] = &[
    "auto",
    "fast-model",
    "balanced-model",
    "smart-model",
    "default",
    "unknown",
    "none",
    "null",
    "fast",
    "balanced",
    "smart",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkBuddyRecord {
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    id: Option<String>,
    #[serde(default, deserialize_with = "jsonl::lenient_i64")]
    timestamp: Option<i64>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    session_id: Option<String>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    cwd: Option<String>,
    #[serde(default, deserialize_with = "jsonl::lenient_object")]
    provider_data: Option<ProviderData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderData {
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    conversation_request_id: Option<String>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    message_id: Option<String>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    trace_id: Option<String>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    model: Option<String>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    request_model_id: Option<String>,
    #[serde(default, deserialize_with = "jsonl::non_empty_string")]
    request_model_name: Option<String>,
    #[serde(default, deserialize_with = "jsonl::lenient_object")]
    usage: Option<WorkBuddyUsage>,
    #[serde(default, deserialize_with = "jsonl::lenient_object")]
    raw_usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkBuddyUsage {
    #[serde(default, deserialize_with = "jsonl::lenient_u64")]
    input_tokens: u64,
    #[serde(default, deserialize_with = "jsonl::lenient_u64")]
    output_tokens: u64,
    #[serde(default, deserialize_with = "jsonl::lenient_u64")]
    total_tokens: u64,
    #[serde(default, deserialize_with = "jsonl::lenient_vec")]
    input_tokens_details: Vec<TokenDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct TokenDetails {
    #[serde(default, deserialize_with = "jsonl::lenient_u64")]
    cached_tokens: u64,
}

#[derive(Debug, Default, Deserialize)]
struct RawUsage {
    #[serde(default, deserialize_with = "jsonl::lenient_u64")]
    prompt_cache_hit_tokens: u64,
    #[serde(default, deserialize_with = "jsonl::lenient_object")]
    prompt_tokens_details: Option<TokenDetails>,
    #[serde(default, deserialize_with = "jsonl::lenient_f64")]
    credit: Option<f64>,
}

pub(super) fn read_usage_file(file: &Path, shared: &SharedArgs) -> Result<Vec<LoadedEntry>> {
    let content = fs::read(file)?;
    let prefilter = LinePrefilter::all(&[br#""providerData""#, br#""usage""#]);
    let fallback_session = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(UNKNOWN_MODEL);
    let tz = parse_tz(shared.timezone.as_deref());
    Ok(
        jsonl::records::<WorkBuddyRecord>(&content, Some(&prefilter))
            .filter_map(|record| to_loaded_entry(record, fallback_session, tz.as_ref()))
            .collect(),
    )
}

fn to_loaded_entry(
    record: WorkBuddyRecord,
    fallback_session: &str,
    tz: Option<&JiffTimeZone>,
) -> Option<LoadedEntry> {
    let timestamp = record.timestamp.filter(|value| *value > 0)?;
    let timestamp = if timestamp < 10_000_000_000 {
        timestamp.saturating_mul(1_000)
    } else {
        timestamp
    };
    let timestamp = TimestampMs::from_millis(timestamp);
    let provider = record.provider_data?;
    let model = pick_model(&provider);
    let source_usage = provider.usage?;
    let raw = provider.raw_usage.unwrap_or_default();
    let cache_read = cache_read_tokens(&raw, &source_usage);
    let input = source_usage.input_tokens.saturating_sub(cache_read);
    let total = source_usage.total_tokens.max(
        source_usage
            .input_tokens
            .saturating_add(source_usage.output_tokens),
    );
    let known = input
        .saturating_add(source_usage.output_tokens)
        .saturating_add(cache_read);
    if known == 0 {
        return None;
    }
    let extra_total_tokens = total.saturating_sub(known);
    let session_id = record
        .session_id
        .unwrap_or_else(|| fallback_session.to_string());
    let project_path = record.cwd.unwrap_or_else(|| UNKNOWN_MODEL.to_string());
    let source_id = provider
        .conversation_request_id
        .or(provider.message_id)
        .or(provider.trace_id)
        .or(record.id);
    let id = source_id.map(|id| format!("workbuddy:{session_id}:{}:{id}", timestamp.as_millis()));
    let usage = TokenUsageRaw {
        input_tokens: input,
        output_tokens: source_usage.output_tokens,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: cache_read,
        ..TokenUsageRaw::default()
    };
    let data = UsageEntry {
        session_id: Some(session_id.clone()),
        timestamp: format_rfc3339_millis(timestamp),
        version: None,
        message: UsageMessage {
            usage,
            model: Some(model.clone()),
            id,
        },
        cost_usd: None,
        request_id: None,
        is_api_error_message: None,
        is_sidechain: None,
    };
    Some(LoadedEntry {
        data,
        timestamp,
        date: format_date_tz(timestamp, tz),
        project: Arc::from("workbuddy"),
        session_id: Arc::from(session_id),
        project_path: Arc::from(project_path),
        cost: 0.0,
        extra_total_tokens,
        credits: raw.credit,
        message_count: Some(1),
        model: Some(model),
        usage_limit_reset_time: None,
        missing_pricing_model: None,
    })
}

fn cache_read_tokens(raw: &RawUsage, usage: &WorkBuddyUsage) -> u64 {
    if raw.prompt_cache_hit_tokens > 0 {
        return raw.prompt_cache_hit_tokens;
    }
    if let Some(tokens) = raw
        .prompt_tokens_details
        .as_ref()
        .map(|details| details.cached_tokens)
        .filter(|tokens| *tokens > 0)
    {
        return tokens;
    }
    usage
        .input_tokens_details
        .first()
        .map_or(0, |details| details.cached_tokens)
}

fn pick_model(provider: &ProviderData) -> String {
    let model = provider.model.as_deref().unwrap_or_default().trim();
    let request_id = provider
        .request_model_id
        .as_deref()
        .unwrap_or_default()
        .trim()
        .strip_prefix(CUSTOM_LOCAL_PREFIX)
        .unwrap_or_else(|| {
            provider
                .request_model_id
                .as_deref()
                .unwrap_or_default()
                .trim()
        });
    if request_id.contains('/') {
        return request_id.to_string();
    }
    if model.is_empty() || MODEL_PLACEHOLDERS.contains(&model.to_ascii_lowercase().as_str()) {
        return first_concrete_model(request_id, provider.request_model_name.as_deref())
            .unwrap_or(UNKNOWN_MODEL)
            .to_string();
    }
    model.to_string()
}

fn first_concrete_model<'a>(id: &'a str, name: Option<&'a str>) -> Option<&'a str> {
    [id, name.unwrap_or_default().trim()]
        .into_iter()
        .find(|value| {
            !value.is_empty() && !MODEL_PLACEHOLDERS.contains(&value.to_ascii_lowercase().as_str())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(model: &str, request_model_id: &str, request_model_name: &str) -> ProviderData {
        ProviderData {
            conversation_request_id: None,
            message_id: None,
            trace_id: None,
            model: Some(model.to_string()),
            request_model_id: Some(request_model_id.to_string()),
            request_model_name: Some(request_model_name.to_string()),
            usage: None,
            raw_usage: None,
        }
    }

    #[test]
    fn picks_concrete_model_across_old_and_new_record_shapes() {
        assert_eq!(
            pick_model(&provider("deepseek-v4", "auto", "Auto")),
            "deepseek-v4"
        );
        assert_eq!(
            pick_model(&provider("auto", "deepseek-v4", "Auto")),
            "deepseek-v4"
        );
        assert_eq!(
            pick_model(&provider("default-model", "default-model", "Auto")),
            "default-model"
        );
        assert_eq!(
            pick_model(&provider(
                "deepseek-v4",
                "custom-local:vendor/deepseek-v4",
                "Auto"
            )),
            "vendor/deepseek-v4"
        );
    }

    #[test]
    fn cache_hit_falls_back_across_the_three_observed_fields() {
        let usage = WorkBuddyUsage {
            input_tokens: 100,
            output_tokens: 5,
            total_tokens: 105,
            input_tokens_details: vec![TokenDetails { cached_tokens: 30 }],
        };
        let raw = RawUsage {
            prompt_cache_hit_tokens: 40,
            prompt_tokens_details: Some(TokenDetails { cached_tokens: 35 }),
            credit: None,
        };
        assert_eq!(cache_read_tokens(&raw, &usage), 40);
        let raw = RawUsage {
            prompt_cache_hit_tokens: 0,
            prompt_tokens_details: Some(TokenDetails { cached_tokens: 35 }),
            credit: None,
        };
        assert_eq!(cache_read_tokens(&raw, &usage), 35);
        let raw = RawUsage::default();
        assert_eq!(cache_read_tokens(&raw, &usage), 30);
    }
}
