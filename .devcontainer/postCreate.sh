#!/usr/bin/env bash

# Delete any lingering success markers to prevent stale state
rm -f /usr/local/rustup/.postcreate-ok

set -euo pipefail

# Callback to invoke on script failure
function on_error {
    local exit_code=$1
    local line_no=$2

    echo
    echo -e "\033[1;31m========================================================\033[0m"
    echo -e "\033[1;31m ✗ postCreate.sh FAILED (exit code ${exit_code}) at line ${line_no}\033[0m"
    echo -e "\033[1;31m   Your dev container is NOT fully set up.\033[0m"
    echo -e "\033[1;31m   Check the log above, fix the issue, then rebuild the container\033[0m"
    echo -e "\033[1;31m========================================================\033[0m"
    echo
    # Make sure any stale success marker is gone so the bashrc check still warns
    rm -f /usr/local/rustup/.postcreate-oko
}
# Set signal handler to run on error
trap 'on_error $? $LINENO' ERR

CERT_DIR="/usr/local/share/ca-certificates/extra"

# Trust the certificates (e.g., corporate or self-signed CAs)
# provided in .devcontainer/certs.
shopt -s nullglob
cert_files=("$CERT_DIR"/*.crt "$CERT_DIR"/*.pem)
if [ ${#cert_files[@]} -gt 0 ]; then
    echo "[postCreate] Found extra CA certificate(s): ${cert_files[*]##*/}"
    echo "[postCreate] Updating system trust store..."
    sudo update-ca-certificates

    # If Cargo is configured to use rusttls instead of the guest OS CA,
    # it will not find the certificates by default. To work around this,
    # create a global config file for Cargo to point it to the certificate
    # bundle updated by `update-ca-certificates`
    #
    # Since this is put into the container's global config, it never enters
    # version control.
    mkdir -p "${CARGO_HOME:-/usr/local/cargo}"
    cat >> "${CARGO_HOME:-/usr/local/cargo}/config.toml" <<'EOF'

[http]
cainfo = "/etc/ssl/certs/ca-certificates.crt"
EOF
else
    echo "[postCreate] No extra CA certificates found in .devcontainer/certs - skipping."
fi

# Install Rust nightly, if needed
if ! command -v rustc >/dev/null 2>&1; then
    echo "[postCreate] Installing niightly Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --no-modify-path --default-toolchain nightly
else
    echo "[postCreate] Rust already installed, skipping rustup."
fi

# Create a success marker file to indicate that everything went well in this script
touch /usr/local/rustup/.postcreate-ok
