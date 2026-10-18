# ccusage-adapter-workbuddy

WorkBuddy usage adapter for ccusage's unified reports and the minimal session JSON surface used by ccusage-viz.

It reads model-call usage directly from WorkBuddy's append-only project JSONL files:

- `${WORKBUDDY_HOME}/projects/**/*.jsonl` when `WORKBUDDY_HOME` is set (comma-separated roots are accepted)
- `~/.workbuddy-ai/projects/**/*.jsonl`
- `~/.workbuddy/projects/**/*.jsonl`

The adapter is stateless: it does not create a cache, archive, index, or database, and never modifies WorkBuddy data. Token usage comes only from each record's `providerData.usage`; `workbuddy.db` and trace files are not added as second token sources.

The source mapping was independently implemented in Rust and cross-checked against the MIT-licensed [`flyibeat/workbuddy-usage-dashboard`](https://github.com/flyibeat/workbuddy-usage-dashboard) parser and the published behavior of other local WorkBuddy usage dashboards. No Python dashboard code is linked or executed at runtime.

WorkBuddy reports cached input inside `inputTokens`. The adapter separates it into ccusage's mutually exclusive fields so `inputTokens + outputTokens + cacheReadTokens + cacheCreationTokens == totalTokens`. Reasoning tokens already included in output are not counted again.
