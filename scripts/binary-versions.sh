# shellcheck shell=bash
# Single source of truth for the pinned binary versions, sourced by fetch-binaries.sh
# (and update-binary-hashes.sh / nix). Each value is overridable via the environment
# (the release workflow passes the upstream latest when bumping). release.yml/
# nightly.yml grep these defaults to detect upstream updates — keep the
# `NAME="${NAME:-vX}"` shape.
XRAY_VERSION="${XRAY_VERSION:-v26.1.23}"
TUN2SOCKS_VERSION="${TUN2SOCKS_VERSION:-v2.7.0}"
# Alternative TUN engine (heiher/hev-socks5-tunnel). Selectable per core in
# Settings; pairs with a socks-only core. Tags have no leading 'v'.
HEV_VERSION="${HEV_VERSION:-2.15.0}"

# Windows only: the wintun driver DLL that tun2socks dlopens from the app
# directory. Bundled next to the cores on the Windows target; unused on Linux.
# Pinned to the last upstream wintun.net build.
WINTUN_VERSION="${WINTUN_VERSION:-0.14.1}"
