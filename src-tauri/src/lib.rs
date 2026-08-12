mod management;
mod cosmetics;
mod mods;
mod accounts;
mod msauth;

use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};
use tauri::Manager;

const VERSION: &str = "1.21.11";
const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

fn safe_name(value: &str) -> String { value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect() }
fn sha1(bytes: &[u8]) -> String { let mut h = Sha1::new(); h.update(bytes); format!("{:x}", h.finalize()) }
fn offline_uuid(username: &str) -> String { let mut h = Sha1::new(); h.update(format!("OfflinePlayer:{}", username).as_bytes()); let hex = format!("{:x}", h.finalize()); format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]) }
async fn fetch_json(client: &reqwest::Client, url: &str) -> Result<Value, String> { client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.json::<Value>().await.map_err(|e| e.to_string()) }
async fn download_file(client: &reqwest::Client, url: &str, path: &Path, expected_hash: Option<&str>) -> Result<(), String> { if path.exists() { if let Some(hash) = expected_hash { let existing = fs::read(path).map_err(|e| e.to_string())?; if sha1(&existing) == hash { return Ok(()); } } else { return Ok(()); } } if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; } let bytes = client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?; if let Some(hash) = expected_hash { if sha1(&bytes) != hash { return Err(format!("Checksum mismatch while downloading {}", url)); } } fs::write(path, &bytes).map_err(|e| e.to_string()) }
fn os_name() -> &'static str { if cfg!(target_os = "windows") { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" } }
fn rules_allow(value: &Value) -> bool { let Some(rules) = value.get("rules").and_then(Value::as_array) else { return true; }; let mut allowed = false; for rule in rules { let matches_os = rule.get("os").and_then(|o| o.get("name")).and_then(Value::as_str).map(|v| v == os_name()).unwrap_or(true); if matches_os { if rule.get("action").and_then(Value::as_str) == Some("disallow") { return false; } if rule.get("action").and_then(Value::as_str) == Some("allow") { allowed = true; } } } if rules.iter().any(|r| r.get("action").and_then(Value::as_str) == Some("allow")) { allowed } else { true } }
fn substitute(mut s: String, game_dir: &Path, assets_dir: &Path, asset_index: &str, natives_dir: &Path, instance: &str, username: &str, uuid: &str, access_token: &str, user_type: &str) -> String { let game_dir_s = game_dir.to_string_lossy().to_string(); let assets_dir_s = assets_dir.to_string_lossy().to_string(); let natives_dir_s = natives_dir.to_string_lossy().to_string(); let library_dir_s = game_dir.join("libraries").to_string_lossy().to_string(); let replacements = [("${auth_player_name}", username),("${auth_uuid}",uuid),("${auth_access_token}",access_token),("${user_type}",user_type),("${version_name}",VERSION),("${version_type}","release"),("${assets_root}",assets_dir_s.as_str()),("${assets_index_name}",asset_index),("${game_directory}",game_dir_s.as_str()),("${natives_directory}",natives_dir_s.as_str()),("${library_directory}",library_dir_s.as_str()),("${classpath_separator}",if cfg!(target_os="windows"){";"}else{":"}),("${launcher_name}","BlockPilot"),("${launcher_version}","0.1.0"),("${clientid}",""),("${auth_xuid}",""),("${instance_name}",instance)]; for (from,to) in replacements { s=s.replace(from,to); } s }
fn collect_args(value: &Value, game_dir: &Path, assets_dir: &Path, asset_index: &str, natives_dir: &Path, instance: &str, username: &str, uuid: &str, access_token: &str, user_type: &str) -> Vec<String> { let mut out=Vec::new(); let Some(args)=value.as_array() else{return out;}; for entry in args { if !rules_allow(entry){continue;} let raw=entry.get("value").unwrap_or(entry); match raw { Value::String(s)=>out.push(substitute(s.clone(),game_dir,assets_dir,asset_index,natives_dir,instance,username,uuid,access_token,user_type)), Value::Array(values)=>for v in values.iter().filter_map(Value::as_str){out.push(substitute(v.to_string(),game_dir,assets_dir,asset_index,natives_dir,instance,username,uuid,access_token,user_type));}, _=>{} } } out }

const AUTHLIB_INJECTOR_URL: &str = "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.5/authlib-injector-1.2.5.jar";

async fn ensure_authlib_injector(client: &reqwest::Client, root: &Path) -> Result<PathBuf, String> {
    let path = root.join("authlib-injector.jar");
    if !path.exists() { download_file(client, AUTHLIB_INJECTOR_URL, &path, None).await?; }
    Ok(path)
}

