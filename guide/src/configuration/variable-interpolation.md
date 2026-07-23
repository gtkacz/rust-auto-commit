# Variable Interpolation

`ACR_API_URL` and `ACR_API_HEADERS` support `$VARIABLE` interpolation from environment variables:

```sh
ACR_API_URL=https://api.example.com/v1/$ACR_MODEL/chat
ACR_API_HEADERS=Authorization: Bearer $ACR_API_KEY, X-Custom: $MY_HEADER
```

Missing variables are errors. Interpolation never mutates the process
environment. Header overrides may use the legacy comma-separated form above or
a JSON object with string values.
