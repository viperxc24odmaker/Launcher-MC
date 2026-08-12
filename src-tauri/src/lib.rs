use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};
use tauri::Manager;

const VERSION: &str = "1.21.11";
const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

fn safe_name(value: &str) -> String {
    value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

async fn fetch_json(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.json::<Value>().await.map_err(|e| e.to_string())
}

async fn download_file(client: &reqwest::Client, url: &str, path: &Path, expected_hash: Option<&str>) -> Result<(), String> {
    if path.exists() {
        if let Some(hash) = expected_hash {
            let existing = fs::read(path).map_err(|e| e.to_string())?;
            if sha256(&existing) == hash { return Ok(()); }
        } else { return Ok(()); }
    }
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let bytes = client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    if let Some(hash) = expected_hash {
        if sha256(&bytes) != hash { return Err(format!("Checksum mismatch while downloading {}", url)); }
    }
    fs::write(path, &bytes).map_err(|e| e.to_string())
}

fn os_name() -> &'static str {
    if cfg!(target_os = "windows") { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" }
}

fn rules_allow(value: &Value) -> bool {
    let Some(rules) = value.get("rules").and_then(Value::as_array) else { return true; };
    let mut allowed = false;
    for rule in rules {
        let matches_os = rule.get("os").and_then(|o| o.get("name")).and_then(Value::as_str).map(|v| v == os_name()).unwrap_or(true);
        if matches_os {
            if rule.get("action").and_then(Value::as_str) == Some("disallow") { return false; }
            if rule.get("action").and_then(Value::as_str) == Some("allow") { allowed = true; }
        }
    }
    if rules.iter().any(|r| r.get("action").and_then(Value::as_str) == Some("allow")) { allowed } else { true }
}

fn substitute(mut s: String, game_dir: &Path, assets_dir: &Path, asset_index: &str, natives_dir: &Path, instance: &str) -> String {
    let username = std::env::var("BLOCKPILOT_PLAYER").unwrap_or_else(|_| "Steve".into());
    let mut hasher = Sha256::new(); hasher.update(username.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    let uuid = format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]);
    let replacements = [
        ("${auth_player_name}", username.as_str()), ("${auth_uuid}", uuid.as_str()),
        ("${auth_access_token}", "0"), ("${user_type}", "legacy"), ("${version_name}", VERSION),
        ("${version_type}", "release"), ("${assets_root}", assets_dir.to_string_lossy().as_ref()),
        ("${assets_index_name}", asset_index), ("${game_directory}", game_dir.to_string_lossy().as_ref()),
        ("${natives_directory}", natives_dir.to_string_lossy().as_ref()), ("${library_directory}", game_dir.join("libraries").to_string_lossy().as_ref()),
        ("${classpath_separator}", if cfg!(target_os = "windows") { ";" } else { ":" }),
        ("${launcher_name}", "BlockPilot"), ("${launcher_version}", "0.1.0"), ("${clientid}", ""), ("${auth_xuid}", ""),
    ];
    for (from, to) in replacements { s = s.replace(from, to); }
    s.replace("${instance_name}", instance)
}

fn collect_args(value: &Value, game_dir: &Path, assets_dir: &Path, asset_index: &str, natives_dir: &Path, instance: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(args) = value.as_array() else { return out; };
    for entry in args {
        if !rules_allow(entry) { continue; }
        let raw = entry.get("value").unwrap_or(entry);
        match raw {
            Value::String(s) => out.push(substitute(s.clone(), game_dir, assets_dir, asset_index, natives_dir, instance)),
            Value::Array(values) => for v in values.iter().filter_map(Value::as_str) { out.push(substitute(v.to_string(), game_dir, assets_dir, asset_index, natives_dir, instance)); },
            _ => {}
        }
    }
    out
}

