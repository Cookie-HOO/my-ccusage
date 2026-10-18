# WorkBuddy source format

Each accepted `projects/**/*.jsonl` line contains a timestamp, session ID, cwd, and `providerData.usage`.

Token mapping:

- input: `usage.inputTokens - cacheRead`
- output: `usage.outputTokens`
- cache read: first non-zero of `rawUsage.prompt_cache_hit_tokens`, `rawUsage.prompt_tokens_details.cached_tokens`, `usage.inputTokensDetails[0].cached_tokens`
- cache creation: `0` (the observed top-level placeholders are not billing facts)
- total: `usage.totalTokens`, with any unexplained remainder retained as extra total tokens

The concrete model is selected from `model`, `requestModelId`, and `requestModelName`, avoiding auto-tier placeholder names. Per-call `rawUsage.credit` is retained as credits metadata and is never presented as USD cost.
