//! Profile deduplication helpers (extracted from the removed subscription applier).

use std::collections::{HashMap, HashSet};

use crate::profile::Profile;

/// Drop duplicate endpoints, keeping the first (or the active one).
pub fn deduplicate_profiles(
    profiles: &[Profile],
    active_id: Option<&str>,
) -> (Vec<Profile>, usize) {
    let mut seen: HashMap<String, String> = HashMap::new(); // key -> kept profile id
    for p in profiles {
        let key = profile_dedup_key(p);
        let is_active = active_id == Some(p.meta().id.as_str());
        if !seen.contains_key(&key) || is_active {
            seen.insert(key, p.meta().id.clone());
        }
    }
    let kept: Vec<Profile> = profiles
        .iter()
        .filter(|p| {
            seen.get(&profile_dedup_key(p)).map(String::as_str) == Some(p.meta().id.as_str())
        })
        .cloned()
        .collect();
    let removed = profiles.len() - kept.len();
    (kept, removed)
}

/// Dedup only within `group_id` (or everything when it's `None`/`"all"`), keeping
/// profiles outside the scope untouched. Returns the surviving profiles and the ids
/// that were dropped.
pub fn deduplicate_profiles_scoped(
    profiles: &[Profile],
    active_id: Option<&str>,
    group_id: Option<&str>,
) -> (Vec<Profile>, HashSet<String>) {
    let affected: Vec<Profile> = match group_id {
        None | Some("all") => profiles.to_vec(),
        Some(g) => profiles
            .iter()
            .filter(|p| p.meta().group_id == g)
            .cloned()
            .collect(),
    };
    let (kept_affected, _) = deduplicate_profiles(&affected, active_id);
    let kept_ids: HashSet<&str> = kept_affected.iter().map(|p| p.meta().id.as_str()).collect();
    let removed_ids: HashSet<String> = affected
        .iter()
        .filter(|p| !kept_ids.contains(p.meta().id.as_str()))
        .map(|p| p.meta().id.clone())
        .collect();
    let kept = profiles
        .iter()
        .filter(|p| !removed_ids.contains(&p.meta().id))
        .cloned()
        .collect();
    (kept, removed_ids)
}

fn profile_dedup_key(p: &Profile) -> String {
    let mut v = serde_json::to_value(p).expect("profile serializes");
    if let Some(meta) = v.get_mut("meta").and_then(|m| m.as_object_mut()) {
        for k in ["id", "remarks", "subId", "groupId", "coreType", "via"] {
            meta.remove(k);
        }
    }
    v.to_string()
}
