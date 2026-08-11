use chrono::{Datelike, Local};
use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;

use crate::config::MawaqitCache;
use crate::i18n::tr;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(crate::USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client")
    })
}

fn normalize_mawaqit_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Invalid Mawaqit URL".to_string());
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    };
    let url = reqwest::Url::parse(&with_scheme).map_err(|_| "Invalid Mawaqit URL".to_string())?;
    let host = url.host_str().unwrap_or_default();
    if host != "mawaqit.net" && !host.ends_with(".mawaqit.net") {
        return Err("Invalid Mawaqit URL".to_string());
    }
    let mut sanitized = url.clone();
    sanitized.set_fragment(None);
    sanitized.set_query(None);
    Ok(sanitized.to_string())
}

fn extract_object_candidates(html: &str) -> Vec<usize> {
    let mut idxs = Vec::new();
    let mut offset = 0;
    while let Some(pos) = html[offset..].find("\"times\":[") {
        idxs.push(offset + pos);
        offset += pos + 7;
    }
    idxs
}

fn extract_braced_json_from(html: &str, near: usize) -> Option<String> {
    let bytes = html.as_bytes();
    let start_limit = near.saturating_sub(8000);
    for start in (start_limit..=near).rev() {
        if bytes.get(start) != Some(&b'{') {
            continue;
        }
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escape = false;
        for end in start..bytes.len() {
            let current = bytes[end] as char;
            if in_str {
                if escape {
                    escape = false;
                } else if current == '\\' {
                    escape = true;
                } else if current == '"' {
                    in_str = false;
                }
                continue;
            }
            match current {
                '"' => in_str = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let snippet = &html[start..=end];
                        if let Ok(json) = serde_json::from_str::<Value>(snippet)
                            && json.get("times").is_some()
                            && json.get("shuruq").is_some()
                            && json.get("calendar").is_some()
                        {
                            return Some(snippet.to_string());
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn parse_cache_from_value(url: &str, json: &Value) -> Result<MawaqitCache, String> {
    let times = json
        .get("times")
        .and_then(|arr| arr.as_array())
        .ok_or_else(|| "Mawaqit fetch failed".to_string())?;
    if times.len() != 5 {
        return Err("Mawaqit fetch failed".to_string());
    }
    let shuruq = json
        .get("shuruq")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Mawaqit fetch failed".to_string())?;
    let timezone = json
        .get("timezone")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let mosque_name = json
        .get("name")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            json.get("label")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        });
    let latitude = json
        .get("latitude")
        .and_then(|number| number.as_f64())
        .or_else(|| json.get("lat").and_then(|number| number.as_f64()));
    let longitude = json
        .get("longitude")
        .and_then(|number| number.as_f64())
        .or_else(|| json.get("lng").and_then(|number| number.as_f64()))
        .or_else(|| json.get("lon").and_then(|number| number.as_f64()));
    let country_code = json
        .get("countryCode")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    let mut months: Vec<std::collections::BTreeMap<u32, [String; 6]>> = Vec::new();
    if let Some(calendar) = json.get("calendar").and_then(|value| value.as_array()) {
        for month_obj in calendar {
            let mut month_map = std::collections::BTreeMap::new();
            if let Some(obj) = month_obj.as_object() {
                for (day, arr) in obj {
                    let Ok(day_num) = day.parse::<u32>() else {
                        continue;
                    };
                    let Some(vals) = arr.as_array() else {
                        continue;
                    };
                    if vals.len() != 6 {
                        continue;
                    }
                    let mut out: [String; 6] = Default::default();
                    let mut ok = true;
                    for (slot_index, value) in vals.iter().enumerate() {
                        if let Some(str_val) = value.as_str() {
                            out[slot_index] = str_val.to_string();
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        month_map.insert(day_num, out);
                    }
                }
            }
            months.push(month_map);
        }
    }

    if months.len() != 12 {
        months = vec![std::collections::BTreeMap::new(); 12];
    }

    let now = Local::now();
    let year = now.year();
    let fetched_on = now.date_naive().to_string();

    let month_idx = now.month0() as usize;
    let day = now.day();
    if months
        .get(month_idx)
        .is_some_and(|month_map| !month_map.contains_key(&day))
    {
        let fajr = times[0].as_str().unwrap_or_default().to_string();
        let dhuhr = times[1].as_str().unwrap_or_default().to_string();
        let asr = times[2].as_str().unwrap_or_default().to_string();
        let maghrib = times[3].as_str().unwrap_or_default().to_string();
        let isha = times[4].as_str().unwrap_or_default().to_string();
        months[month_idx].insert(day, [fajr, shuruq.to_string(), dhuhr, asr, maghrib, isha]);
    }

    Ok(MawaqitCache {
        url: url.to_string(),
        mosque_name,
        timezone,
        latitude,
        longitude,
        country_code,
        year,
        months,
        fetched_on,
    })
}

pub async fn fetch_mawaqit_cache(raw_url: &str) -> Result<MawaqitCache, String> {
    // Mawaqit error strings — expose to xgettext
    if false {
        let _ = tr("Invalid Mawaqit URL");
        let _ = tr("Mawaqit fetch failed");
    }
    let url = normalize_mawaqit_url(raw_url)?;
    let html = client()
        .get(&url)
        .send()
        .await
        .map_err(|_| "Mawaqit fetch failed".to_string())?
        .text()
        .await
        .map_err(|_| "Mawaqit fetch failed".to_string())?;

    for idx in extract_object_candidates(&html) {
        if let Some(snippet) = extract_braced_json_from(&html, idx)
            && let Ok(json) = serde_json::from_str::<Value>(&snippet)
        {
            return parse_cache_from_value(&url, &json);
        }
    }

    Err("Mawaqit fetch failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_payload_from_embedded_json() {
        let html = r#"
        <html><body>
        {"times":["05:44","13:58","17:32","20:30","21:57"],"shuruq":"07:20","timezone":"Europe/Paris","calendar":[{"1":["07:05","08:44","12:59","14:48","17:08","18:35"]},{"1":["05:44","07:20","13:58","17:32","20:30","21:57"]}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}]}
        </body></html>
        "#;
        let idx = html.find("\"times\":[").unwrap();
        let json = extract_braced_json_from(html, idx).expect("extract json");
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        let cache = parse_cache_from_value("https://mawaqit.net/x", &parsed).unwrap();
        assert_eq!(cache.url, "https://mawaqit.net/x");
        assert_eq!(cache.timezone.as_deref(), Some("Europe/Paris"));
        assert_eq!(cache.months.len(), 12);
    }
}
