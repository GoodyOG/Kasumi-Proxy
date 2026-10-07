#!/usr/bin/env bash
# ============================================================
# scripts/fetch-binaries.sh
# Download the proxy core binaries (xray, tun2socks, hev-socks5-tunnel) for Android
# and stage them as the Magisk module payload:
#
#   fetch-binaries.sh android
#     → module/bin/arm64-v8a/<binary>  (arm64 only)
#
# The prebuilt cores' release-asset layout is read from scripts/binaries.json (the
# single source of truth, shared with update-binary-hashes.sh); versions come from
# scripts/binary-versions.sh. These binaries are NOT committed (.gitignore).
# ============================================================
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/binary-versions.sh
. "$HERE/binary-versions.sh"
CATALOG="$HERE/binaries.json"
ROOT="${PROJECT_ROOT:-$(cd "$HERE/.." && pwd)}"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

need() { command -v "$1" >/dev/null 2>&1 || {
	echo "❌ missing dependency: $1" >&2
	exit 1
}; }
need curl
need jq
need unzip
need tar

dl() {
	echo "  ↓ $1" >&2
	curl -fL --retry 3 -o "$2" "$1"
}

extract() { # <archive> <kind:zip|tgz> <destdir>
	mkdir -p "$3"
	case "$2" in
	zip) unzip -o -q "$1" -d "$3" ;;
	tgz) tar -xzf "$1" -C "$3" ;;
	*)
		echo "❌ unknown archive kind: $2" >&2
		exit 1
		;;
	esac
}

# First file matching the member glob (or member+.exe on Windows). head -1 because
# a blind `find -exec cp` would silently clobber on multiple hits.
locate() { # <dir> <member-glob> <ext>
	local src
	src=$(find "$1" -type f \( -name "$2" -o -name "$2$3" \) | head -n1)
	[ -n "$src" ] || {
		echo "❌ no match for '$2' under $1" >&2
		exit 1
	}
	printf '%s' "$src"
}

# Download + extract the core's archive for <arch> into $TMP/<core>-<arch>/, then
# copy the located binary to <dest>. A 'raw' asset IS the binary — downloaded
# straight to <dest>, nothing to extract.
stage_core() { # <core> <arch> <dest> <ext>
	local core="$1" arch="$2" dest="$3" ext="$4"
	local repo vvar member file archive tag ver url dir
	repo=$(jq -r --arg c "$core" '.[$c].repo' "$CATALOG")
	vvar=$(jq -r --arg c "$core" '.[$c].version_var' "$CATALOG")
	member=$(jq -r --arg c "$core" '.[$c].member' "$CATALOG")
	file=$(jq -r --arg c "$core" --arg a "$arch" '.[$c].assets[$a].file' "$CATALOG")
	archive=$(jq -r --arg c "$core" --arg a "$arch" '.[$c].assets[$a].archive' "$CATALOG")
	tag="${!vvar}"
	ver="${tag#v}"
	file="${file//\{ver\}/$ver}"
	url="https://github.com/$repo/releases/download/$tag/$file"
	dir="$TMP/$core-$arch"
	if [ "$archive" = "raw" ]; then
		dl "$url" "$dest"
	else
		dl "$url" "$dir.ar"
		extract "$dir.ar" "$archive" "$dir"
		cp "$(locate "$dir" "$member" "$ext")" "$dest"
	fi
	printf '%s' "$dir"
}

CORES="xray tun2socks hev-socks5-tunnel"

fetch_android() {
	local out="$ROOT/module/bin"
	echo "→ android binaries → module/bin/"
	# abi (module dir) : catalog arch. arm64 only — x86_64 dropped (no x86 devices in use).
	for pair in "arm64-v8a:android-arm64"; do
		local abi="${pair%%:*}" arch="${pair#*:}"
		mkdir -p "$out/$abi"
		for core in $CORES; do
			stage_core "$core" "$arch" "$out/$abi/$core" "" >/dev/null
		done
		chmod 755 "$out/$abi"/xray "$out/$abi"/tun2socks "$out/$abi"/hev-socks5-tunnel
	done
	echo "✅ module/bin/ populated:"
	for abi in arm64-v8a; do
		for core in $CORES; do
			local f="$out/$abi/$core"
			[ -f "$f" ] && printf '   %-22s %s\n' "$abi/$core" "$(du -h "$f" | cut -f1)" || echo "   ⚠️  missing: $abi/$core"
		done
	done
}

case "${1:-}" in
android) fetch_android ;;
*)
	echo "usage: fetch-binaries.sh android" >&2
	exit 2
	;;
esac
