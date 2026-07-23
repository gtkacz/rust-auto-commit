#!/usr/bin/env bash
# cgen installer for Linux and macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.sh | bash
# Pin a release with: CGEN_VERSION=1.3.2 bash install.sh

set -euo pipefail

REPO="gtkacz/smart-commit-rs"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
BINARY_NAME="cgen"

info() { printf "\033[1;34m%s\033[0m\n" "$1"; }
success() { printf "\033[1;32m%s\033[0m\n" "$1"; }
error() { printf "\033[1;31merror:\033[0m %s\n" "$1" >&2; exit 1; }

# Detect OS and architecture
detect_platform() {
    local os arch libc_info

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="linux" ;;
        Darwin) os="macos" ;;
        *)      error "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="amd64" ;;
        arm64|aarch64) arch="arm64" ;;
        *)             error "Unsupported architecture: $arch" ;;
    esac

    if [ "$os" = "linux" ] && [ "$arch" = "amd64" ] && command -v ldd &>/dev/null; then
        libc_info="$(ldd --version 2>&1 || true)"
        if printf '%s' "$libc_info" | grep -qi 'musl'; then
            echo "cgen-linux-amd64-musl"
            return
        fi
    fi

    echo "cgen-${os}-${arch}"
}

# Get latest release tag from GitHub API
get_latest_version() {
    local url="https://api.github.com/repos/$REPO/releases/latest"
    if command -v curl &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -fsSL "$url" | grep '"tag_name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget &>/dev/null; then
        wget --https-only -qO- "$url" | grep '"tag_name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Install one and try again."
    fi
}

download() {
    local url="$1" destination="$2"
    if command -v curl &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -fsSL -o "$destination" "$url"
    elif command -v wget &>/dev/null; then
        wget --https-only -qO "$destination" "$url"
    else
        error "Neither curl nor wget found. Install one and try again."
    fi
}

verify_checksum() {
    local artifact="$1" binary="$2" checksum_file="$3" expected actual
    expected="$(awk -v name="$artifact" '$2 == name || $2 == "*" name { print $1; exit }' "$checksum_file")"
    [ -n "$expected" ] || error "Release does not contain a checksum for $artifact"

    if command -v sha256sum &>/dev/null; then
        actual="$(sha256sum "$binary" | awk '{print $1}')"
    elif command -v shasum &>/dev/null; then
        actual="$(shasum -a 256 "$binary" | awk '{print $1}')"
    else
        error "A SHA-256 tool (sha256sum or shasum) is required"
    fi
    [ "$actual" = "$expected" ] || error "Checksum mismatch for $artifact; installation aborted"
}

main() {
    local artifact version download_url checksum_url tmp_dir staged_target

    info "Detecting platform..."
    artifact="$(detect_platform)"
    info "Platform: $artifact"

    version="${CGEN_VERSION:-}"
    if [ -z "$version" ]; then
        info "Fetching latest release..."
        version="$(get_latest_version)"
    fi

    if ! printf '%s' "$version" | grep -Eq '^v?[0-9]+\.[0-9]+\.[0-9]+$'; then
        error "Could not determine latest version. Check https://github.com/$REPO/releases"
    fi

    info "Latest version: $version"

    download_url="https://github.com/$REPO/releases/download/${version}/${artifact}"
    checksum_url="https://github.com/$REPO/releases/download/${version}/checksums.sha256"

    tmp_dir="$(mktemp -d)"
    trap 'rm -rf -- "$tmp_dir"' EXIT

    info "Downloading $artifact..."
    download "$download_url" "$tmp_dir/$BINARY_NAME"
    download "$checksum_url" "$tmp_dir/checksums.sha256"
    verify_checksum "$artifact" "$tmp_dir/$BINARY_NAME" "$tmp_dir/checksums.sha256"
    chmod +x "$tmp_dir/$BINARY_NAME"

    info "Installing to $INSTALL_DIR/$BINARY_NAME..."
    if [ ! -d "$INSTALL_DIR" ]; then
        if ! mkdir -p "$INSTALL_DIR" 2>/dev/null; then
            command -v sudo &>/dev/null || error "Cannot create $INSTALL_DIR and sudo is unavailable"
            sudo mkdir -p "$INSTALL_DIR"
        fi
    fi
    staged_target="$INSTALL_DIR/.cgen-install-$$"
    if [ -w "$INSTALL_DIR" ]; then
        install -m 0755 "$tmp_dir/$BINARY_NAME" "$staged_target"
        mv -f "$staged_target" "$INSTALL_DIR/$BINARY_NAME"
    else
        command -v sudo &>/dev/null || error "$INSTALL_DIR is not writable and sudo is unavailable"
        sudo install -m 0755 "$tmp_dir/$BINARY_NAME" "$staged_target"
        sudo mv -f "$staged_target" "$INSTALL_DIR/$BINARY_NAME"
    fi

    success "cgen $version installed successfully!"
    echo ""
    echo "  Run 'cgen config' to set up your API key."
    echo "  Run 'cgen --help' for usage information."
}

main
