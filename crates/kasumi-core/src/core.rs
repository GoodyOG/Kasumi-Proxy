//! Core-engine resolution — profile override + per-protocol defaults, with
//! capability guards for transports/protocols xray cannot build (those are
//! reported via [`unsupported_reason`]; sing-box was removed from this build).

use crate::enums::{
    CoreEngine, Flow, HeaderType, Network, PacketEncoding, Security, SsMethod, TunEngine,
};
use crate::mixins::Transport;
use crate::profile::{Profile, Protocol};
use crate::state::AdvancedSettings;

/// Protocols that only sing-box implemented. They are kept as data (parsing and
/// import still work) but cannot run in this xray-only build.
fn unsupported_protocol(proto: Protocol) -> bool {
    matches!(
        proto,
        Protocol::Hysteria2
            | Protocol::Tuic
            | Protocol::Anytls
            | Protocol::Naive
            | Protocol::Shadowtls
    )
}

/// Why a profile cannot run in this build (`None` = xray can run it). Covers the
/// removed sing-box-only protocols and the transports/ciphers/options only
/// sing-box implemented.
pub fn unsupported_reason(p: &Profile) -> Option<&'static str> {
    let proto = p.protocol();
    if unsupported_protocol(proto) {
        return Some("protocol requires sing-box, which was removed from this build");
    }
    if let Profile::Shadowsocks(ss) = p {
        if matches!(
            ss.method,
            SsMethod::Chacha20IetfPoly1305
                | SsMethod::Blake3Aes128Gcm
                | SsMethod::Blake3Aes256Gcm
                | SsMethod::Blake3Chacha20Poly1305
        ) {
            return Some("shadowsocks 2022/chacha20-ietf ciphers require sing-box");
        }
        // Note: `ss.tls.security` defaults to `Tls` (the enum default) even for a
        // plain ss:// link, so it can't signal real TLS. The parser records an
        // actual plugin/transport instead: v2ray-plugin sets a Ws/Quic transport,
        // obfs-http sets the Http header — both caught below. Plain-TCP SS runs
        // fine on xray's shadowsocks outbound (which ignores the tls shape).
        if ss.transport.network() != Network::Tcp || ss.transport.header_type() == HeaderType::Http
        {
            return Some(
                "shadowsocks plugin transport / http-header obfuscation requires sing-box",
            );
        }
    }
    if let Some(t) = p.transport() {
        match t {
            Transport::H2(_) | Transport::Quic(_) => {
                return Some("h2/quic transports require sing-box");
            }
            Transport::Grpc(g) if g.ping_timeout > 0 => {
                return Some("grpc ping_timeout requires sing-box");
            }
            _ => {}
        }
    }
    match p {
        Profile::Vless(v) if v.packet_encoding == PacketEncoding::Packetaddr => {
            return Some("packetaddr encoding requires sing-box");
        }
        Profile::Vmess(v) => {
            if v.packet_encoding == PacketEncoding::Packetaddr {
                return Some("packetaddr encoding requires sing-box");
            }
            if v.vmess_global_padding || v.vmess_authenticated_length {
                return Some("vmess padding options require sing-box");
            }
        }
        _ => {}
    }
    None
}

/// Engine a protocol uses when nothing overrides it: xray, the only core left.
pub fn default_core_for(_proto: Protocol) -> CoreEngine {
    CoreEngine::Xray
}

