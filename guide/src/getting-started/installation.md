# Installation

## Linux / macOS (curl)

```sh
curl -fsSL https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.sh | bash
```

This detects your OS and architecture, downloads the latest release binary to `/usr/local/bin`, and makes it executable. Set `INSTALL_DIR` to change the target:

```sh
INSTALL_DIR=~/.local/bin curl -fsSL https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.sh | bash
```

## Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.ps1 | iex
```

This downloads the latest release to `%LOCALAPPDATA%\cgen\` and adds it to your user PATH.

## Cargo

From [crates.io](https://crates.io/crates/auto-commit-rs):

```sh
cargo install auto-commit-rs
```

From git:

```sh
cargo install --git https://github.com/gtkacz/smart-commit-rs
```

## Manual Download

Grab a binary from the [Releases](https://github.com/gtkacz/smart-commit-rs/releases) page and place it somewhere in your PATH.

Available binaries:
- `cgen-linux-amd64`, Linux x86_64
- `cgen-linux-amd64-musl`, portable Linux x86_64 (static musl build, works on any distro)
- `cgen-linux-arm64`, Linux ARM64
- `cgen-macos-amd64`, macOS Intel
- `cgen-macos-arm64`, macOS Apple Silicon
- `cgen-windows-amd64.exe`, Windows x86_64

Release artifacts ship with a `checksums.sha256` file for verification.

## Verify

```sh
cgen --version
```

Then continue with the [Quick Start](quick-start.md).
