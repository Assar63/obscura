//! Firmware update check: compares the camera's firmware version with the
//! latest one on OBSBOT's download page for the model. Only ever checks;
//! flashing is deliberately out of scope (see README).
//!
//! OBSBOT has no public "latest version" API, so this reads the download
//! page, whose embedded Nuxt state lists each model's firmware as
//! `<key>:{firmware:{… version:"v6.6.11.1" …}}`. If OBSBOT changes the page
//! the check reports that it couldn't find a version rather than guessing.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

/// Where a model's latest firmware is published (`[firmware]` in a profile).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FirmwareSource {
    /// OBSBOT download page for the model's series.
    pub page: String,
    /// The model's key in the page's data, e.g. `tiny3`.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateCheck {
    /// Firmware on the camera.
    pub current: String,
    /// Latest firmware on OBSBOT's download page.
    pub latest: String,
    /// Whether `latest` is newer than `current`.
    pub update_available: bool,
    /// The download page, for the user to follow.
    pub page: String,
}

/// The latest firmware version for `key` in a download page, without the
/// leading `v`.
pub fn latest_version(html: &str, key: &str) -> Option<String> {
    let start = html.find(&format!("{key}:{{firmware:{{"))?;
    let entry = &html[start..];
    let entry = &entry[..entry.find("}}").unwrap_or(entry.len())];
    let v = entry.split("version:\"").nth(1)?;
    let v = v[..v.find('"')?].trim_start_matches('v');
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.')).then(|| v.to_string())
}

/// Compares dotted versions numerically (`6.6.11.1` > `6.6.8.3`).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(a), parse(b));
    for i in 0..a.len().max(b.len()) {
        match a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

/// Compares `current` with the version found in `html`.
pub fn check_page(source: &FirmwareSource, html: &str, current: &str) -> Option<UpdateCheck> {
    let latest = latest_version(html, &source.key)?;
    Some(UpdateCheck {
        update_available: compare_versions(&latest, current) == Ordering::Greater,
        current: current.to_string(),
        latest,
        page: source.page.clone(),
    })
}

/// Fetches the download page and compares. Needs network access; the
/// error is a sentence for the user.
#[cfg(feature = "update-check")]
pub fn check(source: &FirmwareSource, current: &str) -> Result<UpdateCheck, String> {
    let html = ureq::get(&source.page)
        .header("User-Agent", concat!("OBSCura/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("Couldn't reach OBSBOT's download page: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Couldn't read OBSBOT's download page: {e}"))?;
    let result = check_page(source, &html, current).ok_or_else(|| {
        format!(
            "OBSBOT's download page didn't list a firmware version for `{}`; \
             the page may have changed",
            source.key
        )
    })?;
    crate::log::info(format!(
        "Firmware check: camera {}, latest {}{}",
        result.current,
        result.latest,
        if result.update_available {
            " (update available)"
        } else {
            ""
        }
    ));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Excerpt of https://www.obsbot.com/download/obsbot-tiny-3-series
    // (2026-10-04).
    const PAGE: &str = r#"tiny2lite:{firmware:{updateAt:bl,url:"https://resource-cdn.obsbothk.com/firmware/obsbot-tiny-2-lite/Obsbot_ty2lite_OA_E_PW103_6.2.8.1_release.bin",version:"v6.2.8.1",descs:{__EMPTY:"Tiny 2 Lite<br>Firmware",en:bm}}},tiny3:{firmware:{updateAt:bC,url:"https://resource-cdn.obsbothk.com/download/common/Obsbot_tiny3_OA_E_PW106_6.6.11.1_release.bin",version:"v6.6.11.1",descs:{__EMPTY:bh,en:U}}},tiny3lite:{firmware:{updateAt:bC,url:"x",version:"v6.5.11.1",descs:{}}}"#;

    #[test]
    fn finds_each_models_version() {
        assert_eq!(latest_version(PAGE, "tiny3").as_deref(), Some("6.6.11.1"));
        assert_eq!(
            latest_version(PAGE, "tiny3lite").as_deref(),
            Some("6.5.11.1")
        );
        assert_eq!(
            latest_version(PAGE, "tiny2lite").as_deref(),
            Some("6.2.8.1")
        );
        assert_eq!(latest_version(PAGE, "meet2"), None);
        assert_eq!(latest_version("<html></html>", "tiny3"), None);
    }

    #[test]
    fn compares_numerically() {
        assert_eq!(compare_versions("6.6.11.1", "6.6.8.3"), Ordering::Greater);
        assert_eq!(compare_versions("v6.6.8.3", "6.6.8.3"), Ordering::Equal);
        assert_eq!(compare_versions("6.6.8", "6.6.8.1"), Ordering::Less);
    }

    #[test]
    fn reports_an_update() {
        let source = FirmwareSource {
            page: "https://example.invalid".into(),
            key: "tiny3".into(),
        };
        let c = check_page(&source, PAGE, "6.6.8.3").unwrap();
        assert!(c.update_available);
        assert_eq!(c.latest, "6.6.11.1");
        assert!(
            !check_page(&source, PAGE, "6.6.11.1")
                .unwrap()
                .update_available
        );
    }
}
