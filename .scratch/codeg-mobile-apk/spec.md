# Spec: Codeg Android APK (WebView 壳封装)

Status: ready-for-agent

## Problem Statement

codeg 是一个部署在内网的 Web 服务（`http://100.94.107.28:3080/`），所有 `/api/*` 端点要求 `Authorization: Bearer <token>` 头鉴权。用户希望在安卓手机上直接使用 codeg，而不是每次通过浏览器手动输入地址和 token。需要一个安卓 APK，在 App 内填写连接地址和 token，保存后以 WebView 方式加载 codeg 页面，并自动为所有 API 请求注入鉴权头。

## Solution

用 Tauri 2 构建一个 WebView 壳 APK（方案 B），不修改 codeg 源码。App 启动后展示服务列表，用户可添加、编辑、删除多套服务配置（每套含名称、连接地址、token）。点击某个服务时，Tauri 创建一个 WebView 窗口加载该服务的 URL，并通过 `initialization_script` 注入 JS，在页面加载前拦截 `window.fetch` 和 `XMLHttpRequest.prototype.open`，自动为所有请求添加 `Authorization: Bearer <token>` 头，同时将 token 写入 `localStorage` 以兼容 codeg 前端自身的鉴权逻辑。

## User Stories

1. 作为用户，我想要在 App 内填写 codeg 的连接地址，以便在安卓手机上直接访问 codeg 服务
2. 作为用户，我想要在 App 内填写 Bearer token，以便 codeg 的 API 请求能通过鉴权
3. 作为用户，我想要保存多套服务配置，以便在不同环境（内网/测试/生产）之间切换
4. 作为用户，我想要给每套服务起一个名称，以便在服务列表中快速识别
5. 作为用户，我想要在服务列表中查看已保存的所有服务，以便选择要连接的服务
6. 作为用户，我想要添加新的服务配置，以便接入新的 codeg 实例
7. 作为用户，我想要编辑已有服务的名称、地址或 token，以便在服务信息变更时更新配置
8. 作为用户，我想要删除不再使用的服务配置，以便保持列表整洁
9. 作为用户，我想要点击某个服务后直接打开 codeg 界面，以便立即使用
10. 作为用户，我想要 App 自动为 codeg 的所有 API 请求注入 Bearer token，以便无需在浏览器中手动操作
11. 作为用户，我想要 token 同时写入 localStorage，以便 codeg 前端自身的鉴权逻辑能正常工作
12. 作为用户，我想要 App 以全屏 WebView 方式加载 codeg，以便获得接近原生 App 的体验
13. 作为用户，我想要 App 窗口可调整大小，以便在不同屏幕尺寸的安卓设备上灵活使用
14. 作为用户，我想要 APK 能在 arm64-v8a 架构的安卓设备上安装运行，以便在主流安卓手机上使用
15. 作为用户，我想要 App 启动后不闪退，以便稳定使用
16. 作为开发者，我想要通过模拟器验证 APK 启动后不闪退，以便确认修复有效

## Implementation Decisions

- **技术选型：Tauri 2**。Rust 后端 + 系统 WebView，前端纯静态 HTML/CSS/JS，无前端构建步骤。选择 Tauri 而非 Capacitor 或原生 WebView 壳，是因为 Tauri 2 提供 `initialization_script` API 可以在 WebView 页面加载前注入 JS，精确控制请求拦截逻辑。
- **不修改 codeg 源码**。方案 B 的核心约束：codeg 作为已有 Web 服务保持不变，APK 仅作为外壳容器。所有鉴权逻辑在壳层注入，不触碰 codeg 服务端代码。
- **配置存储**：服务配置以 JSON 文件（`services.json`）存储在 Tauri 应用配置目录（`app_config_dir()`）。每套服务包含三个字段：`name`（名称）、`url`（连接地址）、`token`（Bearer token）。
- **Rust 命令接口**：通过 `tauri::generate_handler` 暴露四个命令——`list_services`（列出全部服务）、`save_service`（新增或按名称覆盖更新）、`delete_service`（按名称删除）、`open_service`（按名称打开 WebView 窗口加载服务）。
- **token 注入机制**：`open_service` 构建一段 JS init script，通过 WebViewWindowBuilder 的 `initialization_script` 方法注入。脚本逻辑：
  1. 将 token 写入 `localStorage.setItem('codeg-token', token)`
  2. 包装 `window.fetch`：在请求 init 的 headers 中添加 `Authorization: Bearer <token>`（仅当未手动设置时）
  3. 包装 `XMLHttpRequest.prototype.open`：调用 `setRequestHeader('Authorization', 'Bearer <token>')`
  - 两个拦截点覆盖 codeg 前端可能使用的所有 HTTP 请求方式。