#[tauri::command]
async fn launch_instance(app: tauri::AppHandle, instance: String) -> Result<String, String> {
    if instance.trim().is_empty() { return Err("No Minecraft instance selected".into()); }
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let instance_dir = root.join("instances").join(safe_name(&instance));
    let game_dir = instance_dir.join("game");
    let libraries_dir = game_dir.join("libraries");
    let versions_dir = game_dir.join("versions");
    let assets_dir = game_dir.join("assets");
    let natives_dir = instance_dir.join("natives");
    fs::create_dir_all(&libraries_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&versions_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&natives_dir).map_err(|e| e.to_string())?;

    let client = reqwest::Client::builder().user_agent("BlockPilot/0.1.0").build().map_err(|e| e.to_string())?;
    let manifest = fetch_json(&client, VERSION_MANIFEST).await?;
    let version_url = manifest.get("versions").and_then(Value::as_array).and_then(|v| v.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(VERSION))).and_then(|x| x.get("url")).and_then(Value::as_str).ok_or_else(|| format!("Minecraft {} was not found in Mojang's manifest", VERSION))?;
    let meta = fetch_json(&client, version_url).await?;

    let version_dir = versions_dir.join(VERSION);
    fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;
    let client_jar = version_dir.join(format!("{}.jar", VERSION));
    let client_download = meta.get("downloads").and_then(|d| d.get("client")).ok_or("Minecraft client download is missing")?;
    download_file(&client, client_download.get("url").and_then(Value::as_str).ok_or("Client URL missing")?, &client_jar, client_download.get("sha1").and_then(Value::as_str)).await?;

    let mut classpath: Vec<String> = Vec::new();
    if let Some(libs) = meta.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            if !rules_allow(lib) { continue; }
            if let Some(artifact) = lib.get("downloads").and_then(|d| d.get("artifact")) {
                if let (Some(url), Some(path)) = (artifact.get("url").and_then(Value::as_str), artifact.get("path").and_then(Value::as_str)) {
                    let file = libraries_dir.join(path);
                    download_file(&client, url, &file, artifact.get("sha1").and_then(Value::as_str)).await?;
                    classpath.push(file.to_string_lossy().to_string());
                }
            }
            if let Some(classifiers) = lib.get("downloads").and_then(|d| d.get("classifiers")) {
                let key = if cfg!(target_os = "windows") { "natives-windows" } else if cfg!(target_os = "macos") { "natives-osx" } else { "natives-linux" };
                if let Some(native) = classifiers.get(key) {
                    if let (Some(url), Some(path)) = (native.get("url").and_then(Value::as_str), native.get("path").and_then(Value::as_str)) {
                        let file = libraries_dir.join(path);
                        download_file(&client, url, &file, native.get("sha1").and_then(Value::as_str)).await?;
                        let archive = fs::File::open(&file).map_err(|e| e.to_string())?;
                        let mut zip = zip::ZipArchive::new(archive).map_err(|e| e.to_string())?;
                        for i in 0..zip.len() {
                            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
                            let name = entry.name().to_string();
                            if name.starts_with("META-INF/") || name.ends_with('/') { continue; }
                            let out = natives_dir.join(Path::new(&name).file_name().ok_or("Invalid native filename")?);
                            let mut dest = fs::File::create(out).map_err(|e| e.to_string())?;
                            std::io::copy(&mut entry, &mut dest).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }
    }
    classpath.push(client_jar.to_string_lossy().to_string());

    let asset_index = meta.get("assetIndex").ok_or("Asset index missing")?;
    let asset_id = asset_index.get("id").and_then(Value::as_str).unwrap_or(VERSION);
    let asset_index_file = assets_dir.join("indexes").join(format!("{}.json", asset_id));
    download_file(&client, asset_index.get("url").and_then(Value::as_str).ok_or("Asset index URL missing")?, &asset_index_file, asset_index.get("sha1").and_then(Value::as_str)).await?;
    let assets_json: Value = serde_json::from_slice(&fs::read(&asset_index_file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if let Some(objects) = assets_json.get("objects").and_then(Value::as_object) {
        for object in objects.values() {
            let hash = object.get("hash").and_then(Value::as_str).ok_or("Asset hash missing")?;
            let prefix = &hash[0..2];
            let path = assets_dir.join("objects").join(prefix).join(hash);
            let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);
            download_file(&client, &url, &path, Some(hash)).await?;
        }
    }

    let main_class = meta.get("mainClass").and_then(Value::as_str).ok_or("Minecraft main class missing")?;
    let mut args: Vec<String> = vec!["-Xmx4G".into(), format!("-Djava.library.path={}", natives_dir.to_string_lossy()), "-cp".into(), classpath.join(if cfg!(target_os = "windows") { ";" } else { ":" }).into(), main_class.into()];
    if let Some(jvm) = meta.get("arguments").and_then(|a| a.get("jvm")) { args.extend(collect_args(jvm, &game_dir, &assets_dir, asset_id, &natives_dir, &instance)); }
    if let Some(game) = meta.get("arguments").and_then(|a| a.get("game")) { args.extend(collect_args(game, &game_dir, &assets_dir, asset_id, &natives_dir, &instance)); }
    else if let Some(legacy) = meta.get("minecraftArguments").and_then(Value::as_str) { args.extend(legacy.split_whitespace().map(|s| substitute(s.into(), &game_dir, &assets_dir, asset_id, &natives_dir, &instance))); }

    if !args.iter().any(|a| a == "--username") {
        let username = std::env::var("BLOCKPILOT_PLAYER").unwrap_or_else(|_| "Steve".into());
        let mut h = Sha256::new(); h.update(username.as_bytes()); let hex = format!("{:x}", h.finalize());
        let uuid = format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]);
        args.extend(["--username".into(), username, "--uuid".into(), uuid, "--accessToken".into(), "0".into(), "--userType".into(), "legacy".into(), "--version".into(), VERSION.into(), "--versionType".into(), "release".into(), "--gameDir".into(), game_dir.to_string_lossy().to_string(), "--assetsDir".into(), assets_dir.to_string_lossy().to_string(), "--assetIndex".into(), asset_id.into()]);
    }

    let mut child = Command::new("java").args(&args).current_dir(&game_dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("Could not start Java. Install Java 21 or select a Java 21 runtime in Settings. ({})", e))?;
    let pid = child.id();
    tauri::async_runtime::spawn(async move { let _ = child.wait(); });
    Ok(format!("Minecraft {} launched for '{}' (PID {})", VERSION, instance, pid))
}

#[tauri::command]
fn runtime_info() -> Result<String, String> {
    let java = Command::new("java").arg("-version").output();
    match java { Ok(output) => Ok(String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("Java detected").to_string()), Err(_) => Err("Java was not found on PATH".into()) }
}

#[tauri::command]
fn launcher_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    let dir: PathBuf = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default().invoke_handler(tauri::generate_handler![launch_instance, runtime_info, launcher_data_dir]).run(tauri::generate_context!()).expect("error while running BlockPilot");
}