/// Engine the profile MUST run on (an xray-only capability), or `None` if it's
/// freely runnable on xray. Profiles needing sing-box-only capabilities are not
/// forced anywhere — they are rejected via [`unsupported_reason`].
pub fn forced_core(p: &Profile) -> Option<CoreEngine> {
    use CoreEngine::Xray;
    let proto = p.protocol();
    if proto == Protocol::Custom {
        return Some(Xray);
    }

    // ── Protocol-level differences ──
    match p {
        Profile::Vless(v) => {
            if v.flow == Flow::VisionUdp443 {
                return Some(Xray);
            }
            if !v.encryption.is_empty() && v.encryption != "none" {
                return Some(Xray);
            }
        }
        Profile::Trojan(t) if t.flow != Flow::Empty => {
            return Some(Xray);
        }
        Profile::Shadowsocks(ss) => {
            // Ciphers only Xray implements — these must run on Xray.
            if matches!(
                ss.method,
                SsMethod::Plain | SsMethod::Chacha20Poly1305 | SsMethod::Xchacha20Poly1305
            ) {
                return Some(Xray);
            }
        }
        _ => {}
    }

    // ── Transport-level differences ──
    if let Some(t) = p.transport() {
        match t {
            Transport::Kcp(_) | Transport::Xhttp(_) => return Some(Xray),
            Transport::Httpupgrade(h) if h.accept_proxy_protocol || h.early_data > 0 => {
                return Some(Xray);
            }
            Transport::Ws(w) if w.heartbeat_period > 0 || w.accept_proxy_protocol => {
                return Some(Xray);
            }
            Transport::Grpc(g) => {
                // The parser folds the path fallback into `service_name`.
                if g.service_name.starts_with('/') {
                    return Some(Xray);
                }
                if !g.authority.is_empty()
                    || g.mode == "multi"
                    || g.health_check_timeout > 0
                    || g.initial_window_size > 0
                    || !g.user_agent.is_empty()
                {
                    return Some(Xray);
                }
            }
            Transport::Tcp(_) => {}
            // H2/Quic are sing-box-only → covered by `unsupported_reason`.
            _ => {}
        }
    }

    // ── TLS / Reality-level differences (Xray-only fields) ──
    if let Some(tls) = p.tls() {
        if tls.reject_unknown_sni || tls.enable_session_resumption || !tls.vcn.is_empty() {
            return Some(Xray);
        }
        if tls.security == Security::Reality && !tls.pqv.is_empty() {
            return Some(Xray);
        }
    }

    None
}

/// Resolve the actual core for a profile (capabilities > override > table > fallback).
pub fn resolve_core(p: &Profile, s: &AdvancedSettings) -> CoreEngine {
    if let Some(forced) = forced_core(p) {
        return forced;
    }
    if let Some(engine) = p.meta().core_type {
        return engine;
    }
    s.core_by_protocol
        .get(&p.protocol())
        .copied()
        .unwrap_or_else(|| default_core_for(p.protocol()))
}

/// TUN engine a core uses when nothing overrides it: xray has no TUN of its own
/// so it defaults to the `tun2socks` helper.
pub fn default_tun_for(_core: CoreEngine) -> TunEngine {
    TunEngine::Tun2socks
}

/// Resolve the TUN engine for a resolved core: the per-core override, else the
/// default. Every remaining engine fronts xray's socks inbound with an external
/// tun→socks helper process.
pub fn resolve_tun(core: CoreEngine, s: &AdvancedSettings) -> TunEngine {
    s.tun_by_core
        .get(&core)
        .copied()
        .unwrap_or_else(|| default_tun_for(core))
}

/// Per-core TUN-engine options for the settings UI: for each core its default
/// engine and the engines valid to pick. Derived from the very same
/// [`default_tun_for`]/[`resolve_tun`] rules the data-path uses (a variant is valid
/// for a core iff `resolve_tun` keeps it), so the UI never re-encodes the per-core
/// default or validity; they are emitted to the frontend from here
/// (see `defaults.rs` → `TUN_BY_CORE`).
pub fn tun_by_core_options() -> Vec<TunCoreOptions> {
    use strum::IntoEnumIterator;
    CoreEngine::iter()
        .map(|core| {
            let valid = TunEngine::iter()
                .filter(|&tun| {
                    let mut s = AdvancedSettings::default();
                    s.tun_by_core.insert(core, tun);
                    resolve_tun(core, &s) == tun
                })
                .collect();
            TunCoreOptions {
                core,
                default: default_tun_for(core),
                valid,
            }
        })
        .collect()
}