- **WebView 窗口**：使用 `WebviewWindowBuilder::new` 创建外部 URL 窗口（`WebviewUrl::External`），窗口标签为 `codeg-<服务名>`，标题为 `Codeg - <服务名>`，默认内尺寸 390×844（竖屏手机尺寸），可调整大小。
- **URL 规范化**：服务 URL 在加载前去除尾部 `/` 后统一补 `/`，避免重复斜杠。
- **Android 构建策略**：绕过 `tauri build -t aarch64-linux-android`，改用 `tauri android init` 生成 Android 工程骨架后直接调用 `gradlew` 打包。原因：Tauri 的 `-t` 目标参数在 Android 场景下不稳定，直接走 Gradle 更可控。
- **NDK 链接器配置**：在 `src-tauri/.cargo/config.toml` 中配置 NDK 链接器路径，通过 `ndk-clang.sh` 包装脚本指向 NDK 的 darwin clang，确保 Rust 编译产出 arm64-v8a 的 `.so`。
- **Gradle 打包参数**：`./gradlew :app:assembleUniversalDebug -PabiList=arm64-v8a -ParchList=arm64 -PtargetList=aarch64 -x rustBuildArm64Debug -x rustBuildUniversalDebug`。排除 Rust 构建任务（`-x`）是因为 Rust `.so` 已通过 cargo 单独编译，避免重复构建。
- **buildToolsVersion**：在 `app/build.gradle.kts` 中显式设置 `buildToolsVersion = "34.0.0"`，避免 Gradle 使用不兼容的构建工具版本。
- **前端资源**：编译后的前端静态文件（HTML/CSS/JS）手动复制到 `gen/android/app/src/main/assets/`，由 Gradle 打包进 APK。
- **闪退修复（2026-11-15 实测修正）**：`mobile_entry_point` 宏**必须标在 `lib.rs` 的 `run()` 上，不能标在 main.rs 的 `main()` 上**——Android 构建只编译 lib 目标（cdylib），main.rs 不参与编译，宏标在 main() 上等于从未生效。同时 `tauri` 依赖**必须保留 `wry` 特性**（`tauri = { version = "2", features = ["wry"] }`）：JNI 入口宏展开会调用 `::tauri::android_binding!`，该宏定义在 `#[cfg(all(feature = "wry", target_os = "android"))]` 下，`features = []` 时不会生成 `Java_com_jiaweiwu_codegmobile_Rust_onCreate` 符号。缺失该符号时 `MainActivity` 启动即抛 `UnsatisfiedLinkError` 闪退（模拟器 logcat 实测确认）。另注：`release` 构建类型需显式设置 `manifestPlaceholders["usesCleartextTraffic"] = "true"`，否则无法加载内网 `http://` 服务地址。
- **禁止用缺省 feature 打 APK（2026-11-15 实测）**：tauri v2 的生产模式由 **`custom-protocol` feature** 控制（tauri `build.rs`: `dev = !custom_protocol`），与 cargo profile 无关——`cargo build --release` 不带该 feature 时 `dev` cfg 仍开启，移动端会把所有 WebView 资源请求代理到 dev server（`PROXY_DEV_SERVER = cfg!(all(dev, mobile))`，tauri-2.12.5 `src/protocol/tauri.rs::proxy_dev_request`），项目没跑 dev server，代理目标回落为 `http://tauri.localhost/` 自身，启动即白屏报 "Failed to request http://tauri.localhost/: error sending request for url"。修复：Rust 编译命令必须是 `cargo build --release --target aarch64-linux-android --features tauri/custom-protocol`（tauri-cli 的 `tauri android build` 也会自动加该 feature；绕过 CLI 直接走 gradle 时必须手动加）。Gradle 侧可沿用 debug 构建类型（自带 debug 签名 + cleartext 放行）。注意两点：① 前端改动要重新跑 cargo 编译才会进 `.so`（资源经 `generate_context!` 编译期内嵌）；② gradle 对 jniLibs 下的 symlink 快照可能不追踪目标文件变化，打包后要用 `unzip -p ... | shasum` 核对 `.so` 哈希。

