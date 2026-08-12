use std::{fs, io::{Read, Write}, net::TcpListener, path::PathBuf, time::Duration};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::Manager;

use crate::accounts::{self, Account};

const REDIRECT_URI: &str = "http://127.0.0.1:5285/";
const AUTH_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LauncherSettings {
    pub ms_client_id: Option<String>,
}

fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

#[tauri::command]
pub fn get_launcher_settings(app: tauri::AppHandle) -> Result<LauncherSettings, String> {
    let path = settings_file(&app)?;
    if !path.exists() { return Ok(LauncherSettings::default()); }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.is_empty() { return Ok(LauncherSettings::default()); }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_ms_client_id(app: tauri::AppHandle, client_id: String) -> Result<(), String> {
    let path = settings_file(&app)?;
    let settings = LauncherSettings { ms_client_id: if client_id.trim().is_empty() { None } else { Some(client_id) } };
    fs::write(&path, serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn random_string(len: usize) -> String {
    let bytes: Vec<u8> = (0..len).map(|_| rand::thread_rng().gen()).collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_pair() -> (String, String) {
    let verifier = random_string(64);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}

fn open_browser(url: &str) -> Result<(), String> {
    let cmd = if cfg!(target_os = "windows") { "cmd" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    if cfg!(target_os = "windows") {
        std::process::Command::new(cmd).args(["/C", "start", "", url]).spawn().map_err(|e| e.to_string())?;
    } else {
        std::process::Command::new(cmd).arg(url).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Blocks on a local loopback listener until Microsoft redirects back with a code.
fn await_redirect_code(expected_state: &str) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:5285")
        .map_err(|e| format!("Could not bind the local sign-in listener on port 5285 ({}). Close anything else using that port and try again.", e))?;
    listener.set_ttl(64).ok();

    let mut attempts = 0;
    for stream in listener.incoming() {
        attempts += 1;
        if attempts > 8 { return Err("Timed out waiting for Microsoft sign-in to complete".into()); }
        let mut stream = match stream { Ok(s) => s, Err(_) => continue };
        stream.set_read_timeout(Some(Duration::from_secs(180))).ok();
        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) { Ok(n) => n, Err(_) => continue };
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or("");
        let path = first_line.split_whitespace().nth(1).unwrap_or("/");
        let has_code = path.contains("code=");

        let body = if has_code {
            "<html><body style='font-family:sans-serif;background:#080b09;color:#e9eee9;display:flex;align-items:center;justify-content:center;height:100vh;margin:0'><h2>Signed in - you can close this tab.</h2></body></html>"
        } else {
            "<html><body style='font-family:sans-serif;background:#080b09;color:#e9eee9;display:flex;align-items:center;justify-content:center;height:100vh;margin:0'><h2>Waiting for sign-in...</h2></body></html>"
        };
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();

        if !has_code { continue; }

        let mut code = None;
        let mut state = None;
        if let Some(query) = path.split('?').nth(1) {
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    match k { "code" => code = Some(v.to_string()), "state" => state = Some(v.to_string()), _ => {} }
                }
            }
        }
        return match code {
            Some(c) if state.as_deref() == Some(expected_state) => Ok(c),
            Some(_) => Err("Sign-in state mismatch - try again".into()),
            None => Err("Microsoft did not return a sign-in code. You may have cancelled or denied access.".into()),
        };
    }
    Err("Sign-in window closed unexpectedly".into())
}

#[tauri::command]
pub async fn start_microsoft_login(app: tauri::AppHandle) -> Result<Account, String> {
    let settings = get_launcher_settings(app.clone())?;
    let client_id = settings.ms_client_id.ok_or("Add your Azure app's Client ID in Settings first")?;

    let (verifier, challenge) = pkce_pair();
    let state = random_string(16);
    let scope = "XboxLive.signin offline_access";
    let auth_url = format!(
        "{AUTH_URL}?client_id={client_id}&response_type=code&redirect_uri={redirect}&scope={scope}&code_challenge={challenge}&code_challenge_method=S256&state={state}",
        redirect = urlenc(REDIRECT_URI), scope = urlenc(scope)
    );
    open_browser(&auth_url)?;

    // blocking loopback wait happens off the async executor's cooperative point, acceptable for a one-shot user-driven sign-in
    let code = await_redirect_code(&state)?;

    let client = reqwest::Client::builder().user_agent("BlockPilot/0.1.0").build().map_err(|e| e.to_string())?;

    // 1) MSA token exchange
    let token_resp: Value = client.post(TOKEN_URL).form(&[
        ("client_id", client_id.as_str()),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", verifier.as_str()),
    ]).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let msa_access = token_resp.get("access_token").and_then(Value::as_str).ok_or_else(|| describe_ms_error(&token_resp))?;
    let msa_refresh = token_resp.get("refresh_token").and_then(Value::as_str).map(String::from);

    // 2) Xbox Live user auth
    let xbl: Value = client.post("https://user.auth.xboxlive.com/user/authenticate").json(&json!({
        "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", msa_access) },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    })).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let xbl_token = xbl.get("Token").and_then(Value::as_str).ok_or("Xbox Live sign-in failed")?;
    let uhs = xbl.get("DisplayClaims").and_then(|d| d.get("xui")).and_then(Value::as_array)
        .and_then(|a| a.first()).and_then(|x| x.get("uhs")).and_then(Value::as_str).ok_or("Xbox Live user hash missing")?;

    // 3) XSTS token
    let xsts: Value = client.post("https://xsts.auth.xboxlive.com/xsts/authorize").json(&json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    })).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    if let Some(err_code) = xsts.get("XErr").and_then(Value::as_u64) {
        return Err(describe_xsts_error(err_code));
    }
    let xsts_token = xsts.get("Token").and_then(Value::as_str).ok_or("Xbox security token exchange failed")?;

    // 4) Minecraft login
    let mc: Value = client.post("https://api.minecraftservices.com/authentication/login_with_xbox").json(&json!({
        "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
    })).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let mc_access = mc.get("access_token").and_then(Value::as_str).ok_or("Minecraft sign-in failed")?;

    // 5) Profile (also implicitly confirms game ownership - fails with 404 if the account never bought Minecraft)
    let profile: Value = client.get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_access).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let username = profile.get("name").and_then(Value::as_str)
        .ok_or("This Microsoft account doesn't own Minecraft: Java Edition")?.to_string();
    let uuid_raw = profile.get("id").and_then(Value::as_str).ok_or("Missing profile id")?;
    let uuid = format!("{}-{}-{}-{}-{}", &uuid_raw[0..8], &uuid_raw[8..12], &uuid_raw[12..16], &uuid_raw[16..20], &uuid_raw[20..32]);

    let account = Account {
        id: format!("microsoft-{}", uuid),
        kind: "microsoft".into(),
        username,
        uuid,
        access_token: Some(mc_access.to_string()),
        refresh_token: msa_refresh,
        skin_path: None,
        cape_id: None,
        active: true,
    };
    accounts::upsert_account(&app, account.clone())?;
    Ok(account)
}

fn describe_ms_error(resp: &Value) -> String {
    resp.get("error_description").and_then(Value::as_str)
        .unwrap_or("Microsoft sign-in failed. Double check the Client ID in Settings.").to_string()
}

fn describe_xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile. Create one at xbox.com then try again.".into(),
        2148916235 => "Xbox Live isn't available in this account's region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on the Xbox website.".into(),
        2148916238 => "This is a child account - an adult needs to add it to a Microsoft Family group first.".into(),
        _ => format!("Xbox sign-in was rejected (code {})", code),
    }
}

fn urlenc(s: &str) -> String {
    s.chars().map(|c| match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "%20".to_string(),
        _ => format!("%{:02X}", c as u32),
    }).collect()
}
