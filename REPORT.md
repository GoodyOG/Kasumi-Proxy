# Kasumi-Proxy — Magisk Module Analysis

> **Note (2026-10-06):** sing-box has since been removed from this working copy —
> the module is now xray-core only. This report describes the original v0.4.7
> architecture (cloned 2026-10-06, HEAD `da4dd21`) for reference.

Repo: https://github.com/loss-and-quick/Kasumi-Proxy (cloned 2026-10-06, HEAD `da4dd21`)
Module version: **v0.4.7** (versionCode 015) · Author: minicx · License: GPL-3.0
Fork of vincentng295/Magic_V2Ray. README warns: *"Most of the code was written by AI, so review it before you trust it."*

## What it is

A system-level transparent proxy for **rooted Android** (Magisk / KernelSU / APatch).
Unlike VPN apps, it does NOT use Android's VpnService — a root daemon intercepts traffic
in the kernel via Netfilter (`iptables` + `ip rule`), so the low-memory killer can't drop
the tunnel and leak your IP.

## Module anatomy (`module/` = zip root)

| File | Role |
|---|---|
| `module.prop` | id=`kasumi-proxy`, name=Kasumi Proxy, updateJson for OTA |
| `customize.sh` | Installer: picks arch (arm64-v8a / x86_64), extracts matching `bin/`, restores `webroot/`, grants CAMERA to root-manager apps (QR scanner) |
| `service.sh` | Boot: `rotateLogs`, then starts `kasumi-proxy daemon` in background, logging to `/data/adb/kasumi-proxy/daemon.log` |
| `action.sh` | Magisk "Action" button: reads `{port, token}` from `/data/adb/kasumi-proxy/run/ws.json`, opens WebUI in browser |
| `uninstall.sh` | `kasumi-proxy stop` (kills daemon + tears down TUN/iptables idempotently), then wipes `/data/adb/kasumi-proxy` |
| `bin/<abi>/` | NOT in git — built at release time (see below) |
| `webroot/` | NOT in git — React UI build served by the daemon over loopback HTTP |
| `META-INF/` | Standard Magisk installer stub |

## How it runs

1. **Install** — `customize.sh` extracts only the device's arch binaries into `$MODPATH/bin`
   (flattened), keeps everything else as-is. State dir `/data/adb/kasumi-proxy` is created
   once and preserved across upgrades.
2. **Boot** — `service.sh` launches the single Rust binary `kasumi-proxy daemon`.
3. **Daemon** (`crates/kasumi-daemon`, `src/android/`) — one binary, two modes:
   - `daemon`: boot init (route tables, sysctl locks), core + TUN lifecycle, watchdog,
     subscription auto-update, loopback HTTP/WS server (static `webroot/` + `/ws` RPC).
   - one-shot CLI (`readState`, `status`, `wsInfo`, `rotateLogs`, `stop`, …) used by the
     shell scripts; prints JSON.
4. **Routing** (`android/routing.rs`, ~877 lines) — for the **xray** data path: packets are
   marked in a `KASUMI_PROXY_MARK` mangle chain by UID (`owner --uid-owner`), fwmark 255,
   `ip rule` priority 1000/1010/1011 into tables 1100/1101. Per-app filter: capture all
   (uid 1000 + 9999+), strict mode (every uid but root), per-app bypass/force-proxy,
   kill-switch. **sing-box manages its own TUN via auto_route** (routing.rs is xray-only).
   TUN engines: tun2socks, hev-socks5-tunnel, or sing-box. Network switches (Wi-Fi ↔ mobile)
   re-apply rules on the fly.
5. **UI** — daemon serves the React WebUI on `127.0.0.1:<random-port>/?token=<per-start-token>`;
   `action.sh` opens it. All state in `/data/adb/kasumi-proxy/`.

## Bundled binaries (fetched at build, not in git)

| Binary | Role |
|---|---|
| Xray-core (MPL-2.0) | Main core: VLESS, VMess, Trojan, SS, SOCKS, HTTP, WireGuard |
| sing-box (GPL-3.0) | 2nd core: Hysteria2, TUIC, AnyTLS, Naive, ShadowTLS; also a TUN engine |
| tun2socks (MIT) | TUN→SOCKS5 bridge for Xray |
| hev-socks5-tunnel (MIT) | Alt TUN engine |
| geodat2srs | geoip/geosite .dat → sing-box .srs (built from source) |

Versions pinned in `scripts/binary-versions.sh`, overridable via env (`XRAY_VERSION`, …).

## Security posture (per module/AGENTS.md)

- HTTP/WS on **loopback only**; every WS upgrade + RPC checks a **random per-start token**.
- RPC is a fixed typed `Command` set — **no "run shell string" command** (would be RCE).
- Known trade-off: token travels in the page URL → visible to local apps via history/intents.
- Scripts are Android mksh (`#!/system/bin/sh`); lint with `shellcheck -s sh`.

## Build pipeline

