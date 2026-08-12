use std::{fs, path::PathBuf};
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cosmetic { pub id: String, pub name: String, pub kind: String, pub source: String }

fn cosmetics_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("cosmetics");
    fs::create_dir_all(dir.join("capes")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("wings")).map_err(|e| e.to_string())?;
    Ok(dir)
}

const BUILTIN: [(&str, &str, &str); 4] = [
    ("cape-makeforge", "MakeForge Cape", "cape"),
    ("cape-wave", "Wave Cape", "cape"),
    ("cape-ember", "Ember Cape", "cape"),
    ("wings-aurora", "Aurora Wings", "wings"),
];

#[tauri::command]
pub fn list_cosmetics(app: tauri::AppHandle) -> Result<Vec<Cosmetic>, String> {
    let root = cosmetics_root(&app)?;
    let mut out: Vec<Cosmetic> = BUILTIN.iter().map(|(id, name, kind)| Cosmetic {
        id: id.to_string(), name: name.to_string(), kind: kind.to_string(), source: "builtin".into(),
    }).collect();
    for sub in ["capes", "wings"] {
        let dir = root.join(sub);
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("png") {
                    let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("custom").to_string();
                    out.push(Cosmetic { id: id.clone(), name: id, kind: sub.trim_end_matches('s').into(), source: "imported".into() });
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn import_cosmetic(app: tauri::AppHandle, kind: String, file_path: String) -> Result<Cosmetic, String> {
    if kind != "cape" && kind != "wings" { return Err("kind must be 'cape' or 'wings'".into()); }
    let src = PathBuf::from(&file_path);
    if !src.exists() { return Err("Source PNG not found".into()); }
    let root = cosmetics_root(&app)?;
    let folder = if kind == "cape" { "capes" } else { "wings" };
    let name = src.file_stem().and_then(|s| s.to_str()).unwrap_or("imported").to_string();
    let dest = root.join(folder).join(format!("{}.png", name));
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(Cosmetic { id: name.clone(), name, kind, source: "imported".into() })
}

#[tauri::command]
pub fn apply_cosmetic(app: tauri::AppHandle, instance: String, cosmetic_id: String) -> Result<(), String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let safe_instance: String = instance.chars().map(|c| if c.is_ascii_alphanumeric() || c=='-' || c=='_' {c} else {'_'}).collect();
    let target_dir = root.join("instances").join(safe_instance).join("smoothclient").join("cosmetics");
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    fs::write(target_dir.join("selected.json"), serde_json::to_vec_pretty(&serde_json::json!({ "cosmetic_id": cosmetic_id })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(())
}
