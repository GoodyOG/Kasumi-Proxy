#!/system/bin/sh
# ============================================================
# kasumi-update.sh — update kasumi-proxy WITHOUT rebooting.
#
# Magisk only needs a reboot when a module overlays /system.
# Kasumi doesn't — it's a userspace daemon + binaries in /data.
# So we can overwrite the module files live and restart the daemon.
#
# Usage (in Termux, as root):
#   su -c "sh /sdcard/kasumi-update.sh /sdcard/Download/kasumi-xxx.zip"
# After one flash of a build that bundles this script, use the on-device copy:
#   su -c "sh /data/adb/modules/kasumi-proxy/kasumi-update.sh /sdcard/Download/kasumi-xxx.zip"
#
# What it does:
#   1. stops the running daemon (keeps iptables/TUN intact in kernel)
#   2. unzips the new module over /data/adb/modules/kasumi-proxy
#   3. fixes permissions
#   4. restarts the daemon via service.sh
# ============================================================
set -e

ZIP="${1:?usage: kasumi-update.sh /path/to/kasumi-xxx.zip}"
MODDIR="/data/adb/modules/kasumi-proxy"
DATADIR="/data/adb/kasumi-proxy"
RUNDIR="$DATADIR/run"

[ -f "$ZIP" ] || { echo "zip not found: $ZIP"; exit 1; }
[ -d "$MODDIR" ] || { echo "module not installed: $MODDIR"; exit 1; }

echo "→ stopping daemon…"
if [ -f "$RUNDIR/daemon.pid" ]; then
  kill "$(cat "$RUNDIR/daemon.pid")" 2>/dev/null || true
fi
# belt & suspenders: kill by pattern too
pkill -f "kasumi-proxy.*daemon" 2>/dev/null || true
sleep 2
if pgrep -f "kasumi-proxy.*daemon" >/dev/null 2>&1; then
  echo "  daemon still alive, forcing…"
  pkill -9 -f "kasumi-proxy.*daemon" 2>/dev/null || true
  sleep 1
fi

echo "→ extracting $ZIP → $MODDIR"
unzip -o -q "$ZIP" -d "$MODDIR"

echo "→ fixing permissions…"
chmod 755 "$MODDIR"/bin/* 2>/dev/null || true
chmod 755 "$MODDIR"/*.sh 2>/dev/null || true

echo "→ restarting daemon…"
# service.sh backgrounds the daemon itself
nohup "$MODDIR/service.sh" >/dev/null 2>&1 &
sleep 3

if [ -f "$RUNDIR/daemon.pid" ] && kill -0 "$(cat "$RUNDIR/daemon.pid")" 2>/dev/null; then
  echo "✅ daemon running (pid $(cat "$RUNDIR/daemon.pid"))"
else
  echo "⚠️  daemon pidfile missing — check $DATADIR/daemon.log"
  tail -20 "$DATADIR/daemon.log" 2>/dev/null
  exit 1
fi

echo "done. open the WebUI from the Magisk/KernelSU app (action button)."
