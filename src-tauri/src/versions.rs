use serde::Serialize;
use serde_json::Value;

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Serialize)]
pub struct McVersion { pub id: String, pub kind: String, pub release_time: String }

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("BlockPilot/0.1.0")
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(20))
        .build().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_mc_versions(include_snapshots: bool) -> Result<Vec<McVersion>, String> {
    let resp: Value = client()?.get(VERSION_MANIFEST).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let versions = resp.get("versions").and_then(Value::as_array).ok_or("Malformed version manifest")?;
    let out = versions.iter().filter_map(|v| {
        let kind = v.get("type").and_then(Value::as_str)?.to_string();
        if !include_snapshots && kind != "release" { return None; }
        Some(McVersion {
            id: v.get("id").and_then(Value::as_str)?.to_string(),
            kind,
            release_time: v.get("releaseTime").and_then(Value::as_str).unwrap_or("").to_string(),
        })
    }).collect();
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct FabricLoaderVersion { pub version: String, pub stable: bool }

#[tauri::command]
pub async fn list_fabric_loaders(mc_version: String) -> Result<Vec<FabricLoaderVersion>, String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}", mc_version);
    let resp: Value = client()?.get(&url).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| format!("Fabric has no loader builds for Minecraft {} ({})", mc_version, e))?
        .json().await.map_err(|e| e.to_string())?;
    let arr = resp.as_array().ok_or("Malformed Fabric loader response")?;
    let out = arr.iter().filter_map(|entry| {
        let loader = entry.get("loader")?;
        Some(FabricLoaderVersion {
            version: loader.get("version").and_then(Value::as_str)?.to_string(),
            stable: loader.get("stable").and_then(Value::as_bool).unwrap_or(false),
        })
    }).collect();
    Ok(out)
}
