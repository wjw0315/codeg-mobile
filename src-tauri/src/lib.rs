use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub url: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    pub services: Vec<ServiceConfig>,
}

fn config_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("app config dir not available");
    dir.join("services.json")
}

fn load_config(app: &AppHandle) -> ConfigFile {
    let path = config_path(app);
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => ConfigFile::default(),
        }
    } else {
        ConfigFile::default()
    }
}

fn save_config(app: &AppHandle, config: &ConfigFile) -> Result<(), String> {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_services(app: AppHandle) -> Vec<ServiceConfig> {
    load_config(&app).services
}

#[tauri::command]
fn save_service(app: AppHandle, config: ServiceConfig) -> Result<(), String> {
    let mut cfg = load_config(&app);
    if let Some(existing) = cfg.services.iter_mut().find(|s| s.name == config.name) {
        *existing = config.clone();
    } else {
        cfg.services.push(config);
    }
    save_config(&app, &cfg)
}

#[tauri::command]
fn delete_service(app: AppHandle, name: String) -> Result<(), String> {
    let mut cfg = load_config(&app);
    cfg.services.retain(|s| s.name != name);
    save_config(&app, &cfg)
}

#[tauri::command]
fn open_service(app: AppHandle, name: String) -> Result<(), String> {
    let cfg = load_config(&app);
    let svc = cfg
        .services
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("服务不存在: {name}"))?;

    let url = format!("{}{}", svc.url.trim_end_matches('/'), "/");
    let token = svc.token.clone();
    let window_label = format!("codeg-{}", name);

    // 注入 init script：拦截 fetch/XHR 自动加 Authorization: Bearer 头，兼容 localStorage
    let init_script = format!(
        r#"
        (function() {{
            var t = {:?};
            if (!t) return;
            try {{ localStorage.setItem('codeg-token', t); }} catch(e) {{}}
            var origFetch = window.fetch;
            if (origFetch) {{
                window.fetch = function(input, init) {{
                    init = init || {{}};
                    init.headers = init.headers || {{}};
                    if (!init.headers.Authorization) {{
                        init.headers.Authorization = 'Bearer ' + t;
                    }}
                    return origFetch(input, init);
                }};
            }}
            var origOpen = XMLHttpRequest.prototype.open;
            XMLHttpRequest.prototype.open = function(method, url) {{
                this.setRequestHeader('Authorization', 'Bearer ' + t);
                return origOpen.apply(this, arguments);
            }};
        }})();
        "#,
        token
    );

    let parsed_url = url.parse::<tauri::Url>().map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(&app, &window_label, WebviewUrl::External(parsed_url))
        .title(&format!("Codeg - {}", svc.name))
        .inner_size(390.0, 844.0)
        .resizable(true)
        .initialization_script(init_script)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

// Android/iOS 移动端构建只编译 lib 目标，入口宏必须标在 run() 上才能编进 .so
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_services,
            save_service,
            delete_service,
            open_service
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
