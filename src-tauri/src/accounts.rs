use std::{fs, path::PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: String,
    pub kind: String,           // "offline" | "elyby"
    pub username: String,
    pub uuid: String,
    pub access_token: Option<String>,
    pub skin_path: Option<String>,
    pub cape_id: Option<String>,
    pub active: bool,
}

fn accounts_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("accounts.json"))
}

fn skins_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("skins");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn load_all(app: &tauri::AppHandle) -> Result<Vec<Account>, String> {
    let path = accounts_file(app)?;
    if !path.exists() { return Ok(Vec::new()); }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.is_empty() { return Ok(Vec::new()); }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn save_all(app: &tauri::AppHandle, accounts: &[Account]) -> Result<(), String> {
    let path = accounts_file(app)?;
    fs::write(&path, serde_json::to_vec_pretty(accounts).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn offline_uuid(username: &str) -> String {
    let mut h = Sha1::new();
    h.update(format!("OfflinePlayer:{}", username).as_bytes());
    let hex = format!("{:x}", h.finalize());
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

#[tauri::command]
pub fn list_accounts(app: tauri::AppHandle) -> Result<Vec<Account>, String> { load_all(&app) }

#[tauri::command]
pub fn add_offline_account(app: tauri::AppHandle, username: String) -> Result<Account, String> {
    if username.trim().is_empty() { return Err("Enter a username".into()); }
    let mut accounts = load_all(&app)?;
    if accounts.iter().any(|a| a.kind == "offline" && a.username.eq_ignore_ascii_case(&username)) {
        return Err("That offline profile already exists".into());
    }
    let has_active = accounts.iter().any(|a| a.active);
    let account = Account {
        id: format!("offline-{}", username.to_lowercase()),
        kind: "offline".into(),
        username: username.clone(),
        uuid: offline_uuid(&username),
        access_token: None,
        skin_path: None,
        cape_id: None,
        active: !has_active,
    };
    accounts.push(account.clone());
    save_all(&app, &accounts)?;
    Ok(account)
}

#[tauri::command]
pub async fn add_elyby_account(app: tauri::AppHandle, username: String, password: String) -> Result<Account, String> {
    if username.trim().is_empty() || password.is_empty() { return Err("Enter your ely.by username/email and password".into()); }
    let client = reqwest::Client::builder().user_agent("BlockPilot/0.1.0").build().map_err(|e| e.to_string())?;
    let payload = json!({
        "username": username,
        "password": password,
        "requestUser": true,
        "clientToken": format!("blockpilot-{}", offline_uuid(&username))
    });
    let resp = client.post("https://authserver.ely.by/auth/authenticate").json(&payload).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let body: Value = resp.json().await.unwrap_or(json!({}));
        let msg = body.get("errorMessage").and_then(Value::as_str).unwrap_or("ely.by login failed. Check your username/password.");
        return Err(msg.to_string());
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    let access_token = body.get("accessToken").and_then(Value::as_str).ok_or("ely.by did not return an access token")?.to_string();
    let profile = body.get("selectedProfile").ok_or("ely.by did not return a profile")?;
    let real_username = profile.get("name").and_then(Value::as_str).unwrap_or(&username).to_string();
    let uuid_raw = profile.get("id").and_then(Value::as_str).ok_or("Missing profile id")?;
    let uuid = format!("{}-{}-{}-{}-{}", &uuid_raw[0..8], &uuid_raw[8..12], &uuid_raw[12..16], &uuid_raw[16..20], &uuid_raw[20..32]);

    let mut accounts = load_all(&app)?;
    accounts.retain(|a| a.id != format!("elyby-{}", real_username.to_lowercase()));
    let has_active = accounts.iter().any(|a| a.active);
    let account = Account {
        id: format!("elyby-{}", real_username.to_lowercase()),
        kind: "elyby".into(),
        username: real_username,
        uuid,
        access_token: Some(access_token),
        skin_path: None,
        cape_id: None,
        active: !has_active,
    };
    accounts.push(account.clone());
    save_all(&app, &accounts)?;
    Ok(account)
}

#[tauri::command]
pub fn remove_account(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut accounts = load_all(&app)?;
    let was_active = accounts.iter().any(|a| a.id == id && a.active);
    accounts.retain(|a| a.id != id);
    if was_active { if let Some(first) = accounts.first_mut() { first.active = true; } }
    save_all(&app, &accounts)
}

#[tauri::command]
pub fn set_active_account(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut accounts = load_all(&app)?;
    if !accounts.iter().any(|a| a.id == id) { return Err("Account not found".into()); }
    for a in accounts.iter_mut() { a.active = a.id == id; }
    save_all(&app, &accounts)
}

pub fn get_active_account(app: &tauri::AppHandle) -> Option<Account> {
    load_all(app).ok()?.into_iter().find(|a| a.active)
}

#[tauri::command]
pub fn import_skin(app: tauri::AppHandle, id: String, file_path: String) -> Result<Account, String> {
    let src = PathBuf::from(&file_path);
    if !src.exists() || src.extension().and_then(|e| e.to_str()) != Some("png") {
        return Err("Pick a 64x64 (or 64x32) PNG skin file".into());
    }
    let dir = skins_dir(&app)?;
    let dest = dir.join(format!("{}.png", id));
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    let mut accounts = load_all(&app)?;
    let account = accounts.iter_mut().find(|a| a.id == id).ok_or("Account not found")?;
    account.skin_path = Some(dest.to_string_lossy().to_string());
    let updated = account.clone();
    save_all(&app, &accounts)?;
    Ok(updated)
}

#[tauri::command]
pub fn set_cape(app: tauri::AppHandle, id: String, cape_id: Option<String>) -> Result<Account, String> {
    let mut accounts = load_all(&app)?;
    let account = accounts.iter_mut().find(|a| a.id == id).ok_or("Account not found")?;
    account.cape_id = cape_id;
    let updated = account.clone();
    save_all(&app, &accounts)?;
    Ok(updated)
}
