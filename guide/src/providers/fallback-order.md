# Fallback Order

When `ACR_FALLBACK_ENABLED=1` (default) and the primary LLM call has a
transient failure, cgen tries your saved [presets](presets.md) as fallbacks in
a configurable order, so a rate-limited or briefly unavailable provider doesn't
block the commit. All attempts share one total 120-second deadline.

- Configure the order from the `cgen config` menu under "Configure fallback order...", or directly with `cgen fallback`
- Presets matching the current config are skipped
- Transport failures and HTTP 408/409/425/429/5xx may fall back
- Authentication, invalid-request, configuration, and response-format errors stop immediately — retrying another provider can't fix those
- A summary of all failures is shown if every provider fails
