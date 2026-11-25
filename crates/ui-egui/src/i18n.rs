//! Interface localization (`en` / `he`) and BCP 47 / legacy `iw` preference normalization.
//!
//! `egui 0.37` and `solidcraft-ui-text` handle visual UAX #9 BiDi line formatting inside
//! `Catalog::shown()`, so `locales/he.json` stores logical Hebrew text.

/// Preference value that follows the operating system or browser language.
pub const SYSTEM: &str = solidcraft_ui_text::SYSTEM;
/// English catalog id (fallback).
pub const ENGLISH: &str = "en";
/// Hebrew catalog id (`עברית`).
pub const HEBREW: &str = "he";

/// A selectable language entry for Settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageChoice {
    pub pref: &'static str,
    pub label: String,
}

/// Strip POSIX encoding/modifier suffixes, normalize separators to `-`, lowercase, reject `x-*`
/// private-use tags, and map legacy ISO 639-1 `iw` primary subtags to `he`.
pub fn normalize_language_tag(tag: &str) -> String {
    let base = tag.trim().split(['.', '@']).next().unwrap_or_default().replace('_', "-").to_ascii_lowercase();
    let mut parts = base.split('-').filter(|part| !part.is_empty());
    let Some(primary) = parts.next() else {
        return String::new();
    };
    if primary == "x" {
        return String::new();
    }
    let normalized_primary = if primary == "iw" { HEBREW } else { primary };
    let mut out = String::from(normalized_primary);
    for part in parts {
        out.push('-');
        out.push_str(part);
    }
    out
}

/// Normalize and validate a saved or requested language preference (`system`, `en`, `he`, `iw`).
pub fn validate_preference(pref: &str) -> Result<String, String> {
    let trimmed = pref.trim();
    if trimmed.eq_ignore_ascii_case(SYSTEM) {
        return Ok(SYSTEM.into());
    }
    let normalized = normalize_language_tag(trimmed);
    let primary = normalized.split('-').next().unwrap_or_default();
    match primary {
        "en" => Ok(ENGLISH.into()),
        "he" => Ok(HEBREW.into()),
        _ => Err(format!("unknown language {pref:?} (expected system, en, or he)")),
    }
}

/// Resolve a stored/requested preference (`system`, `en`, `he`, `iw`) to an installed catalog id.
pub fn resolve_preference(pref: &str) -> &'static str {
    let trimmed = pref.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(SYSTEM) {
        return resolve_system_language();
    }
    match validate_preference(trimmed).as_deref() {
        Ok(HEBREW) => HEBREW,
        _ => ENGLISH,
    }
}

fn resolve_system_language() -> &'static str {
    if solidcraft_ui_text::system_language() == HEBREW { HEBREW } else { ENGLISH }
}

/// Install the catalogs and enter a scoped thread-local locale for `pref`.
pub fn scope(pref: &str) -> solidcraft_ui_text::LanguageScope {
    crate::theme::install_look();
    solidcraft_ui_text::scope(resolve_preference(pref))
}

/// Language options shown in Settings (resolved from the active catalog metadata).
pub fn selectable_languages() -> Vec<LanguageChoice> {
    crate::theme::install_look();
    vec![
        LanguageChoice { pref: SYSTEM, label: solidcraft_ui_text::tr("settings.language_system").to_owned() },
        LanguageChoice { pref: ENGLISH, label: solidcraft_ui_text::Locale::for_language(ENGLISH).display_name().into_owned() },
        LanguageChoice { pref: HEBREW, label: solidcraft_ui_text::Locale::for_language(HEBREW).display_name().into_owned() },
    ]
}

