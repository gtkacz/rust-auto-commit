# Updating

- `cgen update` updates the installation that launched it:
  - Cargo installations run `cargo install auto-commit-rs --version <release> --locked --force`.
  - Release installations download the selected platform artifact and `checksums.sha256`, verify SHA-256, then atomically replace (or, on Windows, schedule replacement of) that executable.
- On every run, cgen checks the latest GitHub release tag against the current version.
- The first time cgen runs, it asks whether to enable automatic updates and saves the preference to the global config.
- If `ACR_AUTO_UPDATE=1`, cgen automatically updates when a newer version is found.
- If `ACR_AUTO_UPDATE=0` (or unset after the prompt), a warning is shown at the end of the output with the available version.