/// One core's TUN-engine options (see [`tun_by_core_options`]).
pub struct TunCoreOptions {
    pub core: CoreEngine,
    pub default: TunEngine,
    pub valid: Vec<TunEngine>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::parse_share_link;

    fn p(uri: &str) -> Profile {
        parse_share_link(uri, None).unwrap()
    }

    #[test]
    fn tun_defaults_and_override() {
        use CoreEngine::Xray;
        // xray always fronts an external tun helper; tun2socks is the default.
        assert_eq!(default_tun_for(Xray), TunEngine::Tun2socks);
        let mut s = AdvancedSettings::default();
        assert_eq!(resolve_tun(Xray, &s), TunEngine::Tun2socks);
        // Per-core override wins.
        s.tun_by_core.insert(Xray, TunEngine::Hev);
        assert_eq!(resolve_tun(Xray, &s), TunEngine::Hev);
    }

    #[test]
    fn defaults_by_protocol() {
        assert_eq!(default_core_for(Protocol::Vless), CoreEngine::Xray);
        assert_eq!(default_core_for(Protocol::Hysteria2), CoreEngine::Xray);
        assert_eq!(default_core_for(Protocol::Tuic), CoreEngine::Xray);
    }

    #[test]
    fn removed_protocols_report_unsupported() {
        for uri in [
            "hysteria2://pw@e.x:443",
            "tuic://u:pw@t.ex:443?sni=t.ex",
            "anytls://pw@a.ex:443?sni=a.ex",
            "naive+https://u:pw@e.x:443",
            "shadowtls://pw@e.x:443",
        ] {
            let prof = p(uri);
            assert!(
                unsupported_reason(&prof).is_some(),
                "{uri} should be unsupported"
            );
            // Not forced anywhere — rejection happens via unsupported_reason.
            assert_eq!(forced_core(&prof), None);
        }
    }

    #[test]
    fn removed_capabilities_report_unsupported() {
        // h2 transport → was sing-box-only.
        let h2 = p("vless://u@e.x:443?type=h2&security=tls");
        assert!(unsupported_reason(&h2).is_some());
        assert_eq!(forced_core(&h2), None);
        // quic transport → was sing-box-only.
        let quic = p("vless://u@e.x:443?type=quic&security=tls");
        assert!(unsupported_reason(&quic).is_some());
        // 2022 shadowsocks cipher → was sing-box-only.
        let ss2022 = p("ss://2022-blake3-aes-128-gcm:pw@e.x:443#X");
        assert!(unsupported_reason(&ss2022).is_some());
        // packetaddr encoding → was sing-box-only.
        let mut v = p("vless://u@e.x:443?type=tcp&security=tls");
        if let Profile::Vless(x) = &mut v {
            x.packet_encoding = crate::enums::PacketEncoding::Packetaddr;
        }
        assert!(unsupported_reason(&v).is_some());
        // vmess global padding → was sing-box-only.
        let mut vm = crate::profile::empty_profile(Protocol::Vmess, "g-main");
        if let Profile::Vmess(x) = &mut vm {
            x.vmess_global_padding = true;
        }
        assert!(unsupported_reason(&vm).is_some());
    }

    #[test]
    fn supported_profiles_have_no_unsupported_reason() {
        for uri in [
            "vless://u@e.x:443?type=tcp&security=tls&sni=s",
            // vmess links are base64-wrapped JSON
            "vmess://eyJ2IjogIjIiLCAicHMiOiAieCIsICJhZGQiOiAiZS54IiwgInBvcnQiOiAiNDQzIiwgImlkIjogImI4ZThhOGU4LThlOGEtNGU4YS04ZThhLThlOGE4ZThhOGU4YSIsICJhaWQiOiAiMCIsICJuZXQiOiAidGNwIiwgInR5cGUiOiAibm9uZSIsICJob3N0IjogIiIsICJwYXRoIjogIiIsICJ0bHMiOiAiIn0=",
            "trojan://pw@e.x:443?security=tls",
            "ss://chacha20-poly1305:pw@h.ex:443#x",
            "wireguard://pw@e.x:51820",
        ] {
            let prof = p(uri);
            assert_eq!(unsupported_reason(&prof), None, "{uri}");
        }
    }

