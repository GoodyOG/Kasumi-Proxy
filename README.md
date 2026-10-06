# Kasumi Proxy

System-wide transparent proxy on **Xray-core** for **rooted Android** — a Magisk / KernelSU / APatch module.

It runs the core as a root daemon and routes traffic with `iptables` / `ip rule`, not through `VpnService`. A WebUI served by the daemon handles profiles, subscriptions, routing and diagnostics.

> [!NOTE]
> This is a fork of [loss-and-quick/Kasumi-Proxy](https://github.com/loss-and-quick/Kasumi-Proxy),
> stripped down to the Android module: the desktop app is gone and sing-box is gone.
> Xray-core is the only core.

## Features

- **Xray-core v26.1.23** — pinned to the newest core without the upstream hardcoded
  restrictions (no `allowInsecure` kill date, no plaintext-outbound ban).
- **Protocols:** VLESS, VMess, Trojan, Shadowsocks (incl. 2022), WireGuard, Hysteria2,
  SOCKS, HTTP — with XTLS-Vision, REALITY, XHTTP/SplitHTTP, WS/gRPC/QUIC/mKCP and ECH.
- **Import:** subscription URLs, share links, mixed text and QR codes.
- **Subscriptions:** groups of servers updated in one tap, or in the background with no UI open.
- **Honest status:** an end-to-end probe tells *connected* apart from *no internet* and *failed*.
- **Diagnostics:** TCP ping, real ping and a speed test per profile.
- **Per-app routing** plus a choice of TUN engine: tun2socks or hev-socks5-tunnel.

### Why a root module rather than a VPN app?

- The root daemon is not killed by the low-memory killer, so the tunnel does not drop and leak your IP.
- Traffic is intercepted in the kernel (Netfilter) and never passes through a user-space `tun0`.
- Switching between Wi-Fi and mobile data reapplies the routing rules on the fly.

## Install

Flash `kasumi-proxy-module-vX.Y.Z.zip` in Magisk / KernelSU / APatch, then reboot once.
Open the UI with the module's **Action** button or its WebUI entry.

State and logs live in `/data/adb/kasumi-proxy/`.

### Updating without rebooting

Kasumi never touches `/system` — it's all userspace in `/data` — so updates don't need a reboot.
After the first flash, push new builds live with Termux (root):

```sh
su -c "sh /sdcard/kasumi-update.sh /sdcard/Download/kasumi-xxx.zip"
```

The script stops the daemon, unzips the new build over the module directory, fixes
permissions and restarts the daemon (~5 seconds). Then tap **Start** in the WebUI.
`kasumi-update.sh` is in `scripts/` — copy it to the phone once.

## Building

Prerequisites: Rust (pinned in `rust-toolchain.toml`), `cargo-ndk`, an Android NDK
(`NDK_ROOT`), and `bun`.

```sh
# one-shot: fetch cores, cross-build the daemon, build the WebUI, zip the module
NDK_ROOT=/path/to/ndk scripts/package-release.sh
```

The zip lands in `build/`. Individual steps: `scripts/fetch-binaries.sh android`,
`scripts/build-daemon-android.sh`, `scripts/build-webroot.sh`.

Checks: `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
`cargo fmt --all --check`, and in `frontend/`: `bun run build`, `bun run test`.

## Acknowledgments

Kasumi Proxy ships prebuilt binaries from
[Xray-core](https://github.com/XTLS/Xray-core),
[tun2socks](https://github.com/xjasonlyu/tun2socks) and
[hev-socks5-tunnel](https://github.com/heiher/hev-socks5-tunnel). See
[module/bin/README.md](module/bin/README.md) for their licenses.

## License

[GPL-3.0](LICENSE)