/// Build the Settings language dropdown choices and initial selection index for `saved_pref`.
pub fn language_choices_for(saved_pref: &str) -> (Vec<LanguageChoice>, usize) {
    let choices = selectable_languages();
    let current = validate_preference(saved_pref).unwrap_or_else(|_| ENGLISH.into());
    let idx = choices.iter().position(|c| c.pref == current).unwrap_or(0);
    (choices, idx)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use super::*;

    fn placeholder_names(text: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = text;
        while let Some((_, after)) = rest.split_once('{') {
            let Some((name, tail)) = after.split_once('}') else {
                break;
            };
            names.push(name.to_owned());
            rest = tail;
        }
        names.sort();
        names
    }

    #[test]
    fn legacy_iw_normalization_and_validation() {
        assert_eq!(normalize_language_tag("iw"), "he");
        assert_eq!(normalize_language_tag("iw-IL"), "he-il");
        assert_eq!(normalize_language_tag("iw_IL.UTF-8"), "he-il");
        assert_eq!(normalize_language_tag("he_IL@euro"), "he-il");
        assert_eq!(normalize_language_tag("x-he"), "");

        assert_eq!(validate_preference("system"), Ok("system".into()));
        assert_eq!(validate_preference("en-US"), Ok("en".into()));
        assert_eq!(validate_preference("he-IL"), Ok("he".into()));
        assert_eq!(validate_preference("iw_IL.UTF-8"), Ok("he".into()));
        assert!(validate_preference("x-he").is_err());
        assert!(validate_preference("xx").is_err());
    }

    #[test]
    fn hebrew_catalog_covers_all_english_and_workspace_keys() {
        let en: HashMap<String, String> = serde_json::from_str(include_str!("../locales/en.json")).expect("valid en.json");
        let he: HashMap<String, String> = serde_json::from_str(include_str!("../locales/he.json")).expect("valid he.json");

        let mut expected_keys: BTreeSet<String> = en.keys().cloned().collect();
        for w in fieldcraft_engine::catalogue::Workspace::ALL {
            let (about, tabs) = crate::toolbar::workspace_ids(w);
            expected_keys.insert(about.to_owned());
            expected_keys.insert(tabs.to_owned());
        }

        let missing: Vec<&String> = expected_keys.iter().filter(|k| !he.contains_key(k.as_str())).collect();
        assert!(missing.is_empty(), "missing Hebrew translations in locales/he.json: {missing:?}");

        for (key, he_val) in &he {
            assert!(!he_val.trim().is_empty(), "{key} has empty Hebrew translation");
            assert!(!he_val.chars().any(|c| matches!(c, '\u{200E}' | '\u{200F}')), "{key} contains stray directional mark in {he_val:?}");
            if let Some(en_val) = en.get(key) {
                assert_eq!(placeholder_names(en_val), placeholder_names(he_val), "placeholder mismatch for {key}");
                if !matches!(key.as_str(), "settings.group_app" | "settings.updates_skipped_version" | "links.discord_label" | "links.discord_name") {
                    assert_ne!(en_val, he_val, "{key} was left untranslated in locales/he.json");
                }
            }
        }
    }

    #[test]
    fn scoped_hebrew_translation_and_catalog_display_names() {
        crate::theme::install_look();
        assert_eq!(resolve_preference("iw-IL"), "he");
        {
            let _guard = scope("iw");
            assert_eq!(solidcraft_ui_text::language(), "he");
            assert_ne!(solidcraft_ui_text::tr("settings.title"), "Settings");
            assert_ne!(solidcraft_ui_text::tr("settings.language"), "Language");
            assert_ne!(solidcraft_ui_text::tr("workspace.prepare.about"), "Open geometry and mesh it");
            let formatted = solidcraft_ui_text::format("settings.updates_skipped_version", &[("version", "0.8.1")]);
            assert!(formatted.contains("0.8.1") && formatted.contains("FieldCraft"), "got {formatted:?}");

            let (choices, initial) = language_choices_for("iw-IL");
            assert_eq!(choices[0].pref, SYSTEM);
            assert_eq!(choices[1].label, "English");
            assert_eq!(choices[2].label, "עברית");
            assert_eq!(initial, 2);
        }
    }
}
