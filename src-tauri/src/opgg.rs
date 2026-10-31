//! Deneysel OP.GG MCP istemcisi.
//!
//! Bu modül varsayılan olarak kapalıdır ve yalnızca kullanıcı ayarlardan
//! açtığında çalışır. Yalnızca şampiyon adı + pozisyon gönderilir; Riot ID,
//! PUUID veya hesap bilgisi gönderilmez. Yanıtlar 24 saat yerelde cache'lenir.

use std::time::Duration;

use serde_json::{Value, json};

use crate::db::Database;
use crate::models::MetaReport;

const MCP_URL: &str = "https://mcp-api.op.gg/mcp";
const CACHE_TTL_SECS: i64 = 86_400;
const REQUEST_TIMEOUT_SECS: u64 = 14;

const POSITIONS: [&str; 5] = ["top", "jungle", "mid", "adc", "support"];

pub async fn fetch(db: &Database, champion: &str, position: &str) -> Result<MetaReport, String> {
    let champion = champion.trim().to_uppercase();
    let position = position.trim().to_lowercase();

    if champion.is_empty()
        || champion.len() > 32
        || !champion
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err("Geçersiz şampiyon adı.".to_string());
    }

    if !POSITIONS.contains(&position.as_str()) {
        return Err("Geçersiz pozisyon.".to_string());
    }

    let cache_key = format!("opgg:v1:{}:{}", champion, position);
    if let Some(cached) = db.kv_get(&cache_key, CACHE_TTL_SECS) {
        if let Ok(mut report) = serde_json::from_str::<MetaReport>(&cached) {
            report.cached = true;
            return Ok(report);
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .gzip(true)
        .build()
        .map_err(|error| format!("OP.GG istemcisi oluşturulamadı: {error}"))?;

    let session = initialize(&client).await?;
    let text = call_champion_analysis(&client, &session, &champion, &position).await?;
    let mut report = parse_report(&champion, &position, &text)?;
    report.cached = false;

    if let Ok(serialized) = serde_json::to_string(&report) {
        let _ = db.kv_set(&cache_key, &serialized);
    }

    Ok(report)
}

struct Session {
    id: String,
}

async fn initialize(client: &reqwest::Client) -> Result<Session, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": { "name": "tlas", "version": env!("CARGO_PKG_VERSION") }
        }
    });

    let response = client
        .post(MCP_URL)
        .header("Accept", "application/json, text/event-stream")
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("OP.GG'ye ulaşılamadı: {error}"))?;

    let id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(String::from)
        .ok_or_else(|| "OP.GG oturumu açılamadı.".to_string())?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("OP.GG yanıtı okunamadı: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "OP.GG oturumu açılamadı (HTTP {}).",
            status.as_u16()
        ));
    }

    let value = parse_body(&text)?;
    if let Some(error) = value.get("error") {
        return Err(format!("OP.GG hatası: {}", compact(error)));
    }

    Ok(Session { id })
}

async fn call_champion_analysis(
    client: &reqwest::Client,
    session: &Session,
    champion: &str,
    position: &str,
) -> Result<String, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "lol_get_champion_analysis",
            "arguments": {
                "champion": champion,
                "position": position,
                "game_mode": "ranked",
                "lang": "tr_TR",
                "desired_output_fields": [
                    "champion",
                    "data.summary.average_stats.win_rate",
                    "data.summary.average_stats.pick_rate",
                    "data.summary.average_stats.ban_rate",
                    "data.summary.average_stats.tier",
                    "data.core_items.ids_names[]",
                    "data.core_items.win",
                    "data.core_items.pick_rate",
                    "data.runes.primary_page_name",
                    "data.runes.win",
                    "data.runes.pick_rate"
                ]
            }
        }
    });

    let response = client
        .post(MCP_URL)
        .header("Accept", "application/json, text/event-stream")
        .header("Mcp-Session-Id", &session.id)
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("OP.GG isteği başarısız: {error}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("OP.GG yanıtı okunamadı: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "OP.GG isteği başarısız (HTTP {}).",
            status.as_u16()
        ));
    }

    let value = parse_body(&text)?;
    if let Some(error) = value.get("error") {
        return Err(format!("OP.GG hatası: {}", compact(error)));
    }

    value
        .pointer("/result/content/0/text")
        .and_then(|value| value.as_str())
        .map(String::from)
        .ok_or_else(|| "OP.GG yanıtı boş.".to_string())
}

/// MCP sunucusu JSON veya SSE döndürebilir; ikisini de kabul et.
fn parse_body(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();

    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed)
            .map_err(|error| format!("OP.GG yanıtı çözülemedi: {error}"));
    }

    for line in trimmed.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(data) {
            return Ok(value);
        }
    }

    Err("OP.GG yanıtı çözülemedi.".to_string())
}

