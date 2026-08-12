mod management;
mod cosmetics;
mod mods;
mod accounts;
mod msauth;
mod versions;

use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tauri::{Emitter, Manager};

const VERSION_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

const NEOFORGE_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

const AUTHLIB_INJECTOR_URL: &str =
    "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.5/authlib-injector-1.2.5.jar";

fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sha1(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn offline_uuid(username: &str) -> String {
    let mut h = Sha1::new();
    h.update(format!("OfflinePlayer:{}", username).as_bytes());

    let hex = format!("{:x}", h.finalize());

    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("BlockPilot/0.1.0")
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}

async fn fetch_json(
    client: &reqwest::Client,
    url: &str,
) -> Result<Value, String> {
    client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error reaching {}: {}", url, e))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_text(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, String> {
    client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error reaching {}: {}", url, e))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    expected_hash: Option<&str>,
) -> Result<(), String> {
    if path.exists() {
        if let Some(hash) = expected_hash {
            let existing = fs::read(path).map_err(|e| e.to_string())?;

            if sha1(&existing).eq_ignore_ascii_case(hash) {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error downloading {}: {}", url, e))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(hash) = expected_hash {
        if !hash.is_empty() && !sha1(&bytes).eq_ignore_ascii_case(hash) {
            return Err(format!(
                "Checksum mismatch while downloading {}",
                url
            ));
        }
    }

    fs::write(path, &bytes).map_err(|e| e.to_string())
}

fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn rules_allow(value: &Value) -> bool {
    let Some(rules) = value.get("rules").and_then(Value::as_array) else {
        return true;
    };

    let mut allowed = false;
    let mut saw_allow = false;

    for rule in rules {
        let matches_os = rule
            .get("os")
            .and_then(|o| o.get("name"))
            .and_then(Value::as_str)
            .map(|v| v == os_name())
            .unwrap_or(true);

        if !matches_os {
            continue;
        }

        match rule.get("action").and_then(Value::as_str) {
            Some("disallow") => return false,
            Some("allow") => {
                allowed = true;
                saw_allow = true;
            }
            _ => {}
        }
    }

    if saw_allow {
        allowed
    } else {
        true
    }
}

fn substitute(
    mut s: String,
    game_dir: &Path,
    assets_dir: &Path,
    asset_index: &str,
    natives_dir: &Path,
    instance: &str,
    version: &str,
    username: &str,
    uuid: &str,
    access_token: &str,
    user_type: &str,
) -> String {
    let game_dir_s = game_dir.to_string_lossy().to_string();
    let assets_dir_s = assets_dir.to_string_lossy().to_string();
    let natives_dir_s = natives_dir.to_string_lossy().to_string();
    let library_dir_s = game_dir
        .join("libraries")
        .to_string_lossy()
        .to_string();

    let replacements = [
        ("${auth_player_name}", username),
        ("${auth_uuid}", uuid),
        ("${auth_access_token}", access_token),
        ("${user_type}", user_type),
        ("${version_name}", version),
        ("${version_type}", "release"),
        ("${assets_root}", assets_dir_s.as_str()),
        ("${assets_index_name}", asset_index),
        ("${game_directory}", game_dir_s.as_str()),
        ("${natives_directory}", natives_dir_s.as_str()),
        ("${library_directory}", library_dir_s.as_str()),
        (
            "${classpath_separator}",
            if cfg!(target_os = "windows") {
                ";"
            } else {
                ":"
            },
        ),
        ("${launcher_name}", "BlockPilot"),
        ("${launcher_version}", "0.1.0"),
        ("${clientid}", ""),
        ("${auth_xuid}", ""),
        ("${instance_name}", instance),
    ];

    for (from, to) in replacements {
        s = s.replace(from, to);
    }

    s
}

fn collect_args(
    value: &Value,
    game_dir: &Path,
    assets_dir: &Path,
    asset_index: &str,
    natives_dir: &Path,
    instance: &str,
    version: &str,
    username: &str,
    uuid: &str,
    access_token: &str,
    user_type: &str,
) -> Vec<String> {
    let mut out = Vec::new();

    let Some(args) = value.as_array() else {
        return out;
    };

    for entry in args {
        if !rules_allow(entry) {
            continue;
        }

        let raw = entry.get("value").unwrap_or(entry);

        match raw {
            Value::String(s) => {
                out.push(substitute(
                    s.clone(),
                    game_dir,
                    assets_dir,
                    asset_index,
                    natives_dir,
                    instance,
                    version,
                    username,
                    uuid,
                    access_token,
                    user_type,
                ));
            }

            Value::Array(values) => {
                for v in values.iter().filter_map(Value::as_str) {
                    out.push(substitute(
                        v.to_string(),
                        game_dir,
                        assets_dir,
                        asset_index,
                        natives_dir,
                        instance,
                        version,
                        username,
                        uuid,
                        access_token,
                        user_type,
                    ));
                }
            }

            _ => {}
        }
    }

    out
}

fn maven_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();

    if parts.len() < 3 {
        return None;
    }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];

    let classifier = parts
        .get(3)
        .map(|c| format!("-{}", c))
        .unwrap_or_default();

    Some(format!(
        "{}/{}/{}/{}-{}{}.jar",
        group, artifact, version, artifact, version, classifier
    ))
}

fn emit_progress(
    app: &tauri::AppHandle,
    instance: &str,
    stage: &str,
    detail: &str,
) {
    let _ = app.emit(
        "launch-progress",
        json!({
            "instance": instance,
            "stage": stage,
            "detail": detail
        }),
    );
}

async fn ensure_authlib_injector(
    client: &reqwest::Client,
    root: &Path,
) -> Result<PathBuf, String> {
    let path = root.join("authlib-injector.jar");

    download_file(
        client,
        AUTHLIB_INJECTOR_URL,
        &path,
        None,
    )
    .await?;

    Ok(path)
}

fn java_command() -> &'static str {
    "java"
}

