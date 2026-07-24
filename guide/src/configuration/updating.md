# Updating

## `cgen update`

`cgen update` updates the installation that launched it:

- **Cargo installations** run `cargo install auto-commit-rs --version <release> --locked --force`.
- **Release installations** download the platform artifact and `checksums.sha256`, verify the SHA-256, then atomically replace (or, on Windows, schedule replacement of) that executable.

## Automatic updates

On every run, cgen checks the latest GitHub release tag against the current
version. The first time cgen runs, it asks whether to enable automatic updates
and saves the preference to the global config (`ACR_AUTO_UPDATE` is global-only
and never written to a project `.env`).

- `ACR_AUTO_UPDATE=1`: cgen updates itself automatically when a newer version is found.
- `ACR_AUTO_UPDATE=0` (or unset after the prompt): a notice with the available version is shown at the end of the output instead.