fn compact(value: &Value) -> String {
    value
        .get("message")
        .and_then(|value| value.as_str())
        .map(String::from)
        .unwrap_or_else(|| value.to_string())
}

fn parse_report(champion: &str, position: &str, text: &str) -> Result<MetaReport, String> {
    let stats = find_call(text, "AverageStats").and_then(parse_average_stats);
    let core_items = find_call(text, "CoreItems")
        .map(parse_string_array)
        .unwrap_or_default();
    let rune = find_call(text, "Runes").and_then(parse_first_string);

    if stats.is_none() && core_items.is_empty() && rune.is_none() {
        return Err("OP.GG yanıtı beklenen biçimde değil.".to_string());
    }

    let (winrate, pickrate, banrate, tier) = match stats {
        Some((winrate, pickrate, banrate, tier)) => {
            (Some(winrate), Some(pickrate), Some(banrate), Some(tier))
        }
        None => (None, None, None, None),
    };

    Ok(MetaReport {
        champion: champion.to_string(),
        position: position.to_string(),
        tier,
        winrate,
        pickrate,
        banrate,
        core_items,
        rune,
        cached: false,
        fetched_at: chrono::Utc::now().timestamp(),
    })
}

fn parse_average_stats(content: &str) -> Option<(f64, f64, f64, i64)> {
    let parts = split_top_level(content);
    if parts.len() < 4 {
        return None;
    }

    let winrate = parse_number(parts[0])? * 100.0;
    let pickrate = parse_number(parts[1])? * 100.0;
    let banrate = parse_number(parts[2])? * 100.0;
    let tier = parse_number(parts[3])? as i64;

    Some((round2(winrate), round2(pickrate), round2(banrate), tier))
}

fn parse_string_array(content: &str) -> Vec<String> {
    let Some(start) = content.find('[') else {
        return Vec::new();
    };
    let Some(end) = content[start..].find(']').map(|offset| start + offset) else {
        return Vec::new();
    };

    parse_quoted(&content[start + 1..end])
}

fn parse_first_string(content: &str) -> Option<String> {
    parse_quoted(content).into_iter().next()
}

fn parse_quoted(content: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }

        let mut value = String::new();
        let mut escaped = false;

        for next in chars.by_ref() {
            if escaped {
                value.push(next);
                escaped = false;
            } else if next == '\\' {
                escaped = true;
            } else if next == '"' {
                break;
            } else {
                value.push(next);
            }
        }

        values.push(value);
    }

    values
}

fn parse_number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

/// `Name(...)` çağrısının içeriğini, iç içe parantezleri ve tırnakları
/// hesaba katarak döndürür.
fn find_call<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{}(", name);
    let start = text.find(&needle)? + needle.len();
    let bytes = text.as_bytes();

    let mut depth = 1usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, byte) in bytes[start..].iter().enumerate() {
        match *byte {
            b'"' if !escaped => in_string = !in_string,
            b'\\' if in_string => escaped = !escaped,
            b'(' if !in_string => depth += 1,
            b')' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + offset]);
                }
            }
            _ => {}
        }

        if *byte != b'\\' {
            escaped = false;
        }
    }

    None
}

fn split_top_level(content: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in content.bytes().enumerate() {
        match byte {
            b'"' if !escaped => in_string = !in_string,
            b'\\' if in_string => escaped = !escaped,
            b'[' | b'(' | b'{' if !in_string => depth += 1,
            b']' | b')' | b'}' if !in_string => depth -= 1,
            b',' if !in_string && depth == 0 => {
                parts.push(content[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }

        if byte != b'\\' {
            escaped = false;
        }
    }

    parts.push(content[start..].trim());
    parts
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "LolGetChampionAnalysis(\"Kayn\",Data(Summary(AverageStats(0.5,0.08,0.06,2)),CoreItems([\"Elektrikli Devrikılıcı\",\"Kara Balta\",\"Ölümün Dansı\"],3732,0.08),Runes(\"Hâkimiyet\",12917,0.24)))";

    #[test]
    fn parses_champion_analysis() {
        let report = parse_report("KAYN", "jungle", SAMPLE).expect("parse");

        assert_eq!(report.tier, Some(2));
        assert_eq!(report.winrate, Some(50.0));
        assert_eq!(report.pickrate, Some(8.0));
        assert_eq!(report.banrate, Some(6.0));
        assert_eq!(report.core_items.len(), 3);
        assert_eq!(report.core_items[0], "Elektrikli Devrikılıcı");
        assert_eq!(report.rune.as_deref(), Some("Hâkimiyet"));
    }

    #[test]
    fn splits_nested_arguments() {
        let parts = split_top_level("\"A,B\",1,[2,3],4");
        assert_eq!(parts, vec!["\"A,B\"", "1", "[2,3]", "4"]);
    }

    #[test]
    fn rejects_unknown_body() {
        assert!(parse_report("KAYN", "jungle", "boş yanıt").is_err());
    }
}
