use std::{path::PathBuf, process::Command};
use tauri::Manager;

#[tauri::command]
fn launch_instance(instance: String) -> Result<String, String> {
    if instance.trim().is_empty() { return Err("No Minecraft instance selected".into()); }
    // The command boundary is intentionally native: the full Minecraft metadata,
    // authentication and classpath builder will live here rather than in the UI.
    // For now this validates the selected profile and returns a useful status.
    Ok(format!("Instance '{}' queued for native launch", instance))
}

#[tauri::command]
fn runtime_info() -> Result<String, String> {
    let java = Command::new("java").arg("-version").output();
    match java {
        Ok(output) => Ok(String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("Java detected").to_string()),
        Err(_) => Err("Java was not found on PATH".into()),
    }
}

#[tauri::command]
fn launcher_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    let dir: PathBuf = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![launch_instance, runtime_info, launcher_data_dir])
        .run(tauri::generate_context!())
        .expect("error while running BlockPilot");
}
