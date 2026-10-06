//! The per-app filter on desktop, where apps are matched by executable instead of
//! by uid.
//!
//! An `appFilter` key `exe:<path>` names an installed program (listed from the
//! desktop's launcher entries). It becomes a routing rule on xray's `process`
//! matcher — bypass apps go `direct`, force-proxy apps go `proxy`, both ahead of
//! the user's routing rules, and in capture-none mode everything else goes
//! `direct`. Android keys (`pkg:uid`) are routed by uid in the platform and never
//! reach this.
//!
//! Xray only sees the process behind a connection that it accepts itself: its
//! local proxy ports (the non-tun modes). Behind an external tun helper every
//! connection comes from the helper, so no rules are emitted there
//! ([`sees_processes`]); the UI says the filter is unavailable.

use serde_json::{Value, json};

use crate::state::{AdvancedSettings, AppCaptureMode, AppFilterMode, ProxyMode};

/// `appFilter` key prefix for a desktop program.
pub const EXE_KEY_PREFIX: &str = "exe:";

/// Whether xray can tell which program opened a connection under `s`.
pub fn sees_processes(s: &AdvancedSettings) -> bool {
    // Xray has no native tun; only its own local proxy ports (the non-tun
    // modes) reveal the connecting process.
    s.proxy_mode != ProxyMode::Tun
}

/// The process names a program shows up under. The executable's own name, plus
/// the name Nix's wrappers give the real binary (`.firefox-wrapped`), since a
/// launcher on NixOS points at the wrapper. A Windows name keeps its `.exe`.
fn process_names(path: &str) -> Vec<String> {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if base.is_empty() {
        return Vec::new();
    }
    let mut names = vec![base.to_string()];
    if !base.to_ascii_lowercase().ends_with(".exe") {
        names.push(format!(".{base}-wrapped"));
    }
    names
}

