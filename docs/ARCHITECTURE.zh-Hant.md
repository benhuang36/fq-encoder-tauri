# 架構說明 — Tauri 導覽

[English](ARCHITECTURE.md) · **繁體中文**

這個專案雖小,卻用上了打造一個實際 Tauri 2 App 所需的大部分概念:commands、plugins、權限模型、tray 圖示、背景工作、共享狀態,以及 JS↔Rust 橋接。這份文件把檔案對應到這些概念,讓你帶著地圖讀程式碼。

> 文中提到的每個符號都是真的 —— 打開對應檔案搜尋即可找到。

---

## 1. Tauri 心智模型

一個 Tauri App 是**兩個半邊透過 IPC 橋接溝通**:

```
┌─────────────────────────┐         invoke()          ┌──────────────────────────┐
│   Web 前端                │  ───────────────────────► │   Rust 核心                │
│   (src/, index.html)     │                            │   (src-tauri/)            │
│   跑在 OS 的原生 webview    │  ◄─────────────────────── │   原生 binary;掌管          │
│                          │         events / 回傳值     │   視窗、tray、剪貼簿         │
└─────────────────────────┘                            └──────────────────────────┘
```

- **Rust 核心**掌管 OS:視窗、tray、剪貼簿、檔案系統。它是一個真正的原生程序。
- **前端**是 HTML/CSS/JS,渲染在平台的**原生** webview —— WKWebView(macOS)、WebView2(Windows)、WebKitGTK(Linux)。Tauri **不**內含瀏覽器,所以安裝檔只有幾 MB,而不像 Electron 動輒 ~100 MB。
- 前端透過 **`invoke("command", args)`** 呼叫 Rust;Rust 可以回推 **events**。整個橋接就這樣。

---

## 2. 逐檔註解的目錄樹

```
fq-encoder-tauri/
├── index.html                  # webview 進入點頁面
├── package.json                # 前端依賴 + npm scripts;"version" 決定安裝檔名稱
├── vite.config.ts              # 前端打包器(dev server 在 :1420)
├── tsconfig.json
├── src/                        # ── 前端 ──
│   ├── main.ts                 #   UI 邏輯:invoke()、i18n、store、事件
│   └── styles.css              #   漸層／毛玻璃外觀
├── src-tauri/                  # ── RUST 核心 ──
│   ├── Cargo.toml              #   Rust 依賴:tauri、plugins、sha2、sys-locale
│   ├── build.rs                #   編譯期執行 tauri-build 程式碼生成
│   ├── tauri.conf.json         # ★ 核心設定:視窗、bundle、build hooks
│   ├── capabilities/
│   │   └── default.json        # ★ 權限白名單(Tauri 2 安全模型)
│   ├── icons/                  #   App 圖示(由 `tauri icon` 產生)
│   └── src/
│       ├── main.rs             #   binary 進入點 → 呼叫 lib::run()
│       ├── lib.rs              # ★ 整個 App:Builder、commands、tray、monitor
│       └── codec.rs            # ★ 純邏輯 + 測試(不依賴 Tauri)
└── .github/workflows/
    ├── build.yml               # CI:每次 push 跑 test + 打包 artifacts
    └── release.yml             # tag v* → 建置三平台 → 草稿 Release
```

四個 ★ 檔案是所有 Tauri 專屬概念所在之處,先讀這些。

---

## 3. 關鍵檔 → 各自教的概念

### `src-tauri/tauri.conf.json` — 宣告式 App 設定

不寫程式就定義出 App 外殼:

- `app.windows[]` —— 視窗:`"label": "main"`、尺寸 `480×660`、標題。label `"main"` 是 Rust 之後找視窗的依據(`get_webview_window("main")`),也是 `capabilities` 套用權限的範圍。
- `build` —— 把 Rust build 接上前端 build:`beforeDevCommand: "npm run dev"`、`devUrl: "http://localhost:1420"`、`frontendDist: "../dist"`。
- `bundle` —— 安裝檔圖示／目標。`productName` + `version` 決定產出檔名。

### `src-tauri/capabilities/default.json` — 權限模型

Tauri 2 和 Electron 最大的差別:**預設什麼都不允許**。你必須逐項授權。本專案:

```json
"permissions": [
  "core:default", "opener:default", "store:default",
  "clipboard-manager:allow-read-text", "clipboard-manager:allow-write-text"
]
```

如果某個 plugin 呼叫在執行期報「not allowed」錯誤,先來這裡看 —— 通常是忘了授權。

### `src-tauri/src/lib.rs` — 核心(請慢慢讀)

`run()` 是一條流暢的 `tauri::Builder` 鏈,一次示範了六個概念:

