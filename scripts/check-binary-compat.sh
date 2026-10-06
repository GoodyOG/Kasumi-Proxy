#!/usr/bin/env bash
# ============================================================
# scripts/check-binary-compat.sh
# Run the config-validation harness against the real xray core, so a core
# version whose config schema drifted from our generator (a config the core
# now rejects) fails loudly. Used two ways:
#   - release.yml gates the auto-bump on it: a core bump that needs generator
#     changes blocks the release instead of shipping a broken build.
#   - core-compat.yml runs it on a schedule against the LATEST upstream xray for
#     early warning (opens a tracking issue) before the bump ever happens.
#
# Versions come from scripts/binary-versions.sh; override the one under test with
# the usual env var, e.g. XRAY_VERSION=v26.4.0 scripts/check-binary-compat.sh
#
# Usage:
#   KASUMI_XRAY_BIN=/path/to/xray scripts/check-binary-compat.sh   # use a staged binary
#   scripts/check-binary-compat.sh                                 # download the pinned one
# ============================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/binary-versions.sh
. "$ROOT/scripts/binary-versions.sh"

if [ -z "${KASUMI_XRAY_BIN:-}" ]; then
	# Download the pinned xray release to a temp dir for the check.
	TMP="$(mktemp -d)"
	trap 'rm -rf "$TMP"' EXIT
	ver="${XRAY_VERSION#v}"
	arch="$(uname -m)"
	case "$arch" in
	x86_64) asset="Xray-linux-64.zip" ;;
	aarch64) asset="Xray-linux-arm64-v8a.zip" ;;
	*) echo "❌ unsupported host arch for compat check: $arch" >&2; exit 1 ;;
	esac
	curl -fL --retry 3 -o "$TMP/xray.zip" \
		"https://github.com/XTLS/Xray-core/releases/download/$XRAY_VERSION/$asset"
	unzip -o -q "$TMP/xray.zip" -d "$TMP/xray"
	export KASUMI_XRAY_BIN="$TMP/xray/xray"
fi

# Run the harness with the staged core present (it auto-detects KASUMI_XRAY_BIN
# and validates every generated config against the real core). A rejected config
# fails the test — and therefore this script.
cargo test --manifest-path "$ROOT/Cargo.toml" \
	-p kasumi-core --test core_validation -- --nocapture