#[tauri::command]
async fn launch_instance(app: tauri::AppHandle, instance: String) -> Result<String, String> {
    if instance.trim().is_empty() { return Err("No Minecraft instance selected".into()); }
    let root=app.path().app_data_dir().map_err(|e|e.to_string())?; let instance_dir=root.join("instances").join(safe_name(&instance)); let game_dir=instance_dir.join("game"); let libraries_dir=game_dir.join("libraries"); let versions_dir=game_dir.join("versions"); let assets_dir=game_dir.join("assets"); let natives_dir=instance_dir.join("natives");
    for dir in [&libraries_dir,&versions_dir,&assets_dir,&natives_dir] { fs::create_dir_all(dir).map_err(|e|e.to_string())?; }
    let client=reqwest::Client::builder().user_agent("BlockPilot/0.1.0").build().map_err(|e|e.to_string())?; let manifest=fetch_json(&client,VERSION_MANIFEST).await?; let version_url=manifest.get("versions").and_then(Value::as_array).and_then(|v|v.iter().find(|x|x.get("id").and_then(Value::as_str)==Some(VERSION))).and_then(|x|x.get("url")).and_then(Value::as_str).ok_or_else(||format!("Minecraft {} was not found in Mojang's manifest",VERSION))?; let meta=fetch_json(&client,version_url).await?;
    let version_dir=versions_dir.join(VERSION); fs::create_dir_all(&version_dir).map_err(|e|e.to_string())?; let client_jar=version_dir.join(format!("{}.jar",VERSION)); let client_download=meta.get("downloads").and_then(|d|d.get("client")).ok_or("Minecraft client download is missing")?; download_file(&client,client_download.get("url").and_then(Value::as_str).ok_or("Client URL missing")?,&client_jar,client_download.get("sha1").and_then(Value::as_str)).await?;
    let mut classpath=Vec::new(); if let Some(libs)=meta.get("libraries").and_then(Value::as_array){ for lib in libs { if !rules_allow(lib){continue;} if let Some(artifact)=lib.get("downloads").and_then(|d|d.get("artifact")){ if let (Some(url),Some(path))=(artifact.get("url").and_then(Value::as_str),artifact.get("path").and_then(Value::as_str)){ let file=libraries_dir.join(path); download_file(&client,url,&file,artifact.get("sha1").and_then(Value::as_str)).await?; classpath.push(file.to_string_lossy().to_string()); } } if let Some(classifiers)=lib.get("downloads").and_then(|d|d.get("classifiers")){ let key=if cfg!(target_os="windows"){"natives-windows"}else if cfg!(target_os="macos"){"natives-osx"}else{"natives-linux"}; if let Some(native)=classifiers.get(key){ if let (Some(url),Some(path))=(native.get("url").and_then(Value::as_str),native.get("path").and_then(Value::as_str)){ let file=libraries_dir.join(path); download_file(&client,url,&file,native.get("sha1").and_then(Value::as_str)).await?; let archive=fs::File::open(&file).map_err(|e|e.to_string())?; let mut zip=zip::ZipArchive::new(archive).map_err(|e|e.to_string())?; for i in 0..zip.len(){ let mut entry=zip.by_index(i).map_err(|e|e.to_string())?; let name=entry.name().to_string(); if name.starts_with("META-INF/")||name.ends_with('/') {continue;} let filename=Path::new(&name).file_name().ok_or("Invalid native filename")?; let out=natives_dir.join(filename); let mut dest=fs::File::create(out).map_err(|e|e.to_string())?; std::io::copy(&mut entry,&mut dest).map_err(|e|e.to_string())?; } } } } } }
    classpath.push(client_jar.to_string_lossy().to_string()); let asset_index=meta.get("assetIndex").ok_or("Asset index missing")?; let asset_id=asset_index.get("id").and_then(Value::as_str).unwrap_or(VERSION); let asset_index_file=assets_dir.join("indexes").join(format!("{}.json",asset_id)); download_file(&client,asset_index.get("url").and_then(Value::as_str).ok_or("Asset index URL missing")?,&asset_index_file,asset_index.get("sha1").and_then(Value::as_str)).await?; let assets_json:Value=serde_json::from_slice(&fs::read(&asset_index_file).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?; if let Some(objects)=assets_json.get("objects").and_then(Value::as_object){ for object in objects.values(){ let hash=object.get("hash").and_then(Value::as_str).ok_or("Asset hash missing")?; let prefix=&hash[0..2]; let path=assets_dir.join("objects").join(prefix).join(hash); let url=format!("https://resources.download.minecraft.net/{}/{}",prefix,hash); download_file(&client,&url,&path,Some(hash)).await?; } }

    // resolve identity: microsoft (real Mojang session), ely.by (skins/capes via authlib-injector), or offline fallback
    let account = accounts::get_active_account(&app);
    let (username, uuid, access_token, user_type) = match &account {
        Some(acc) if acc.kind == "microsoft" => (acc.username.clone(), acc.uuid.clone(), acc.access_token.clone().unwrap_or_else(|| "0".into()), "msa".to_string()),
        Some(acc) if acc.kind == "elyby" => (acc.username.clone(), acc.uuid.clone(), acc.access_token.clone().unwrap_or_else(|| "0".into()), "mojang".to_string()),
        Some(acc) => (acc.username.clone(), acc.uuid.clone(), "0".to_string(), "legacy".to_string()),
        None => ("Steve".to_string(), offline_uuid("Steve"), "0".to_string(), "legacy".to_string()),
    };

    let main_class=meta.get("mainClass").and_then(Value::as_str).ok_or("Minecraft main class missing")?; let separator=if cfg!(target_os="windows"){";"}else{":"}; let classpath_value=classpath.join(separator); let mut args:Vec<String>=vec!["-Xmx4G".into(),format!("-Djava.library.path={}",natives_dir.to_string_lossy())];
    if let Some(acc) = &account { if acc.kind == "elyby" {
        let injector = ensure_authlib_injector(&client, &root).await?;
        args.push(format!("-javaagent:{}=ely.by", injector.to_string_lossy()));
    }}
    if let Some(jvm)=meta.get("arguments").and_then(|a|a.get("jvm")){args.extend(collect_args(jvm,&game_dir,&assets_dir,asset_id,&natives_dir,&instance,&username,&uuid,&access_token,&user_type).into_iter().map(|arg|arg.replace("${classpath}",&classpath_value)));}
    if !args.iter().any(|a|a == "-cp") { args.extend(["-cp".into(),classpath_value.clone()]); }
    args.push(main_class.into());
    if let Some(game)=meta.get("arguments").and_then(|a|a.get("game")){args.extend(collect_args(game,&game_dir,&assets_dir,asset_id,&natives_dir,&instance,&username,&uuid,&access_token,&user_type));} else if let Some(legacy)=meta.get("minecraftArguments").and_then(Value::as_str){args.extend(legacy.split_whitespace().map(|s|substitute(s.into(),&game_dir,&assets_dir,asset_id,&natives_dir,&instance,&username,&uuid,&access_token,&user_type)));}
    if !args.iter().any(|a|a=="--username"){ args.extend(["--username".into(),username.clone(),"--uuid".into(),uuid,"--accessToken".into(),access_token,"--userType".into(),user_type,"--version".into(),VERSION.into(),"--versionType".into(),"release".into(),"--gameDir".into(),game_dir.to_string_lossy().to_string(),"--assetsDir".into(),assets_dir.to_string_lossy().to_string(),"--assetIndex".into(),asset_id.into()]); }
    let mut child=Command::new("java").args(&args).current_dir(&game_dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e|format!("Could not start Java. Install Java 21 or select a Java 21 runtime in Settings. ({})",e))?; let pid=child.id(); tauri::async_runtime::spawn(async move{let _=child.wait();}); Ok(format!("Minecraft {} launched for '{}' as {} (PID {})",VERSION,instance,username,pid))
}

#[tauri::command]
fn runtime_info() -> Result<String,String> { match Command::new("java").arg("-version").output(){Ok(output)=>Ok(String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("Java detected").to_string()),Err(_)=>Err("Java was not found on PATH".into())} }
#[tauri::command]
fn launcher_data_dir(app:tauri::AppHandle)->Result<String,String>{let dir=app.path().app_data_dir().map_err(|e|e.to_string())?;fs::create_dir_all(&dir).map_err(|e|e.to_string())?;Ok(dir.to_string_lossy().to_string())}
#[tauri::command]
fn open_instance_folder(app:tauri::AppHandle,instance:String)->Result<(),String>{let root=app.path().app_data_dir().map_err(|e|e.to_string())?;let dir=root.join("instances").join(safe_name(&instance));fs::create_dir_all(&dir).map_err(|e|e.to_string())?;let cmd=if cfg!(target_os="windows"){"explorer"}else if cfg!(target_os="macos"){"open"}else{"xdg-open"};Command::new(cmd).arg(&dir).spawn().map_err(|e|e.to_string())?;Ok(())}
#[derive(serde::Serialize)]
struct JavaRuntime{path:String,version:String}
#[tauri::command]
fn list_java_runtimes()->Result<Vec<JavaRuntime>,String>{let mut found=Vec::new();for cmd in ["java"]{if let Ok(output)=Command::new(cmd).arg("-version").output(){let line=String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("Unknown").to_string();found.push(JavaRuntime{path:cmd.into(),version:line});}}if found.is_empty(){return Err("No Java runtime found on PATH. Install Java 21 for Minecraft 1.21.x.".into());}Ok(found)}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(){tauri::Builder::default().plugin(tauri_plugin_shell::init()).invoke_handler(tauri::generate_handler![launch_instance,runtime_info,launcher_data_dir,open_instance_folder,list_java_runtimes,management::list_instances,management::create_instance,management::snapshot_instance,management::analyze_crash,management::delete_instance,cosmetics::list_cosmetics,cosmetics::apply_cosmetic,cosmetics::import_cosmetic,mods::search_mods,mods::install_mod,accounts::list_accounts,accounts::add_offline_account,accounts::add_elyby_account,accounts::remove_account,accounts::set_active_account,accounts::import_skin,accounts::set_cape,msauth::start_microsoft_login,msauth::get_launcher_settings,msauth::set_ms_client_id]).run(tauri::generate_context!()).expect("error while running BlockPilot");}