| `run()` 裡的那一行 | 概念 |
|---|---|
| `.plugin(tauri_plugin_clipboard_manager::init())`(以及 `store`、`opener`) | **Plugins** —— 選用的 OS 能力 |
| `.invoke_handler(generate_handler![encode, decode])` | **Commands** —— 暴露給 JS 的 `#[tauri::command]` 函式 |
| `.manage(state.clone())` / `app.state::<Arc<MonitorState>>()` | **共享狀態** —— 型別化、隨處可取 |
| `.setup(\|app\| { … })` | **啟動 hook** —— 初始化後執行一次 |
| `TrayIconBuilder::with_id("main-tray")…` | **Tray 圖示 + 選單**(`MenuItem`、`CheckMenuItem`) |
| `.on_window_event(\| CloseRequested \| …)` | **視窗事件** —— 隱藏而非結束(常駐 App) |

`setup()` 裡值得追蹤的:

- `app.set_activation_policy(ActivationPolicy::Accessory)` —— 僅 macOS;移除 Dock 圖示,變成純選單列 App。
- `tray_labels()` —— 依 OS 語系把 tray 選單在地化(`sys-locale`)。
- `spawn_clipboard_monitor(app.handle().clone(), state)` —— 啟動背景執行緒(見下方流程 ②)。

### `src-tauri/src/codec.rs` — 與框架無關的邏輯

注意它**完全沒有 Tauri import**。純 Rust(`encode`、`decode`、`looks_encoded`)加上 `#[cfg(test)]` 測試,用單純的 `cargo test` 就能跑。好習慣:讓你的領域邏輯和框架解耦,才好測試、好移植(這個檔案正是與 macOS Swift 版逐字相同,由 golden-vector 測試驗證)。

### `src/main.ts` — 橋接的前端側

- `invoke<string>("encode", { text, key })` —— 呼叫 Rust command;JS 的參數名(`text`、`key`)必須對上 Rust 函式的參數名。
- `load(STORE_FILE, …)` + `store.get/set` —— `@tauri-apps/plugin-store` 的 JS API,讀寫和 Rust 端同一個 `settings.json`。

---

## 4. 兩條值得追蹤的資料流

**① 手動 encode(按鈕 → Rust → 返回):**

```
click handler (main.ts)
  → invoke("encode", { text, key })            [JS]
  → #[tauri::command] fn encode(text, key)     [Rust, lib.rs:26]
  → codec::encode(&text, &key)                 [Rust, codec.rs:58]
  → 回傳 String → resolve JS promise → setOutput()
```

`decode` 一樣,但回傳 `Result<String, String>` —— 出錯時回傳一個**穩定的代碼**(`e.code()`,例如 `invalid_character:x`),由 `main.ts` 在地化。這就是「把 UI 語言留在前端、不寫進 Rust」的做法。

**② 剪貼簿自動監聽(完全不經過 UI):**

```
std::thread 每 500 毫秒輪詢一次       (spawn_clipboard_monitor, lib.rs:66)
  → app.clipboard().read_text()         [clipboard plugin]
  → 防迴圈守衛 (MonitorState.last_seen / last_written)
  → codec::looks_encoded() ? decode : encode
  → app.clipboard().write_text(result)
```

**store 共享同步**:兩個半邊都需要密碼。前端把它寫進 store(`fq.encodingPassword`);Rust 監聽器每次輪詢時用 `read_password()`(lib.rs:50)讀取。用 store 當共享設定,省掉額外的 IPC。

---

## 5. 概念索引

- **Command** —— 標了 `#[tauri::command]` 並註冊進 `invoke_handler` 的 Rust 函式;JS 透過 `invoke` 呼叫。
- **Plugin** —— 打包好的 OS 能力(clipboard、store、opener),用 `.plugin(...)` 註冊,並由 `capabilities` 裡對應的權限放行。
- **Capability / 權限** —— `capabilities/default.json` 裡的白名單;Tauri 2 會擋掉所有沒列出的東西。
- **Tray** —— 用 `TrayIconBuilder` + `Menu` 建立的狀態列／系統匣圖示。
- **State(狀態)** —— 透過 `.manage(T)` 與 `app.state::<T>()` 的型別化共享資料。
- **Event(事件)** —— Rust→JS(或 JS→Rust)的訊息,用 `emit`/`listen`(本 App 用 commands、沒用自訂事件,但它是橋接的另一半)。
- **Store** —— `plugin-store` 的鍵值 JSON 檔,JS 和 Rust 兩端都能用。

---

## 6. 建議閱讀順序

1. `src-tauri/tauri.conf.json` —— App 外殼。
2. `src-tauri/src/lib.rs` —— `run()`:Builder、commands、plugins、setup、tray。
3. `src-tauri/capabilities/default.json` —— 為什麼那些 plugin 呼叫被允許。
4. `src/main.ts` —— `invoke` + store 的 JS 側。
5. `src-tauri/src/codec.rs` —— 看邏輯如何保持解耦。

---

## 7. 接下來看哪裡(Tauri v2 官方文件)

- 從前端呼叫 Rust(commands):https://tauri.app/develop/calling-rust/
- Capabilities 與權限:https://tauri.app/security/capabilities/
- Plugins(clipboard、store…):https://tauri.app/plugin/
- 系統匣 tray:https://tauri.app/learn/system-tray/
- 建置與打包:https://tauri.app/distribute/
