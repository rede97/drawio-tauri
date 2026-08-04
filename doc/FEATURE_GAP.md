# draw.io Tauri — 未实现功能缺口清单

参照对象：官方 Electron 版 [drawio-desktop](https://github.com/jgraph/drawio-desktop)
（行为参考代码：`temp/drawio-desktop-ref/src/main/electron.js`，本地参考克隆，未入库）。

状态截至 **v31.1.5**。优先级：高 = 用户可感知的核心功能缺失；中 = 有官方功能但影响面小；低 = 锦上添花。

## 未实现功能

| # | 功能 | 官方实现方式 | 未实现原因 | 可行方案 | 优先级 |
|---|------|--------------|------------|----------|--------|
| 1 | **导出 PNG/JPG/PDF 文件** | Electron 隐藏窗口渲染后 `capturePage()` / `printToPDF()` 产出 | Tauri 没有 webview 截图/打印成 PDF 的 API；打印/SVG/XML 导出已实现 | (a) 渲染端 JS 将 SVG 栅格化到 canvas 回传 PNG dataURL；(b) Rust 侧 `resvg`/`printpdf`；(c) WebView2 私有 API `PrintAsync` | 高 |
| 2 | **插件管理（Extras → Plugins）** | Electron 主进程下载/安装/读取 webapp 插件文件到用户目录 | `installPlugin`/`uninstallPlugin`/`getPluginFile` 目前直接返回错误；需要网络下载 + 本地插件目录 + 启动注入机制 | 在 `<data_local_dir>/drawio/plugins/` 维护插件文件，`getPluginFile` 读取返回；安装/卸载可用 ureq/reqwest 下载官方插件仓库文件 | 中 |
| 3 | **VSDX 导入（本地解析）** | Electron 侧用 Node.js 解析 VSDX（Office Open XML zip） | Rust 生态没有现成的 Visio VSDX 解析库；需移植解析逻辑 | 短期：关联 .vsdx 后提示用户先在线转换；长期：Rust 侧 zip + XML 解析移植（工作量大） | 中 |
| 4 | **原生应用菜单栏** | Electron `Menu.setApplicationMenu()`（macOS 必需，Windows/Linux 可选） | draw.io webapp 自带页面内菜单栏，功能等价；仅 macOS 上缺少系统菜单约定（About/Quit/Copy/Paste） | 用 `tauri::menu::Menu` 仅针对 macOS 添加系统菜单，其余平台保持页面菜单 | 低 |
| 5 | **系统托盘** | 官方无托盘（drawio-desktop 同样没有） | `Cargo.toml` 已开 `tray-icon` feature 但未使用；官方行为一致，无需求 | 如需最小化到托盘再自行添加 | 低 |
| 6 | **命令行 `--layout` / `--mermaid-image` 参数** | 官方 `args.js` 解析 CLI 参数控制导入行为 | 属于批量转换场景的小众功能；当前仅透传文件路径（`args-obj`） | 在 `app-load-finished` 的 payload 中补充解析后的 `layout`/`mermaidImage` 字段 | 低 |

## 已实现（与官方对齐，供对照）

文件对话框与 I/O、自动保存草稿（`.$name.dtmp`）、保存前备份（`.$name.bkp`）与损坏恢复（`getBkpFile`）、三键关闭确认握手（Save/Discard/Cancel）、File → Exit、文件监听、系统字体枚举、拼写检查开关、Google Fonts 开关、备份开关、**自动更新检查（启动静默 + 菜单手动，tauri-plugin-updater）**、命令行文件路径参数、窗口缩放（CSS）、剪贴板读写（文本/图像）、文件关联（.drawio/.drawio.xml/.vsdx/.mmd/.mermaid）、窗口尺寸/位置记忆（tauri-plugin-window-state）、`windowAction`/`isFullscreen`/`checkFileExists` IPC（对齐官方，webapp 31.1.5 未调用）、**多窗口（File → New Window，独立关闭握手/文件监听路由）**、**单实例（二次启动转发文件到新窗口）**、**打印与 SVG/XML 本地导出（隐藏 export3.html 渲染窗口 + 系统打印对话框）**。

## 升级 drawio 子模块时的对齐检查

每次 bump `drawio/` 子模块后：

1. diff `drawio/src/main/webapp/js/diagramly/ElectronApp.js` 中的 `action: '...'` 与 `sendMessage('...')` 清单
2. 对照 `src-tauri/src/ipc.rs` 的 match 分支，新增分支归入上表或实现
3. 行为语义以 `temp/drawio-desktop-ref/src/main/electron.js` 为准