fn names_for(s: &AdvancedSettings, want: AppFilterMode) -> Vec<String> {
    let mut names: Vec<String> = s
        .app_filter
        .iter()
        .filter(|(_, m)| **m == want)
        .filter_map(|(k, _)| k.strip_prefix(EXE_KEY_PREFIX))
        .flat_map(process_names)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Insert the program rules into a built config, after the rules that must keep
/// running first (traffic sniffing / DNS hijack, the always-on force inbound) and
/// ahead of everything else. A no-op when the filter is empty or the core can't
/// see processes in this mode.
pub fn apply_process_filter(s: &AdvancedSettings, cfg: &mut Value) {
    if !sees_processes(s) {
        return;
    }
    let bypass = names_for(s, AppFilterMode::Bypass);
    let force = names_for(s, AppFilterMode::ForceProxy);
    let capture_none = s.app_capture_mode == AppCaptureMode::None;
    if bypass.is_empty() && force.is_empty() && !capture_none {
        return;
    }
    let mut add: Vec<Value> = Vec::new();
    if !bypass.is_empty() {
        add.push(json!({ "type": "field", "process": bypass, "outboundTag": "direct" }));
    }
    if !force.is_empty() {
        add.push(json!({ "type": "field", "process": force, "outboundTag": "proxy" }));
    }
    if capture_none {
        add.push(json!({ "type": "field", "network": "tcp,udp", "outboundTag": "direct" }));
    }
    let Some(rules) = cfg
        .get_mut("routing")
        .and_then(|r| r.get_mut("rules"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let at = rules
        .iter()
        .rposition(keeps_running_first)
        .map_or(0, |i| i + 1);
    rules.splice(at..at, add);
}

/// A rule the program rules must not pre-empt: the force inbound, DNS answered
/// by the core's DNS module, and that module's own upstream queries.
fn keeps_running_first(rule: &Value) -> bool {
    let tags = |key: &str| -> Vec<&str> {
        rule.get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    };
    rule.get("port").is_some_and(|p| p == 53 || p == "53")
        || tags("inboundTag")
            .iter()
            .any(|t| matches!(*t, "force-in" | "dns-module"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(entries: &[(&str, AppFilterMode)], mode: ProxyMode) -> AdvancedSettings {
        AdvancedSettings {
            proxy_mode: mode,
            app_filter: entries.iter().map(|(k, m)| (k.to_string(), *m)).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn names_cover_nix_wrappers_and_keep_exe() {
        assert_eq!(
            process_names("/usr/bin/firefox"),
            ["firefox", ".firefox-wrapped"]
        );
        assert_eq!(
            process_names(r"C:\Program Files\Mozilla Firefox\firefox.exe"),
            ["firefox.exe"]
        );
    }

    #[test]
    fn xray_rules_follow_dns_and_force_and_precede_user_rules() {
        let s = settings(
            &[
                ("exe:/usr/bin/firefox", AppFilterMode::Bypass),
                ("exe:/usr/bin/telegram-desktop", AppFilterMode::ForceProxy),
                // An Android key is never matched by process.
                ("com.app:10123", AppFilterMode::Bypass),
            ],
            ProxyMode::ProxyOnly,
        );
        let mut cfg = json!({ "routing": { "rules": [
            { "type": "field", "inboundTag": ["force-in"], "outboundTag": "proxy" },
            { "type": "field", "inboundTag": ["socks-in"], "port": 53, "outboundTag": "direct" },
            { "type": "field", "domain": ["geosite:private"], "outboundTag": "direct" },
        ] } });
        apply_process_filter(&s, &mut cfg);
        let rules = cfg["routing"]["rules"].as_array().unwrap();
        assert_eq!(
            rules[2],
            json!({ "type": "field", "process": [".firefox-wrapped", "firefox"], "outboundTag": "direct" })
        );
        assert_eq!(
            rules[3],
            json!({ "type": "field", "process": [".telegram-desktop-wrapped", "telegram-desktop"], "outboundTag": "proxy" })
        );
        assert_eq!(rules[4]["domain"], json!(["geosite:private"]));
        assert_eq!(rules.len(), 5);
    }

    #[test]
    fn capture_none_sends_everything_else_direct() {
        let mut s = settings(
            &[("exe:/usr/bin/curl", AppFilterMode::ForceProxy)],
            ProxyMode::ProxyOnly,
        );
        s.app_capture_mode = AppCaptureMode::None;
        let mut cfg = json!({ "routing": { "rules": [
            { "type": "field", "inboundTag": ["force-in"], "outboundTag": "proxy" },
            { "type": "field", "inboundTag": ["socks-in"], "port": 53, "outboundTag": "direct" },
            { "type": "field", "inboundTag": ["socks-in"], "network": "tcp,udp", "outboundTag": "proxy" },
        ] } });
        apply_process_filter(&s, &mut cfg);
        let rules = cfg["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules[2]["process"], json!([".curl-wrapped", "curl"]));
        assert_eq!(rules[2]["outboundTag"], "proxy");
        assert_eq!(
            rules[3],
            json!({ "type": "field", "network": "tcp,udp", "outboundTag": "direct" })
        );
        assert_eq!(rules[4]["inboundTag"], json!(["socks-in"]));
    }

    #[test]
    fn skipped_where_xray_cannot_see_processes() {
        let s = settings(
            &[("exe:/usr/bin/firefox", AppFilterMode::Bypass)],
            ProxyMode::Tun,
        );
        // xray behind tun2socks/hev: every connection comes from the helper.
        assert!(!sees_processes(&s));
        let mut cfg = json!({ "routing": { "rules": [] } });
        apply_process_filter(&s, &mut cfg);
        assert_eq!(cfg["routing"]["rules"], json!([]));
        // Any non-tun mode sees the process through xray's own proxy ports.
        assert!(sees_processes(&settings(&[], ProxyMode::System)));
        assert!(sees_processes(&settings(&[], ProxyMode::ProxyOnly)));
    }

    #[test]
    fn empty_filter_changes_nothing() {
        let s = settings(&[], ProxyMode::ProxyOnly);
        let mut cfg = json!({ "routing": { "rules": [{ "type": "field", "port": 53 }] } });
        let before = cfg.clone();
        apply_process_filter(&s, &mut cfg);
        assert_eq!(cfg, before);
    }
}
