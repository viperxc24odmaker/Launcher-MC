use std::{fs, io::Write, path::PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::Manager;
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceSummary { pub name: String, pub path: String, pub has_game: bool, pub version: String, pub loader: String, pub loader_version: Option<String> }

fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("instances");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn safe(value: &str) -> String { value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect() }

pub fn read_profile(path: &PathBuf) -> (String, String, Option<String>) {
    let profile_path = path.join("profile.json");
    let Ok(bytes) = fs::read(&profile_path) else { return ("1.21.11".into(), "vanilla".into(), None); };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return ("1.21.11".into(), "vanilla".into(), None); };
    let version = json.get("minecraft").and_then(|v| v.as_str()).unwrap_or("1.21.11").to_string();
    let loader = json.get("loader").and_then(|v| v.as_str()).unwrap_or("vanilla").to_string();
    let loader_version = json.get("loader_version").and_then(|v| v.as_str()).map(String::from);
    (version, loader, loader_version)
}

#[tauri::command]
pub fn list_instances(app: tauri::AppHandle) -> Result<Vec<InstanceSummary>, String> {
    let dir = root(&app)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
        let path = entry.path();
        let (version, loader, loader_version) = read_profile(&path);
        result.push(InstanceSummary { name: entry.file_name().to_string_lossy().to_string(), path: path.to_string_lossy().to_string(), has_game: path.join("game").exists(), version, loader, loader_version });
    }
    result.sort_by(|a,b| a.name.cmp(&b.name));
    Ok(result)
}

#[tauri::command]
pub fn create_instance(app: tauri::AppHandle, name: String, version: String, loader: String, loader_version: Option<String>) -> Result<InstanceSummary, String> {
    if name.trim().is_empty() { return Err("Instance name cannot be empty".into()); }
    if version.trim().is_empty() { return Err("Pick a Minecraft version".into()); }
    let loader = if loader.trim().is_empty() { "vanilla".to_string() } else { loader };
    let path = root(&app)?.join(safe(&name));
    if path.exists() { return Err("An instance with that name already exists".into()); }
    fs::create_dir_all(path.join("game")).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("mods")).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("resourcepacks")).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("saves")).map_err(|e| e.to_string())?;
    fs::write(path.join("profile.json"), serde_json::to_vec_pretty(&json!({"name": name, "minecraft": version, "loader": loader, "loader_version": loader_version, "ram_mb": 4096, "jvm_args": []})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(InstanceSummary { name: safe(&name), path: path.to_string_lossy().to_string(), has_game: true, version, loader, loader_version })
}

#[tauri::command]
pub fn snapshot_instance(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let instance = root(&app)?.join(safe(&name));
    if !instance.exists() { return Err("Instance does not exist".into()); }
    let snapshot_dir = instance.join("snapshots");
    fs::create_dir_all(&snapshot_dir).map_err(|e| e.to_string())?;
    let stamp = format!("{}", chrono_free_timestamp());
    let archive_path = snapshot_dir.join(format!("snapshot-{}.zip", stamp));
    let file = fs::File::create(&archive_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    for entry in WalkDir::new(&instance).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.starts_with(&snapshot_dir) || path == instance { continue; }
        let rel = path.strip_prefix(&instance).map_err(|e| e.to_string())?;
        if path.is_file() {
            zip.start_file(rel.to_string_lossy().replace('\\', "/"), options).map_err(|e| e.to_string())?;
            let data = fs::read(path).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(archive_path.to_string_lossy().to_string())
}

fn chrono_free_timestamp() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

#[tauri::command]
pub fn delete_instance(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let instance = root(&app)?.join(safe(&name));
    if !instance.exists() { return Err("Instance does not exist".into()); }
    fs::remove_dir_all(&instance).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_resource_packs(app: tauri::AppHandle, instance: String) -> Result<Vec<String>, String> {
    let dir = root(&app)?.join(safe(&instance)).join("resourcepacks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "zip" {
            out.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
pub fn import_resource_pack(app: tauri::AppHandle, instance: String, file_path: String) -> Result<String, String> {
    let src = PathBuf::from(&file_path);
    if !src.exists() || src.extension().and_then(|e| e.to_str()) != Some("zip") {
        return Err("Pick a .zip resource pack file".into());
    }
    let dir = root(&app)?.join(safe(&instance)).join("resourcepacks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let filename = src.file_name().ok_or("Invalid file name")?;
    let dest = dir.join(filename);
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(filename.to_string_lossy().to_string())
}

#[tauri::command]
pub fn remove_resource_pack(app: tauri::AppHandle, instance: String, filename: String) -> Result<(), String> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') { return Err("Invalid filename".into()); }
    let path = root(&app)?.join(safe(&instance)).join("resourcepacks").join(&filename);
    if path.exists() { fs::remove_file(path).map_err(|e| e.to_string())?; }
    Ok(())
}

#[tauri::command]
pub fn analyze_crash(log: String) -> Result<String, String> {
    let lower = log.to_lowercase();
    let diagnosis = if lower.contains("outofmemoryerror") || lower.contains("java heap space") { "Java ran out of heap memory. Increase the instance RAM or remove heavy mods." }
    else if lower.contains("nosuchmethoderror") || lower.contains("noclassdeffounderror") { "A mod/library mismatch is likely. Check the mod loader and dependency versions." }
    else if lower.contains("invalidsession") || lower.contains("authentication") { "Minecraft authentication failed. Sign in again or use the offline fallback for single-player." }
    else if lower.contains("unsupportedclassversion") { "The selected Java runtime is incompatible with this Minecraft version. Minecraft 1.21.x requires Java 21." }
    else if lower.contains("mixin") { "A mixin failed during mod initialization. Check the first mod named above the Mixin failure and its dependencies." }
    else { "No single failure signature was detected. Open the full latest.log and inspect the first ERROR/Exception before the crash cascade." };
    Ok(diagnosis.to_string())
}
