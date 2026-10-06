//! Resolve the core engine for a profile and build its exact launch config.
//! Shared by the backend (to launch the core) and config-diffing
//! (restart-on-change).

use serde_json::Value;

use crate::core::{resolve_core, resolve_tun, unsupported_reason};
use crate::enums::{CoreEngine, TunEngine};
use crate::profile::Profile;
use crate::state::{AdvancedSettings, ProxyMode, RoutingRule};
use crate::xray_config::build_xray_config;

/// Engine + TUN engine + config JSON for a profile, mirroring what the core is
/// launched with.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreConfig {
    pub engine: CoreEngine,
    /// The resolved TUN engine for this core. Xray is always built socks-only;
    /// an external tun→socks engine fronts it.
    pub tun: TunEngine,
    /// The built config as a JSON value (serialize to a string when writing it
    /// to disk; comparing values is order-independent for restart diffing).
    pub config: Value,
}

/// Build the launch config for a profile. Fails when the profile needs a
/// capability the xray-only build cannot provide (see
/// [`crate::core::unsupported_reason`]).
pub fn build_core_config(
    profile: &Profile,
    settings: &AdvancedSettings,
    routing_rules: &[RoutingRule],
    profiles: &[Profile],
) -> Result<CoreConfig, String> {
    if let Some(reason) = unsupported_reason(profile) {
        return Err(format!("unsupported profile: {reason}"));
    }
    let engine = resolve_core(profile, settings);
    let tun = resolve_tun(engine, settings);
    let config = match engine {
        CoreEngine::Xray => build_xray_config(profile, settings, routing_rules, profiles)?,
    };
    Ok(CoreConfig {
        engine,
        tun,
        config,
    })
}

/// True when two resolved core configs differ — i.e. the core must be restarted.
/// A TUN-engine switch counts too: it changes how the data-path is brought up even
/// when the xray config JSON is identical.
pub fn active_config_changed(prev: &CoreConfig, next: &CoreConfig) -> bool {
    prev.engine != next.engine || prev.tun != next.tun || prev.config != next.config
}

/// What a settings mutation means for a running data path, decided against the
/// build + proxy mode it was started with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationEffect {
    /// The core config is unchanged and the mode moved within the non-tun family
    /// (proxy-only ↔ system ↔ pac): re-point the OS proxy live, no restart.
    LiveModeSwitch(ProxyMode),
    /// Whether the running data path still matches the saved settings — i.e. a
    /// restart is (`true`) or is no longer (`false`) needed to apply them.
    SetPending(bool),
}

/// Compare the running data path's build + mode against the ones the saved
/// settings now produce. Entering or leaving tun mode always needs a restart
/// (the tun device and routing are start-time constructs), as does any change
/// to the resolved core config; a mode move within the non-tun family is the
/// one live-applicable case.
pub fn mutation_effect(
    running: &CoreConfig,
    running_mode: ProxyMode,
    next: &CoreConfig,
    next_mode: ProxyMode,
) -> MutationEffect {
    let config_changed = active_config_changed(running, next);
    let tunness_changed = (running_mode == ProxyMode::Tun) != (next_mode == ProxyMode::Tun);
    if !config_changed && !tunness_changed && next_mode != running_mode {
        MutationEffect::LiveModeSwitch(next_mode)
    } else {
        MutationEffect::SetPending(config_changed || tunness_changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::parse_share_link;

    #[test]
    fn builds_xray_config() {
        let s = AdvancedSettings::default();
        let xray = parse_share_link("vless://u@e.x:443?type=tcp&security=tls&sni=s", None).unwrap();

        let cx = build_core_config(&xray, &s, &[], std::slice::from_ref(&xray)).unwrap();
        assert_eq!(cx.engine, CoreEngine::Xray);
        assert_eq!(cx.tun, TunEngine::Tun2socks);
        assert!(cx.config["outbounds"].is_array());
    }

    #[test]
    fn removed_protocols_fail_loudly() {
        let s = AdvancedSettings::default();
        let sb = parse_share_link("tuic://u:pw@t.ex:443?sni=t.ex", None).unwrap();
        let err = build_core_config(&sb, &s, &[], std::slice::from_ref(&sb)).unwrap_err();
        assert!(err.contains("sing-box"), "{err}");
    }

    #[test]
    fn tun_override_is_honored() {
        let xray = parse_share_link("vless://u@e.x:443?type=tcp&security=tls&sni=s", None).unwrap();
        let mut s = AdvancedSettings::default();
        s.tun_by_core.insert(CoreEngine::Xray, TunEngine::Hev);
        let c = build_core_config(&xray, &s, &[], std::slice::from_ref(&xray)).unwrap();
        assert_eq!(c.tun, TunEngine::Hev);
    }

    #[test]
    fn change_detection() {
        let s = AdvancedSettings::default();
        let p = parse_share_link("vless://u@e.x:443?type=tcp&security=tls&sni=s", None).unwrap();
        let a = build_core_config(&p, &s, &[], std::slice::from_ref(&p)).unwrap();
        let b = a.clone();
        assert!(!active_config_changed(&a, &b));

        let mut s2 = s.clone();
        s2.fragment = true; // changes the built config
        let c = build_core_config(&p, &s2, &[], std::slice::from_ref(&p)).unwrap();
        assert!(active_config_changed(&a, &c));
    }

    #[test]
    fn mutation_effect_decides_live_apply_vs_pending() {
        let s = AdvancedSettings::default();
        let p = parse_share_link("vless://u@e.x:443?type=tcp&security=tls&sni=s", None).unwrap();
        let cfg = build_core_config(&p, &s, &[], std::slice::from_ref(&p)).unwrap();

        // Identical build + mode: nothing is stale.
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::Tun, &cfg, ProxyMode::Tun),
            MutationEffect::SetPending(false)
        );

        // A changed build needs a restart whatever the mode.
        let mut s2 = s.clone();
        s2.fragment = true;
        let changed = build_core_config(&p, &s2, &[], std::slice::from_ref(&p)).unwrap();
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::Tun, &changed, ProxyMode::Tun),
            MutationEffect::SetPending(true)
        );

        // Entering/leaving tun mode needs a restart even with an identical build.
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::Tun, &cfg, ProxyMode::System),
            MutationEffect::SetPending(true)
        );
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::ProxyOnly, &cfg, ProxyMode::Tun),
            MutationEffect::SetPending(true)
        );

        // Same build, mode moved within the non-tun family: live apply.
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::ProxyOnly, &cfg, ProxyMode::System),
            MutationEffect::LiveModeSwitch(ProxyMode::System)
        );
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::System, &cfg, ProxyMode::Pac),
            MutationEffect::LiveModeSwitch(ProxyMode::Pac)
        );

        // Config change + non-tun mode move: the restart wins over the live apply.
        assert_eq!(
            mutation_effect(&cfg, ProxyMode::ProxyOnly, &changed, ProxyMode::System),
            MutationEffect::SetPending(true)
        );
    }
}