`scripts/package-release.sh` → fetch cores → cross-build daemon (`cargo-ndk`,
needs `NDK_ROOT`) → build React UI → zip `module/` as the flashable zip.
Checks: `cargo fmt/clippy/test`, `bun run build/test/check`, `shellcheck -s sh module/*.sh`.

## Pruned working copy

`~/workspace/kasumi-proxy-magisk/` (3.2 MB) — desktop fully removed:
deleted `src-tauri/` (Tauri desktop shell), `docs/`, `.github/`, `nix/`,
`flake.nix`/`flake.lock`/`bun.nix` (nix dev env), `release-signing-key.asc` (AppImage signing).
Kept `frontend/` because the module's WebUI (`webroot/`) is built from it — note the
`frontend/src/lib/bridge-*` files: that's just the backend-bridge abstraction, the module
uses the WebSocket side of it, not the Tauri side.

## Xray-core vs sing-box — and what removing sing-box means

**What they are.**
- **Xray-core** (MPL-2.0): a V2Ray-descendant proxy *protocol specialist*. Its crown jewels
  are anti-censorship transports — XTLS-Vision (`VisionUdp443` flow), REALITY, XHTTP/splitHTTP,
  KCP, gRPC variants. It has **no native TUN**; on Android it always pairs with an external
  TUN helper (tun2socks by default, hev-socks5-tunnel as alternative).
- **sing-box** (GPL-3.0): a universal proxy *platform*. Speaks the newest protocols Xray
  doesn't (Hysteria2, TUIC, AnyTLS, Naive, ShadowTLS) and **owns a native TUN stack**
  (`auto_route`, gVisor/system/mixed stacks), so it needs no helper process.

**The three jobs sing-box does in this module** (all verified in code):
1. **Proxy core** for protocols/transports Xray can't build — forced in
   `crates/kasumi-core/src/core.rs`: Hysteria2, TUIC, AnyTLS, Naive, ShadowTLS;
   H2 and QUIC transports; Shadowsocks 2022/IETF ciphers; VLESS/VMess `packetaddr`
   encoding; VMess global padding/authenticated length; some gRPC variants.
2. **TUN engine** (`TunEngine::SingboxTun`): native TUN when the core *is* sing-box;
   a **sidecar** sing-box in tun→socks bridge mode when the core is Xray and the user
   picks `singbox-tun` in settings.
3. **geodat2srs**: converts geoip/geosite `.dat` → sing-box `.srs` at build time.
   Xray reads `.dat` directly, so this tool becomes pointless without sing-box.

**What you lose if you remove sing-box:**
- Protocols gone entirely: **Hysteria2, TUIC, AnyTLS, Naive, ShadowTLS** (+ H2/QUIC
  transports, SS-2022 ciphers). Everything else — VLESS (incl. Vision/REALITY),
  VMess, Trojan, Shadowsocks (common ciphers), SOCKS, HTTP, WireGuard — stays on Xray.
- The `singbox-tun` engine option disappears (native + sidecar). Xray keeps working
  with tun2socks (default) and hev.
- One of the two big core binaries per arch drops out of the zip → smaller module.
- geodat2srs + the `.srs` conversion step can go too.

**What you'd have to touch** (report only — no code changed yet):
`core.rs` (forced_core/default_core_for — sing-box-only protocols need an "unsupported"
path), `enums.rs` (remove `CoreEngine::SingBox`, `TunEngine::SingboxTun`, `SingboxStack`/
`SingboxFragment` knobs), `singbox_config.rs`, the sidecar lifecycle in
`kasumi-backend/src/lifecycle.rs` + `kasumi-daemon/src/android/platform.rs`,
`customize.sh` + `fetch-binaries.sh` + `binary-versions.sh` (stop fetching sing-box),
`module/bin/licenses/sing-box-LICENSE`, and the frontend's core/TUN pickers
(`CoresSection.tsx`, `ConnectionSection.tsx`, `forms.tsx`). ~30 files reference it,
but it's all mechanical deletion once the core-selection fallbacks are decided.

**Verdict:** removing sing-box is feasible and simplifies the module (one core, one TUN
story, smaller zip). The real cost is protocol coverage — if your users need Hysteria2/
TUIC/AnyTLS, it stays. If Xray's protocol set is enough, sing-box is dead weight.

## Things worth knowing before hacking on it

1. **AI-written code** — the repo itself says to review before trusting; routing.rs and
   platform.rs (~880 lines each) deserve a careful read.
2. **Rename cost** — renaming touches `/data/adb/kasumi-proxy`, the iptables chain name,
   the binary name, `module.prop`, and string literals (grep all case forms).
3. **Rust is source of truth** — TS bindings in `frontend/src/generated/` are codegen'd;
   never hand-edit them.
4. **xray vs sing-box paths differ** — routing.rs only handles xray; sing-box self-manages.
5. No `logprobs`-style concerns here, but the per-start token + loopback binding is the
   whole auth model — don't expose the port beyond loopback.
