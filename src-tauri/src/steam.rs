use crate::models::{DetectedGame, GameDetails, GameSummary};
use reqwest::blocking::Client;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, time::Duration};
use walkdir::WalkDir;

fn fetch_json(url: &str) -> Result<Value, String> {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| format!("Не удалось подготовить сетевой клиент: {error}"))?
        .get(url)
        .header("User-Agent", "Mozilla/5.0 CrocodileGenaStudio/3.0")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("Steam Store API не ответил: {error}"))?
        .json::<Value>()
        .map_err(|error| format!("Steam Store вернул некорректные данные: {error}"))
}

fn encode_query(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (*byte as char).to_string()
            }
            b' ' => "%20".to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn clean_query(query: &str) -> String {
    query
        .chars()
        .filter(|character| {
            character.is_alphanumeric()
                || character.is_whitespace()
                || *character == '-'
                || *character == '_'
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn search_game_inner(query: &str) -> Result<Option<GameSummary>, String> {
    let clean = clean_query(query);
    if clean.chars().count() < 2 {
        return Ok(None);
    }
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
        encode_query(&clean)
    );
    let json = fetch_json(&url)?;
    let Some(item) = json
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return Ok(None);
    };
    let Some(appid) = item.get("id").and_then(Value::as_u64) else {
        return Ok(None);
    };
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unknown game")
        .to_string();
    Ok(Some(GameSummary {
        appid,
        name,
        header_image: format!(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{appid}/header.jpg"
        ),
    }))
}

fn details_payload(appid: u64) -> Result<Option<Value>, String> {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={appid}&l=english");
    let json = fetch_json(&url)?;
    let Some(entry) = json.get(appid.to_string()) else {
        return Ok(None);
    };
    if !entry
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    Ok(entry.get("data").cloned())
}

fn get_dlc_names(ids: &[u64]) -> BTreeMap<String, String> {
    let mut dlcs = BTreeMap::new();
    for id in ids.iter().take(25) {
        let name = details_payload(*id)
            .ok()
            .flatten()
            .and_then(|data| data.get("name").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| format!("DLC {id}"));
        dlcs.insert(id.to_string(), name);
    }
    for id in ids.iter().skip(25) {
        dlcs.insert(id.to_string(), format!("DLC {id}"));
    }
    dlcs
}

pub(crate) fn get_app_details_inner(appid: u64) -> Result<Option<GameDetails>, String> {
    let Some(data) = details_payload(appid)? else {
        return Ok(None);
    };
    let ids = data
        .get("dlc")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(Some(GameDetails {
        appid,
        name: data
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown game")
            .to_string(),
        header_image: data
            .get("header_image")
            .and_then(Value::as_str)
            .map(str::to_string),
        short_description: data
            .get("short_description")
            .and_then(Value::as_str)
            .map(str::to_string),
        dlcs: get_dlc_names(&ids),
    }))
}

#[tauri::command]
pub fn search_game(query: String) -> Result<Option<GameSummary>, String> {
    search_game_inner(&query)
}

#[tauri::command]
pub fn get_app_details(appid: u64) -> Result<Option<GameDetails>, String> {
    get_app_details_inner(appid)
}

pub fn auto_detect_from_path(game_path: &Path) -> DetectedGame {
    if !game_path.is_dir() {
        return DetectedGame::default();
    }

    for entry in WalkDir::new(game_path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("steam_appid.txt")
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if let Some(appid) = content
                    .split(|c: char| !c.is_ascii_digit())
                    .find_map(|part| part.parse::<u64>().ok())
                {
                    if let Ok(Some(details)) = get_app_details_inner(appid) {
                        return DetectedGame {
                            detected: true,
                            appid: Some(appid),
                            name: Some(details.name),
                            header_image: details.header_image,
                            dlcs: details.dlcs,
                            source: Some("steam_appid.txt".into()),
                        };
                    }
                }
            }
        }
    }

    let mut queries = Vec::new();
    if let Some(folder) = game_path.file_name().and_then(|name| name.to_str()) {
        queries.push(folder.to_string());
    }
    for entry in WalkDir::new(game_path)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        let lower = name.to_lowercase();
        if lower.ends_with(".exe")
            && !["unins", "crash", "setup", "launcher", "dxsetup", "vcredist"]
                .iter()
                .any(|skip| lower.contains(skip))
        {
            if let Some(stem) = entry.path().file_stem().and_then(|value| value.to_str()) {
                queries.push(stem.to_string());
            }
        }
    }

    queries.sort();
    queries.dedup();
    for query in queries.into_iter().take(8) {
        if let Ok(Some(summary)) = search_game_inner(&query) {
            let details = get_app_details_inner(summary.appid).ok().flatten();
            return DetectedGame {
                detected: true,
                appid: Some(summary.appid),
                name: Some(summary.name),
                header_image: details
                    .as_ref()
                    .and_then(|item| item.header_image.clone())
                    .or(Some(summary.header_image)),
                dlcs: details.map(|item| item.dlcs).unwrap_or_default(),
                source: Some(format!("Steam Store · {query}")),
            };
        }
    }
    DetectedGame::default()
}
