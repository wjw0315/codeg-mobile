use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

// 当前打开服务页的 token，供每次导航都会执行的 init script 拉取后装拦截
static ACTIVE_TOKEN: Mutex<Option<String>> = Mutex::new(None);

// init script 挂在主窗口上，每次导航（新文档）都会执行：
// 捕获页面 JS 错误回传 logcat；从 Rust 拉取当前 token 后拦截 fetch/XHR 加 Authorization 头
const INIT_SCRIPT: &str = r#"
(function() {
    var __logErr = function(m) {
        try {
            if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
                window.__TAURI__.core.invoke('log_js_error', { msg: String(m) });
            }
        } catch(e) {}
    };
    window.addEventListener('error', function(e) {
        __logErr('error: ' + (e.message || e.error) + ' @ ' + (e.filename || '') + ':' + (e.lineno || 0));
    });
    window.addEventListener('unhandledrejection', function(e) {
        var r = e.reason;
        __logErr('unhandledrejection: ' + ((r && (r.stack || r.message)) || r));
    });
    var __origErr = console.error;
    console.error = function() {
        try {
            __logErr('console.error: ' + Array.prototype.map.call(arguments, function(a) {
                try { return typeof a === 'object' ? JSON.stringify(a) : String(a); } catch(e) { return String(a); }
            }).join(' '));
        } catch(e) {}
        return __origErr.apply(this, arguments);
    };
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
        window.__TAURI__.core.invoke('get_active_token').then(function(t) {
            if (t) {
                try { localStorage.setItem('codeg-token', t); } catch(e) {}
                var origFetch = window.fetch;
                if (origFetch) {
                    window.fetch = function(input, init) {
                        init = init || {};
                        init.headers = init.headers || {};
                        if (!init.headers.Authorization) {
                            init.headers.Authorization = 'Bearer ' + t;
                        }
                        return origFetch(input, init);
                    };
                }
                var origOpen = XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open = function(method, url) {
                    this.setRequestHeader('Authorization', 'Bearer ' + t);
                    return origOpen.apply(this, arguments);
                };
            }
        }).catch(function(){});
    }
})();
"#;

// 插件把 init script 挂到所有 webview——每次导航的新文档都会执行，
// 这样 navigate 到服务页后仍能捕获 JS 错误、拉取 token 装拦截。
// （wry 的 initialization_script 只在建窗时可用，用插件可挂在 config 自动创建的窗口上，
// 避免手动建窗与 Android Activity 生命周期冲突导致启动崩溃。）
struct InitScriptPlugin;

impl<R: tauri::Runtime> tauri::plugin::Plugin<R> for InitScriptPlugin {
    fn name(&self) -> &'static str {
        "init-script"
    }
    fn initialization_script(&self) -> Option<String> {
        Some(INIT_SCRIPT.to_string())
    }
}

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
fn log_js_error(msg: String) {
    // 诊断用：WebView 内页面 JS 错误经 invoke 回传，打 logcat（RustStdoutStderr）
    eprintln!("[JS-ERR] {msg}");
}

#[tauri::command]
fn get_active_token() -> Option<String> {
    ACTIVE_TOKEN.lock().ok().and_then(|g| g.clone())
}

#[tauri::command]
fn open_service(app: AppHandle, name: String) -> Result<(), String> {
    let cfg = load_config(&app);
    let svc = cfg
        .services
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("服务不存在: {name}"))?
        .clone();

    let url = format!("{}{}", svc.url.trim_end_matches('/'), "/");
    let token = svc.token.clone();
    let parsed_url = url.parse::<tauri::Url>().map_err(|e| e.to_string())?;

    // 主窗口内导航，不建第二个 WebView——wry 0.57.2 Android 的多窗口建窗
    // 在事件循环线程处理 GetWebViewVersion 时存在竞态（rx 已 drop 时
    // tx.send().unwrap() panic），事件循环 panic 后整个 App 失去响应。
    let webview = app.get_webview_window("main").ok_or("主窗口不存在")?;

    // 把当前 token 存入全局，供主窗口 init script（每次导航都执行）拉取后装拦截。
    // 不在此处 eval——navigate 后是新文档上下文，eval 的钩子会丢失。
    if let Ok(mut g) = ACTIVE_TOKEN.lock() {
        *g = Some(token);
    }
    webview.navigate(parsed_url).map_err(|e| e.to_string())?;

    Ok(())
}

// Android/iOS 移动端构建只编译 lib 目标，入口宏必须标在 run() 上才能编进 .so
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(InitScriptPlugin)
        .invoke_handler(tauri::generate_handler![
            list_services,
            save_service,
            delete_service,
            open_service,
            log_js_error,
            get_active_token
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            // 在服务页按返回：导航回服务列表，不退出 App
            let on_service_page = app_handle
                .get_webview_window("main")
                .and_then(|w| w.url().ok())
                .map(|u| {
                    let host = u.host_str().unwrap_or("");
                    !host.contains("tauri.localhost")
                        && (u.scheme() == "http" || u.scheme() == "https")
                })
                .unwrap_or(false);
            if on_service_page {
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.navigate(
                    "https://tauri.localhost/index.html"
                        .parse::<tauri::Url>()
                        .expect("valid asset url"),
                );
                }
                api.prevent_exit();
            }
        }
    });
}