async fn forge_version(
    client: &reqwest::Client,
    mc: &str,
    requested: Option<&str>,
) -> Result<String, String> {
    if let Some(v) = requested.filter(|v| !v.trim().is_empty()) {
        return Ok(v.to_string());
    }

    let promotions = fetch_json(client, FORGE_PROMOTIONS).await?;

    let map = promotions
        .get("promos")
        .and_then(Value::as_object)
        .ok_or("Forge promotions response is malformed")?;

    let recommended = format!("{}-recommended", mc);
    let latest = format!("{}-latest", mc);

    if let Some(v) = map.get(&recommended).and_then(Value::as_str) {
        return Ok(v.to_string());
    }

    if let Some(v) = map.get(&latest).and_then(Value::as_str) {
        return Ok(v.to_string());
    }

    Err(format!(
        "No Forge build is published for Minecraft {}",
        mc
    ))
}

fn neoforge_prefix(mc: &str) -> String {
    // NeoForge versions use the Minecraft minor/patch pair:
    // 1.20.2 -> 20.2, 1.20.4 -> 20.4, 1.21.1 -> 21.1.
    let parts: Vec<&str> = mc.split('.').collect();

    match parts.as_slice() {
        [major, minor, patch, ..] => {
            let major = major.strip_prefix('1').unwrap_or(major);
            format!("{}.{}", if major.is_empty() { minor } else { major }, patch)
        }
        [major, minor, ..] => {
            let major = major.strip_prefix('1').unwrap_or(major);
            format!("{}.0", if major.is_empty() { minor } else { major })
        }
        _ => mc.to_string(),
    }
}