    #[test]
    fn xray_only_capabilities() {
        // vless + reality + pqv → xray
        assert_eq!(
            forced_core(&p(
                "vless://u@e.x:443?type=tcp&security=reality&pbk=PK&sni=s&pqv=Q"
            )),
            Some(CoreEngine::Xray)
        );
        // xhttp transport → xray
        assert_eq!(
            forced_core(&p("vless://u@e.x:443?type=xhttp&security=tls")),
            Some(CoreEngine::Xray)
        );
        // trojan flow → xray
        assert_eq!(
            forced_core(&p("trojan://pw@e.x:443?security=tls&flow=xtls-rprx-vision")),
            Some(CoreEngine::Xray)
        );
    }

    #[test]
    fn shadowsocks_supported_ciphers_stay_on_xray() {
        use CoreEngine::Xray;
        for m in ["plain", "chacha20-poly1305", "xchacha20-poly1305"] {
            assert_eq!(
                forced_core(&p(&format!("ss://{m}:pw@h.ex:443#x"))),
                Some(Xray),
                "{m} must route to xray"
            );
        }
    }

    #[test]
    fn more_url_reachable_forced_branches() {
        use CoreEngine::Xray;
        // The udp443 vision flow is an Xray-only variant.
        assert_eq!(
            forced_core(&p(
                "vless://u@e.x:443?type=tcp&security=tls&flow=xtls-rprx-vision-udp443"
            )),
            Some(Xray)
        );
        // A non-"none" vless encryption is Xray-only.
        assert_eq!(
            forced_core(&p(
                "vless://u@e.x:443?type=tcp&security=tls&encryption=mlkem768"
            )),
            Some(Xray)
        );
        // mKCP transport → Xray.
        assert_eq!(forced_core(&p("vless://u@e.x:443?type=kcp")), Some(Xray));
        // gRPC service-name with a leading slash (the parser's path fallback) → Xray.
        assert_eq!(
            forced_core(&p(
                "vless://u@e.x:443?type=grpc&serviceName=/svc&security=tls"
            )),
            Some(Xray)
        );
        // ws accept-proxy-protocol is an Xray-only knob.
        assert_eq!(
            forced_core(&p(
                "vless://u@e.x:443?type=ws&security=tls&acceptProxyProtocol=1"
            )),
            Some(Xray)
        );
    }

    #[test]
    fn forced_core_struct_only_fields() {
        // A ws heartbeat is an Xray-only feature.
        let mut wsf = p("vless://u@e.x:443?type=ws&security=tls");
        if let Profile::Vless(x) = &mut wsf
            && let Transport::Ws(w) = &mut x.transport
        {
            w.heartbeat_period = 5;
        }
        assert_eq!(forced_core(&wsf), Some(CoreEngine::Xray));
    }

    #[test]
    fn selectable_falls_through_to_override_and_table() {
        // plain vless tcp tls: not forced.
        let prof = p("vless://u@e.x:443?type=tcp&security=tls");
        assert_eq!(forced_core(&prof), None);
        let s = AdvancedSettings::default();
        assert_eq!(resolve_core(&prof, &s), CoreEngine::Xray); // default table

        // per-profile override still wins over the table (only xray exists now).
        let mut prof = prof;
        if let Profile::Vless(v) = &mut prof {
            v.meta.core_type = Some(CoreEngine::Xray);
        }
        assert_eq!(resolve_core(&prof, &s), CoreEngine::Xray);
    }
}