## Testing Decisions

## Testing Decisions

- **UI 设计（2026-11-15 定稿）**：服务页采用「沉浸大卡片」方案——整卡点击即连接，编辑/删除收进卡片右上 ⋯ 底部菜单，添加/编辑共用带把手底部表单（保存并使用/取消），头部「Codeg.」+ ＋ 方形按钮（无 FAB）。三方案原型与对比记录见 `prototype/services-page-design` 分支（`prototype/services-page.html`），用户从 A/B/C 中选定 A。未采用：状态点/最近连接时间（无真实数据源，避免误导）。
- **首次使用体验**：服务列表为空时前端自动弹出"添加服务"表单（填写名称/地址/token），而不是停留在空列表或报错页。
- **模拟器验证为主**：使用 Android 模拟器（arm64-v8a 架构，API 35）验证 APK 能正常启动、不闪退。这是最关键的验证——闪退修复的有效性只能通过实际启动确认。
- **2026-11-15 启动白屏修复实测结果**：① 修复后 APK 启动直接渲染服务页（无 "Failed to request" 报错页）；② 空列表自动弹出"添加服务"表单（screencap 确认 名称/连接地址/Token 三个输入框）；③ 表单填写 test / http://1.2.3.4:3080 / tok123 后点保存，服务卡片正常渲染出 连接/编辑/删除 按钮，证明 `list_services`/`save_service` 在 Android 上读写 `app_config_dir()/services.json` 正常。④ `.so` 哈希校验：`unzip -p apk lib/arm64-v8a/libcodeg_mobile_lib.so | shasum` 与 `target/aarch64-linux-android/release/libcodeg_mobile_lib.so` 一致（custom-protocol 构建）。
- **前端 JS 验证点**：WebView 无打包器，`import '@tauri-apps/api/core'` 这类裸模块说明符会静默失败（页面卡在"加载中..."），必须走 `withGlobalTauri: true` 的 `window.__TAURI__.core.invoke`；验证方式是 adb screencap 看页面是否从"加载中"推进到服务列表/表单。
- **APK 内容校验**：通过 `unzip -p` 提取 APK 内的 `.so` 文件，计算 shasum 与 Rust 编译产物（`target/aarch64-linux-android/release/libcodeg_mobile_lib.so`）比对，确认修复后的原生库已正确打入 APK。
- **adb logcat 观察**：模拟器启动后通过 `adb logcat` 观察是否有崩溃日志，确认 App 进程稳定运行。
- **测试范围**：本次验证聚焦于"APK 能否正常启动、不闪退"。WebView 加载 codeg 页面、token 注入的实际效果属于功能验证，不在本次验证范围内（需要 codeg 服务端可达 + 有效 token）。
- **模拟器环境**：无头模式（`-no-window -no-audio -no-boot-anim`），软件渲染（`-gpu swiftshader_indirect`），2GB 内存，2 核 CPU。system image 使用 default 变体（`android/arm64-v8a-35_r02`）。

## Out of Scope

- codeg 服务端任何修改（方案 B 明确不碰 codeg 源码）
- iOS 版本（仅安卓 APK）
- x86 架构支持（仅 arm64-v8a）
- 生产环境签名发布（当前产出为 debug 签名 APK）
- codeg 前端功能本身的测试（token 注入后 codeg 页面是否能完整可用，需实际服务端验证）
- App 内浏览器后退/前进导航控制
- 服务配置的导入/导出功能
- 多窗口同时打开多个服务

## Further Notes

- **网络环境**：codeg 服务部署在内网（`100.94.107.28:3080`），APK 需在能访问该地址的网络环境下使用。
- **构建环境依赖**：Android SDK（build-tools 34.0.0、platform-tools、emulator）、NDK、JDK 21、Rust 工具链（含 Android target）。
- **已知构建坑**：dl.google.com 的 HTTPS 下载在此网络环境下被重置或极慢，Android SDK 组件需通过腾讯镜像（`mirrors.cloud.tencent.com/AndroidSDK/`）下载。
- **APK 产物**：当前产出 `codeg-mobile-debug-fixed.apk`（13.3MB，debug 签名，含 custom-protocol release `.so` + 首次使用表单），修复了启动白屏报错并实现空列表首屏表单。
- **后续工作**：可生成 release 签名 APK；在真实 codeg 服务上测试 token 注入效果；后续如引入前端构建步骤可换回 npm 包导入方式。
