use std::{fs, path::PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct ModResult { pub project_id: String, pub title: String, pub description: String, pub downloads: u64, pub icon_url: Option<String> }

#[tauri::command]
pub async fn search_mods(query: String, loader: Option<String>, version: Option<String>) -> Result<Vec<ModResult>, String> {
    let client = reqwest::Client::builder().user_agent("BlockPilot/0.1.0").build().map_err(|e| e.to_string())?;
    let mut facets: Vec<Vec<String>> = vec![vec!["project_type:mod".into()]];
    if let Some(l) = &loader { if !l.is_empty() { facets.push(vec![format!("categories:{}", l.to_lowercase())]); } }
    if let Some(v) = &version { if !v.is_empty() { facets.push(vec![format!("versions:{}", v)]); } }
    let facets_json = serde_json::to_string(&facets).map_err(|e| e.to_string())?;
    let url = format!("https://api.modrinth.com/v2/search?query={}&facets={}&limit=20", urlencoding_light(&query), urlencoding_light(&facets_json));
    let resp: Value = client.get(&url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let hits = resp.get("hits").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(hits.into_iter().map(|h| ModResult {
        project_id: h.get("project_id").and_then(Value::as_str).unwrap_or("").to_string(),
        title: h.get("title").and_then(Value::as_str).unwrap_or("Unknown").to_string(),
        description: h.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
        downloads: h.get("downloads").and_then(Value::as_u64).unwrap_or(0),
        icon_url: h.get("icon_url").and_then(Value::as_str).map(String::from),
    }).collect())
}

#[tauri::command]
pub async fn install_mod(app: tauri::AppHandle, instance: String, project_id: String, game_version: String, loader: String) -> Result<String, String> {
    let client = reqwest::Client::builder().user_agent("BlockPilot/0.1.0").build().map_err(|e| e.to_string())?;
    let url = format!("https://api.modrinth.com/v2/project/{}/version?loaders=[\"{}\"]&game_versions=[\"{}\"]", project_id, loader, game_version);
    let versions: Value = client.get(&url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let first = versions.as_array().and_then(|a| a.first()).ok_or("No compatible mod version found for this Minecraft/loader combo")?;
    let file = first.get("files").and_then(Value::as_array).and_then(|f| f.first()).ok_or("Mod version has no downloadable file")?;
    let file_url = file.get("url").and_then(Value::as_str).ok_or("Missing file URL")?;
    let file_name = file.get("filename").and_then(Value::as_str).ok_or("Missing filename")?;
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let safe_instance: String = instance.chars().map(|c| if c.is_ascii_alphanumeric() || c=='-' || c=='_' {c} else {'_'}).collect();
    let mods_dir: PathBuf = root.join("instances").join(safe_instance).join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    let bytes = client.get(file_url).send().await.map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    fs::write(mods_dir.join(file_name), &bytes).map_err(|e| e.to_string())?;
    Ok(format!("Installed {}", file_name))
}

fn urlencoding_light(s: &str) -> String {
    s.chars().map(|c| match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        _ => format!("%{:02X}", c as u32),
    }).collect()
}
