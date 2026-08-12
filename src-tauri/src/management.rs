use std::{fs, io::Write, path::PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::Manager;
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceSummary { pub name: String, pub path: String, pub has_game: bool }

fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("instances");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn safe(value: &str) -> String { value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect() }

#[tauri::command]
pub fn list_instances(app: tauri::AppHandle) -> Result<Vec<InstanceSummary>, String> {
    let dir = root(&app)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
        let path = entry.path();
        result.push(InstanceSummary { name: entry.file_name().to_string_lossy().to_string(), path: path.to_string_lossy().to_string(), has_game: path.join("game").exists() });
    }
    result.sort_by(|a,b| a.name.cmp(&b.name));
    Ok(result)
}

#[tauri::command]
pub fn create_instance(app: tauri::AppHandle, name: String, version: Option<String>) -> Result<InstanceSummary, String> {
    if name.trim().is_empty() { return Err("Instance name cannot be empty".into()); }
    let path = root(&app)?.join(safe(&name));
    if path.exists() { return Err("An instance with that name already exists".into()); }
    fs::create_dir_all(path.join("game")).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("mods")).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("resourcepacks")).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("saves")).map_err(|e| e.to_string())?;
    fs::write(path.join("profile.json"), serde_json::to_vec_pretty(&json!({"name": name, "minecraft": version.unwrap_or_else(|| "1.21.11".into()), "loader": "vanilla", "ram_mb": 4096, "jvm_args": []})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(InstanceSummary { name: safe(&name), path: path.to_string_lossy().to_string(), has_game: true })
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
