# Fallback Order

When `ACR_FALLBACK_ENABLED=1` (default) and the primary LLM has a transient
failure, cgen tries fallback presets in the configured order. All attempts share
one total 120-second deadline.

- Configure fallback order from the `cgen config` menu under "Configure fallback order..."
- Presets matching the current config are skipped
- Transport failures and HTTP 408/409/425/429/5xx may fall back
- Authentication, invalid-request, configuration, and response-format errors stop immediately
- A summary of all failures is shown if every provider fails