async fn neoforge_version(
    client: &reqwest::Client,
    mc: &str,
    requested: Option<&str>,
) -> Result<String, String> {
    if let Some(v) = requested.filter(|v| !v.trim().is_empty()) {
        return Ok(v.to_string());
    }

    let xml = fetch_text(client, NEOFORGE_METADATA).await?;
    let prefix = neoforge_prefix(mc);

    let mut versions = Vec::new();

    for part in xml.split("<version>").skip(1) {
        if let Some(v) = part.split("</version>").next() {
            if v.starts_with(&(prefix.clone() + ".")) {
                versions.push(v.to_string());
            }
        }
    }

    versions.sort_by(|a, b| {
        let parse = |s: &str| {
            s.split('.')
                .map(|p| p.parse::<u64>().unwrap_or(0))
                .collect::<Vec<_>>()
        };

        parse(a).cmp(&parse(b))
    });

    versions
        .pop()
        .ok_or_else(|| {
            format!(
                "No NeoForge build is published for Minecraft {}",
                mc
            )
        })
}

async fn install_loader(
    client: &reqwest::Client,
    game_dir: &Path,
    mc: &str,
    loader: &str,
    requested: Option<&str>,
    app: &tauri::AppHandle,
    instance: &str,
) -> Result<String, String> {
    let (url, version, label) = match loader {
        "forge" => {
            let v = forge_version(client, mc, requested).await?;

            (
                format!(
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{}-{}/forge-{}-{}-installer.jar",
                    mc, v, mc, v
                ),
                v,
                "Forge",
            )
        }

        "neoforge" => {
            let v = neoforge_version(client, mc, requested).await?;

            (
                format!(
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
                    v, v
                ),
                v,
                "NeoForge",
            )
        }

        _ => {
            return Err(format!("Unknown loader {}", loader));
        }
    };

    let installer_dir = game_dir.join(".blockpilot");

    fs::create_dir_all(&installer_dir)
        .map_err(|e| e.to_string())?;

    let installer = installer_dir.join(format!(
        "{}-installer.jar",
        loader
    ));

    emit_progress(
        app,
        instance,
        "loader",
        &format!("Installing {} {}…", label, version),
    );

    download_file(
        client,
        &url,
        &installer,
        None,
    )
    .await?;

    let output = Command::new(java_command())
        .arg("-jar")
        .arg(&installer)
        .arg("--installClient")
        .arg(game_dir)
        .output()
        .map_err(|e| {
            format!(
                "Could not run {} installer: {}. Make sure Java is installed.",
                label, e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let message = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };

        return Err(format!(
            "{} installer failed ({}): {}",
            label,
            output.status,
            message
        ));
    }

    Ok(version)
}

async fn find_loader_profile(
    game_dir: &Path,
    loader: &str,
    loader_version: &str,
) -> Result<Value, String> {
    let versions = game_dir.join("versions");
    let mut candidates = Vec::new();

    if versions.exists() {
        for entry in fs::read_dir(&versions)
            .map_err(|e| e.to_string())?
        {
            let entry = entry.map_err(|e| e.to_string())?;

            if !entry
                .file_type()
                .map_err(|e| e.to_string())?
                .is_dir()
            {
                continue;
            }

            let name = entry
                .file_name()
                .to_string_lossy()
                .to_lowercase();

            let wanted = if loader == "forge" {
                "forge"
            } else {
                "neoforge"
            };

            if name.contains(wanted)
                && (
                    loader_version.is_empty()
                        || name.contains(&loader_version.to_lowercase())
                )
            {
                candidates.push(entry.path());
            }
        }
    }

    candidates.sort();
    candidates.reverse();

    for dir in candidates {
        let name = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        let json_path = dir.join(format!("{}.json", name));

        if json_path.exists() {
            let bytes = fs::read(json_path)
                .map_err(|e| e.to_string())?;

            return serde_json::from_slice(&bytes)
                .map_err(|e| e.to_string());
        }
    }

    Err(format!(
        "{} installed but no generated launcher profile was found",
        loader
    ))
}

async fn version_meta(
    client: &reqwest::Client,
    manifest: &Value,
    id: &str,
) -> Result<Value, String> {
    let url = manifest
        .get("versions")
        .and_then(Value::as_array)
        .and_then(|versions| {
            versions.iter().find(|v| {
                v.get("id")
                    .and_then(Value::as_str)
                    == Some(id)
            })
        })
        .and_then(|v| v.get("url"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("Version metadata not found for {}", id)
        })?;

    fetch_json(client, url).await
}

async fn resolve_inherits(
    client: &reqwest::Client,
    manifest: &Value,
    mut meta: Value,
) -> Result<Value, String> {
    let mut chain = Vec::new();

    loop {
        let parent = meta
            .get("inheritsFrom")
            .and_then(Value::as_str)
            .map(str::to_string);

        chain.push(meta);

        let Some(parent_id) = parent else {
            break;
        };

        meta = version_meta(client, manifest, &parent_id).await?;
    }

    let mut merged = chain
        .pop()
        .unwrap_or_else(|| json!({}));

    while let Some(child) = chain.pop() {
        for key in [
            "id",
            "mainClass",
            "type",
            "releaseTime",
            "time",
            "assetIndex",
            "javaVersion",
            "minecraftArguments",
        ] {
            if let Some(v) = child.get(key) {
                merged[key] = v.clone();
            }
        }

        let mut libs = merged
            .get("libraries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if let Some(extra) =
            child.get("libraries").and_then(Value::as_array)
        {
            libs.extend(extra.iter().cloned());
        }

        if !libs.is_empty() {
            merged["libraries"] = Value::Array(libs);
        }

        let mut merged_args = merged
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        if let Some(args) = child.get("arguments") {
            for kind in ["jvm", "game"] {
                let mut arr = merged_args
                    .get(kind)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();

                if let Some(extra) =
                    args.get(kind).and_then(Value::as_array)
                {
                    arr.extend(extra.iter().cloned());
                }

                if !arr.is_empty() {
                    merged_args[kind] = Value::Array(arr);
                }
            }

            merged["arguments"] = merged_args;
        }
    }

    Ok(merged)
}

async fn download_libraries_and_natives(
    client: &reqwest::Client,
    meta: &Value,
    game_dir: &Path,
    natives_dir: &Path,
    classpath: &mut Vec<String>,
    app: &tauri::AppHandle,
    instance: &str,
) -> Result<(), String> {
    let libraries_dir = game_dir.join("libraries");

    fs::create_dir_all(&libraries_dir)
        .map_err(|e| e.to_string())?;

    fs::create_dir_all(natives_dir)
        .map_err(|e| e.to_string())?;

    let libs = meta
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let total = libs.len();

    let native_key = if cfg!(target_os = "windows") {
        "natives-windows"
    } else if cfg!(target_os = "macos") {
        "natives-osx"
    } else {
        "natives-linux"
    };

    for (i, lib) in libs.iter().enumerate() {
        if !rules_allow(lib) {
            continue;
        }

        if i % 10 == 0 {
            emit_progress(
                app,
                instance,
                "libraries",
                &format!(
                    "Preparing libraries… {}/{}",
                    i + 1,
                    total
                ),
            );
        }

        if let Some(artifact) = lib
            .get("downloads")
            .and_then(|d| d.get("artifact"))
        {
            if let (Some(url), Some(path)) = (
                artifact.get("url").and_then(Value::as_str),
                artifact.get("path").and_then(Value::as_str),
            ) {
                let file = libraries_dir.join(path);

                download_file(
                    client,
                    url,
                    &file,
                    artifact
                        .get("sha1")
                        .and_then(Value::as_str),
                )
                .await?;

                classpath.push(
                    file.to_string_lossy().to_string()
                );
            }
        } else if let Some(name) =
            lib.get("name").and_then(Value::as_str)
        {
            if let Some(rel) = maven_path(name) {
                let base = lib
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or(
                        "https://libraries.minecraft.net",
                    );

                let file = libraries_dir.join(&rel);

                let url = format!(
                    "{}/{}",
                    base.trim_end_matches('/'),
                    rel
                );

                download_file(
                    client,
                    &url,
                    &file,
                    None,
                )
                .await?;

                classpath.push(
                    file.to_string_lossy().to_string()
                );
            }
        }

        if let Some(native) = lib
            .get("downloads")
            .and_then(|d| d.get("classifiers"))
            .and_then(|c| c.get(native_key))
        {
            if let (Some(url), Some(path)) = (
                native.get("url").and_then(Value::as_str),
                native.get("path").and_then(Value::as_str),
            ) {
                let file = libraries_dir.join(path);

                download_file(
                    client,
                    url,
                    &file,
                    native
                        .get("sha1")
                        .and_then(Value::as_str),
                )
                .await?;

                let archive =
                    fs::File::open(&file)
                        .map_err(|e| e.to_string())?;

                let mut zip =
                    zip::ZipArchive::new(archive)
                        .map_err(|e| e.to_string())?;

                for index in 0..zip.len() {
                    let mut entry = zip
                        .by_index(index)
                        .map_err(|e| e.to_string())?;

                    let name = entry.name().to_string();

                    if name.starts_with("META-INF/")
                        || name.ends_with('/')
                    {
                        continue;
                    }

                    let destination =
                        natives_dir.join(&name);

                    if let Some(parent) =
                        destination.parent()
                    {
                        fs::create_dir_all(parent)
                            .map_err(|e| e.to_string())?;
                    }

                    let mut buf = Vec::new();

                    entry
                        .read_to_end(&mut buf)
                        .map_err(|e| e.to_string())?;

                    fs::write(destination, buf)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    Ok(())
}

async fn download_assets(
    client: &reqwest::Client,
    meta: &Value,
    assets_dir: &Path,
    app: &tauri::AppHandle,
    instance: &str,
) -> Result<String, String> {
    let asset_index = meta
        .get("assetIndex")
        .ok_or("Asset index missing")?;

    let asset_id = asset_index
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("legacy")
        .to_string();

    let index_url = asset_index
        .get("url")
        .and_then(Value::as_str)
        .ok_or("Asset index URL missing")?;

    let index_file = assets_dir
        .join("indexes")
        .join(format!("{}.json", asset_id));

    download_file(
        client,
        index_url,
        &index_file,
        asset_index
            .get("sha1")
            .and_then(Value::as_str),
    )
    .await?;

    let assets_json: Value =
        serde_json::from_slice(
            &fs::read(&index_file)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

    if let Some(objects) =
        assets_json.get("objects").and_then(Value::as_object)
    {
        let total = objects.len();

        for (i, object) in objects.values().enumerate() {
            if i % 100 == 0 {
                emit_progress(
                    app,
                    instance,
                    "assets",
                    &format!(
                        "Downloading assets… {}/{}",
                        i, total
                    ),
                );
            }

            let Some(hash) =
                object.get("hash").and_then(Value::as_str)
            else {
                continue;
            };

            if hash.len() < 2 {
                continue;
            }

            let prefix = &hash[0..2];

            let path = assets_dir
                .join("objects")
                .join(prefix)
                .join(hash);

            let url = format!(
                "https://resources.download.minecraft.net/{}/{}",
                prefix,
                hash
            );

            download_file(
                client,
                &url,
                &path,
                Some(hash),
            )
            .await?;
        }
    }

    Ok(asset_id)
}

async fn fetch_fabric_meta(
    client: &reqwest::Client,
    mc_version: &str,
    loader_version: &str,
) -> Result<Value, String> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        mc_version, loader_version
    );
    fetch_json(client, &url).await
}

#[tauri::command]
async fn launch_instance(
    app: tauri::AppHandle,
    instance: String,
) -> Result<String, String> {
    if instance.trim().is_empty() {
        return Err(
            "No Minecraft instance selected".into()
        );
    }

    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    let instance_dir = root
        .join("instances")
        .join(safe_name(&instance));

    if !instance_dir.exists() {
        return Err(format!(
            "Instance '{}' does not exist. Create it first.",
            instance
        ));
    }

    let (mc_version, loader, loader_version_pref) =
        management::read_profile(&instance_dir);

    let profile_path =
        instance_dir.join("profile.json");

    let profile: Value =
        serde_json::from_slice(
            &fs::read(&profile_path)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| {
            format!(
                "Invalid instance profile: {}",
                e
            )
        })?;

    let game_dir = instance_dir.join("game");
    let libraries_dir = game_dir.join("libraries");
    let versions_dir = game_dir.join("versions");
    let assets_dir = game_dir.join("assets");
    let natives_dir = instance_dir.join("natives");

    for dir in [
        &game_dir,
        &libraries_dir,
        &versions_dir,
        &assets_dir,
        &natives_dir,
    ] {
        fs::create_dir_all(dir)
            .map_err(|e| e.to_string())?;
    }

    let client = http_client()?;

    emit_progress(
        &app,
        &instance,
        "manifest",
        "Checking Minecraft version…",
    );

    let manifest =
        fetch_json(&client, VERSION_MANIFEST).await?;

    let vanilla_meta =
        version_meta(&client, &manifest, &mc_version)
            .await?;

    let version_dir =
        versions_dir.join(&mc_version);

    fs::create_dir_all(&version_dir)
        .map_err(|e| e.to_string())?;

    let client_jar = version_dir.join(
        format!("{}.jar", mc_version),
    );

    let client_download = vanilla_meta
        .get("downloads")
        .and_then(|d| d.get("client"))
        .ok_or("Minecraft client download is missing")?;

    let client_url = client_download
        .get("url")
        .and_then(Value::as_str)
        .ok_or("Client URL missing")?;

    emit_progress(
        &app,
        &instance,
        "client",
        &format!(
            "Downloading Minecraft {} client…",
            mc_version
        ),
    );

    download_file(
        &client,
        client_url,
        &client_jar,
        client_download
            .get("sha1")
            .and_then(Value::as_str),
    )
    .await?;

    let mut meta = vanilla_meta.clone();

    let mut main_class = meta
        .get("mainClass")
        .and_then(Value::as_str)
        .ok_or("Minecraft main class missing")?
        .to_string();

    let mut installed_loader_version =
        loader_version_pref.clone().unwrap_or_default();

    match loader.as_str() {
        "vanilla" => {}

        "fabric" => {
            emit_progress(
                &app,
                &instance,
                "fabric",
                "Installing Fabric loader…",
            );

            let lv =
                if installed_loader_version.is_empty() {
                    versions::list_fabric_loaders(
                        mc_version.clone(),
                    )
                    .await?
                    .into_iter()
                    .next()
                    .map(|l| l.version)
                    .unwrap_or_default()
                } else {
                    installed_loader_version.clone()
                };

            if lv.is_empty() {
                return Err(
                    "No Fabric loader is available for this Minecraft version."
                        .into(),
                );
            }

            match fetch_fabric_meta(
                &client,
                &mc_version,
                &lv,
            )
            .await
            {
                Ok(profile) => {
                    meta = profile;
                    installed_loader_version = lv;
                }

                Err(e) => {
                    return Err(format!(
                        "Failed to fetch Fabric metadata: {}",
                        e
                    ));
                }
            }
        }

        "forge" | "neoforge" => {
            // Reuse an already-installed loader instead of running the
            // installer on every launch. If no matching profile exists,
            // install it and then load the generated profile.
            let existing = find_loader_profile(
                &game_dir,
                &loader,
                &loader_version_pref.clone().unwrap_or_default(),
            )
            .await;

            match existing {
                Ok(profile) => {
                    meta = profile;
                    installed_loader_version = loader_version_pref.clone().unwrap_or_default();
                }
                Err(_) => {
                    let version = install_loader(
                        &client,
                        &game_dir,
                        &mc_version,
                        &loader,
                        if loader_version_pref
                            .as_deref()
                            .unwrap_or("")
                            .is_empty()
                        {
                            None
                        } else {
                            loader_version_pref.as_deref()
                        },
                        &app,
                        &instance,
                    )
                    .await?;

                    installed_loader_version = version;
                    meta = find_loader_profile(
                        &game_dir,
                        &loader,
                        &installed_loader_version,
                    )
                    .await?;
                }
            }

            if let Some(mc) =
                meta.get("mainClass").and_then(Value::as_str)
            {
                main_class = mc.to_string();
            }
        }

        other => {
            return Err(format!(
                "Unsupported loader '{}'. Use vanilla, fabric, forge, or neoforge.",
                other
            ));
        }
    }

    if meta
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .is_some()
    {
        meta = resolve_inherits(
            &client,
            &manifest,
            meta,
        )
        .await?;

        if let Some(mc) =
            meta.get("mainClass").and_then(Value::as_str)
        {
            main_class = mc.to_string();
        }
    }

    let mut classpath = Vec::new();

    download_libraries_and_natives(
        &client,
        &meta,
        &game_dir,
        &natives_dir,
        &mut classpath,
        &app,
        &instance,
    )
    .await?;

    classpath.push(
        client_jar.to_string_lossy().to_string()
    );

    let asset_id = download_assets(
        &client,
        &meta,
        &assets_dir,
        &app,
        &instance,
    )
    .await?;

    let account =
        accounts::get_active_account(&app);

    let (
        username,
        uuid,
        access_token,
        user_type,
    ) = match &account {
        Some(a) if a.kind == "microsoft" => (
            a.username.clone(),
            a.uuid.clone(),
            a.access_token.clone().unwrap_or_default(),
            "msa".to_string(),
        ),

        Some(a) if a.kind == "elyby" => (
            a.username.clone(),
            a.uuid.clone(),
            a.access_token.clone().unwrap_or_default(),
            "legacy".to_string(),
        ),

        _ => (
            "Steve".to_string(),
            offline_uuid("Steve"),
            "".to_string(),
            "legacy".to_string(),
        ),
    };

    let separator = if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    };

    let ram = profile
        .get("ram_mb")
        .and_then(Value::as_u64)
        .filter(|v| *v >= 512)
        .unwrap_or(4096);

    let mut jvm_args = vec![
        format!("-Xmx{}M", ram),
        format!(
            "-Djava.library.path={}",
            natives_dir.to_string_lossy()
        ),
        "-XX:+IgnoreUnrecognizedVMOptions".to_string(),
        "--add-opens=java.base/java.io=ALL-UNNAMED"
            .to_string(),
    ];

    if let Some(custom_jvm) =
        profile.get("jvm_args").and_then(Value::as_array)
    {
        for value in custom_jvm
            .iter()
            .filter_map(Value::as_str)
        {
            jvm_args.push(substitute(
                value.to_string(),
                &game_dir,
                &assets_dir,
                &asset_id,
                &natives_dir,
                &instance,
                &mc_version,
                &username,
                &uuid,
                &access_token,
                &user_type,
            ));
        }
    }

    if let Some(jvm) = meta
        .get("arguments")
        .and_then(|a| a.get("jvm"))
    {
        jvm_args.extend(collect_args(
            jvm,
            &game_dir,
            &assets_dir,
            &asset_id,
            &natives_dir,
            &instance,
            &mc_version,
            &username,
            &uuid,
            &access_token,
            &user_type,
        ));
    }

    if let Some(a) = &account {
        if a.kind == "elyby" {
            emit_progress(
                &app,
                &instance,
                "identity",
                "Preparing ely.by authentication…",
            );

            let injector =
                ensure_authlib_injector(
                    &client,
                    &root,
                )
                .await?;

            jvm_args.push(format!(
                "-javaagent:{}",
                injector.to_string_lossy()
            ));
        }
    }

    let mut args = jvm_args;

    args.extend([
        "-cp".into(),
        classpath.join(separator),
        main_class.clone(),
    ]);

    if let Some(game) = meta
        .get("arguments")
        .and_then(|a| a.get("game"))
    {
        args.extend(collect_args(
            game,
            &game_dir,
            &assets_dir,
            &asset_id,
            &natives_dir,
            &instance,
            &mc_version,
            &username,
            &uuid,
            &access_token,
            &user_type,
        ));
    } else if let Some(legacy) =
        meta.get("minecraftArguments")
            .and_then(Value::as_str)
    {
        for value in legacy.split_whitespace() {
            args.push(substitute(
                value.to_string(),
                &game_dir,
                &assets_dir,
                &asset_id,
                &natives_dir,
                &instance,
                &mc_version,
                &username,
                &uuid,
                &access_token,
                &user_type,
            ));
        }
    }

    if !args.iter().any(|a| a == "--username") {
        args.extend([
            "--username".into(),
            username.clone(),
            "--uuid".into(),
            uuid.clone(),
            "--accessToken".into(),
            access_token.clone(),
            "--userType".into(),
            user_type.clone(),
            "--versionType".into(),
            "BlockPilot".into(),
        ]);
    }

    emit_progress(
        &app,
        &instance,
        "starting",
        &format!(
            "Starting Minecraft {}{}…",
            mc_version,
            if loader != "vanilla" {
                format!(
                    " with {} {}",
                    loader,
                    installed_loader_version
                )
            } else {
                String::new()
            }
        ),
    );

    let log_path = instance_dir.join("minecraft.log");
    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|e| format!("Could not create Minecraft log: {}", e))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("Could not prepare Minecraft log: {}", e))?;

    let mut child = Command::new(java_command())
        .args(&args)
        .current_dir(&game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err))
        .spawn()
        .map_err(|e| {
            format!("Could not start Java: {}", e)
        })?;

    let pid = child.id();

    tauri::async_runtime::spawn(async move {
        let _ = child.wait();
    });

    Ok(format!(
        "Minecraft {} launched for '{}' as {} (PID {}). Log: {}",
        mc_version,
        instance,
        username,
        pid,
        log_path.to_string_lossy()
    ))
}

#[tauri::command]
fn runtime_info() -> Result<String, String> {
    match Command::new(java_command())
        .arg("-version")
        .output()
    {
        Ok(output) => {
            let line = String::from_utf8_lossy(
                &output.stderr,
            )
            .lines()
            .next()
            .unwrap_or("Java not installed")
            .to_string();

            Ok(line)
        }

        Err(_) => Err(
            "Java not found".to_string()
        ),
    }
}

#[tauri::command]
fn launcher_data_dir(
    app: tauri::AppHandle,
) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())?;

    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
fn open_instance_folder(
    app: tauri::AppHandle,
    instance: String,
) -> Result<(), String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    let dir = root
        .join("instances")
        .join(safe_name(&instance));

    fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct JavaRuntime {
    path: String,
    version: String,
}

#[tauri::command]
fn list_java_runtimes()
    -> Result<Vec<JavaRuntime>, String>
{
    let mut found = Vec::new();

    if let Ok(output) = Command::new(java_command())
        .arg("-version")
        .output()
    {
        let line = String::from_utf8_lossy(
            &output.stderr,
        )
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

        if !line.is_empty() {
            found.push(JavaRuntime {
                path: java_command().to_string(),
                version: line,
            });
        }
    }

    Ok(found)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            launch_instance,
            runtime_info,
            launcher_data_dir,
            open_instance_folder,
            list_java_runtimes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
