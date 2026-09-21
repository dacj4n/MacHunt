<p align="center"><img src="src-tauri/icons/icon.png" width="384" alt="MacHunt 图标" /></p>

<h1 align="center">MacHunt</h1>

<p align="center">
  <img src="https://img.shields.io/badge/-Rust-000000?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/-TypeScript-3178C6?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/-CSS-1572B6?logo=css3&logoColor=white" alt="CSS" />
  <img src="https://img.shields.io/badge/-Objective--C-3A95E3?logo=apple&logoColor=white" alt="Objective-C" />
  <img src="https://img.shields.io/badge/-HTML-E34F26?logo=html5&logoColor=white" alt="HTML" />
  <img src="https://img.shields.io/badge/-React-61DAFB?logo=react&logoColor=black" alt="React" />
  <img src="https://img.shields.io/badge/-Tauri-24C8D8?logo=tauri&logoColor=white" alt="Tauri" />
</p>

一个纯本地运行的 macOS 文件/文件夹搜索工具，提供 CLI 与原生 GUI（Tauri + React）两种入口。不依赖 HTTP 后端，不上传任何数据。

[English](README.md)

## 简介

MacHunt 将你的整个文件系统扫描到本地 SQLite FTS5 索引中。CLI 搜索耗时 <5ms。通过 macOS FSEvents 实现增量实时更新。可以理解为开源的 Spotlight，带强大的 CLI，数据永不离开本机。

## 截图

<table>
<thead>
<tr>
<th width="50%" align="center">搜索</th>
<th width="50%" align="center">收藏</th>
</tr>
</thead>
<tbody>
<tr>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/search.png"><img src="./screenshots/search.png" alt="搜索" width="100%" style="max-width: 100%;"></a></td>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/pinned.png"><img src="./screenshots/pinned.png" alt="收藏" width="100%" style="max-width: 100%;"></a></td>
</tr>
<tr>
<td align="center"><strong>全盘搜索、分类标签</strong></td>
<td align="center"><strong>收藏页面，重启数据不丢失</strong></td>
</tr>
</tbody>
</table>

<table>
<thead>
<tr>
<th width="50%" align="center">Quick Look 预览</th>
<th width="50%" align="center">设置</th>
</tr>
</thead>
<tbody>
<tr>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/quicklook.png"><img src="./screenshots/quicklook.png" alt="Quick Look" width="100%" style="max-width: 100%;"></a></td>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/settings.png"><img src="./screenshots/settings.png" alt="设置" width="100%" style="max-width: 100%;"></a></td>
</tr>
<tr>
<td align="center"><strong>空格触发原生 Quick Look</strong></td>
<td align="center"><strong>设置</strong></td>
</tr>
</tbody>
</table>

## 安装

从 [GitHub Releases](https://github.com/dacj4n/MacHunt/releases) 下载最新 `.dmg`，挂载后拖入 `/Applications`。

> **首次启动**：macOS Gatekeeper 可能拦截未签名应用。如果提示"无法验证开发者"或"已损坏"，在 Finder 中右键 `MacHunt.app` → 选择**"打开"** → 弹出对话框中点击**"打开"**。或在终端执行 `xattr -cr /Applications/MacHunt.app` 后重新双击打开。

或从源码构建：

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
```

### 仅 CLI

```bash
cargo build --release
./target/release/machunt --help
```

### GUI（开发）

```bash
npm install
npm run tauri dev
```

### GUI（打包）

```bash
npm run build
npm run tauri build
```

## 环境要求

| macOS 15 (Apple Silicon) | ✅ 已测试 |
| macOS 13–14 | ✅ 预计可用 |
| macOS 10.15–12 | ⚠️ 理论支持（未测试） |
| Intel Mac (x86_64) | ⚠️ 通用二进制已包含，未测试 |

> **说明**：App 构建为通用二进制（arm64 + x86_64）。macOS <13 上登录项功能使用 AppleScript 降级方案，不影响正常使用。

- Rust 1.70+
- Node.js 18+（仅 GUI）
- npm 9+（仅 GUI）

## 快速上手

```bash
# 首先构建索引（全盘扫描，300万文件约 10 秒）
machunt build

# 子串搜索（不区分大小写）
machunt search "预算"

# 通配符模式
machunt search -p "*.rs"

# 模糊搜索（空格分词，多 token 子串 AND 匹配）
machunt search -F "系统 远程"   # 分词后全部匹配

# 区分大小写
machunt search -c "Makefile"

# JSON 输出，方便脚本处理
machunt search --json "发票" | jq .

# 启动实时监听 + 交互搜索
machunt watch
```

## CLI 命令参考

```
machunt <COMMAND>
```

### `search`

```bash
machunt search [OPTIONS] <QUERY>
```

| 选项 | 说明 |
|------|------|
| `-p, --pattern` | 通配符模式（如 `*.rs`、`test?.txt`） |
| `-F, --fuzzy` | 多 token 子串模糊搜索（空格分词，全部 AND 匹配） |
| `-c, --case-sensitive` | 区分大小写 |
| `-n, --limit <N>` | 最大结果数（默认 100） |
| `-P, --path <PATH>` | 路径前缀过滤 |
| `-f, --files` | 仅文件 |
| `-d, --dirs` | 仅目录 |
| `--json` | JSON 输出 |

通配符规则：
- `*` — 匹配任意字符不含 `/`（单层目录）
- `**` — 匹配任意字符含 `/`（所有层级）
- `?` — 匹配单个字符不含 `/`
- `{a,b}` — 匹配 `a` 或 `b`

### `build`

```bash
machunt build [OPTIONS]
```

| 选项 | 说明 |
|------|------|
| `-p, --path <PATH>` | 仅构建指定范围 |
| `--rebuild` | 先清空再重建 |
| `--include-dirs <true\|false>` | 是否索引目录（默认 `true`） |

### `watch`

```bash
machunt watch
```

启动 FSEvents 监听，有历史 EventID 时从上次位置续跑，进入终端交互搜索循环。

### `optimize`

```bash
machunt optimize [--vacuum]
```

默认执行 WAL checkpoint，可选 `--vacuum` 回收数据库文件空间。

## 工作原理

```
┌──────────┐     ┌───────────────┐     ┌──────────┐
│  WalkDir │ ──→ │  SQLite FTS5  │ ←── │ FSEvents │
│  (构建)   │     │  (trigram)    │     │  (监听)  │
└──────────┘     └───────┬───────┘     └──────────┘
                         │
                    ┌────▼────┐
                    │  搜索    │
                    │  <5ms   │
                    └─────────┘
```

- **构建**：`WalkDir` 遍历文件系统，把 `(name_lower, path)` 写入 `files` 并同步到 FTS5（trigram 分词器），同时记录文件大小与修改时间，使大小/时间筛选与排序能在 SQL 层完成而无需逐个 `stat`。扫描通过 crossbeam 通道并行处理。
- **搜索**：FTS5 trigram `MATCH` 处理子串查询，CLI 下为个位数毫秒。短于 3 个字符的查询，以及中文等非 ASCII 查询，会回退到 `LIKE` 扫描——日常使用依然很快，但在数百万文件的索引上是最慢的一条路径。区分大小写时额外用 GLOB 后过滤（SQLite 的 `LIKE` 对 ASCII 不区分大小写）。模糊搜索基于空格分词 + 多 token 子串 AND 匹配。
- **监听**：通过 CoreServices FFI 直调 FSEvents，监听文件的创建、修改、删除、重命名事件，增量更新索引，重启后从持久化的 EventID 续跑。事件密集时 FSEvents 会主动声明「已丢弃事件」（`MustScanSubDirs` / `UserDropped` / `KernelDropped`）并指定需要重扫的目录；此时按 Apple 的约定重扫该目录，把漏报的文件补回索引，而不是等下一次全量重建。
- **卷**：每 10 秒轮询一次 `/Volumes` 以发现已挂载磁盘，因为 FSEvents 不会上报 SMB/WebDAV 共享上的变更。新卷在后台建立索引，卸载时立即清除其索引条目。

## GUI

原生 macOS GUI，基于 Tauri 2 + React。与 CLI 共用同一 Rust 核心引擎——没有 HTTP 服务器，没有额外的 IPC 开销。

### 主窗口

- 实时全盘搜索
- 标签导航：搜索 / 收藏 / 设置（`Cmd+1/2/3`）
- 通配符开关 + 区分大小写开关
- 路径过滤（手动输入 + 下拉建议 + Finder 选取）
- 筛选器：应用（186 条扩展名映射）、时间（日历组件自定义范围）、大小（自定义数值 + 单位）
- 模糊搜索按钮（空格分词，多 token 子串 AND 匹配）
- 分类标签：全部 / 文件 / 文件夹 / 文档 / 图片 / 音视频 / 代码 / 压缩包
- 表头排序：名称、路径、类型、大小、修改时间
- 列宽拖拽，宽度记忆持久化
- 单选/多选（`Shift` 连选、`Cmd` 多选）
- 空格触发 Quick Look（支持多选）
- 双击打开
- 拖拽导出：把选中的结果直接拖到访达、桌面或其他应用
- 每行末尾内嵌收藏按钮（hover 时显示）

> 拖拽走的是原生 AppKit 拖拽会话，而不是 HTML5 拖放——HTML5 只能在网页之间传数据，落到原生应用上的会是空内容。拖出去的是真实的文件对象（可按文件粘贴，不是路径文本），并且只提供「拷贝」语义，因此拖到任何地方**都不会移动或删除原文件**。

### 快捷键

窗口打开时会自动聚焦搜索框并全选文字，可以直接开始输入。`Tab` 在搜索框与结果列表之间切换焦点，方向键与 `Enter` 作用于当前获得焦点的一侧。

| 快捷键 | 行为 |
|--------|------|
| `Tab` | 在搜索框与结果列表之间切换焦点 |
| `↑` `↓` | 移动选中项（需结果列表获得焦点） |
| `Enter` / `Cmd+O` | 打开选中项 |
| `Space` | 原生 Quick Look 预览（支持多选） |
| `Cmd+A` | 全选结果 |
| `Cmd+C` | 拷贝选中项（作为文件对象） |
| `Cmd+F` | 聚焦并全选搜索框 |
| `Cmd+1` / `Cmd+2` / `Cmd+3` | 切换 搜索 / 收藏 / 设置 |
| `Esc` | 隐藏窗口 |
| `Cmd+Shift+D` | 全局显示/隐藏窗口（可在设置中修改） |

在路径过滤框内，`↑` `↓` `Enter` `Esc` 用于浏览下拉建议。

### 菜单栏图标

MacHunt 常驻菜单栏。点击图标切换窗口显隐（并自动聚焦搜索框）；图标菜单包含 搜索、收藏、设置（`Cmd+,`）、退出（`Cmd+Q`）。图标使用模板图（template image），会自动适配浅色/深色菜单栏；菜单文案跟随应用语言。可在「设置 → 启动」中关闭该图标。

### 外置卷

已挂载磁盘不走 FSEvents：由于 FSEvents 不监听 SMB/WebDAV 共享，程序每 10 秒轮询一次 `/Volumes`。新挂载的卷会在后台建立索引，文件随索引进度出现在结果中；卸载时立即清除该卷的索引条目。卷被检测到、索引完成、卷消失时，界面都会收到提示。

### 右键菜单

打开、打开于...（在 Finder 中查看 / 在 QSpace Pro 中查看 / 在终端中打开 / 在 WezTerm 中打开）、拷贝名称、拷贝路径、拷贝结果（作为文件对象）、拷贝所有结果、拷贝所有文件名、拷贝所有文件路径、移到废纸篓、收藏 / 取消收藏。

### 收藏页面

搜索结果可点击星标收藏到专属收藏页。收藏数据存储在 localStorage（key: `machunt.pinned.items`），重启后数据不丢失。收藏页支持表头排序、列宽拖拽、空格 Quick Look、Cmd+A 全选，点击星标或右键即可取消收藏。星标按钮位于每行末尾，仅在鼠标悬浮时显示——金色实心为已收藏，空心为未收藏。

### 设置页面

主页面：

- **主题模式**：跟随系统 / 浅色 / 深色
- **语言**：中文 / English
- **快捷键**：全局唤起/隐藏窗口（默认 `Cmd+Shift+D`）
- **启动**：开机自启、静默启动、显示/隐藏程序坞图标、显示/隐藏菜单栏图标
- **搜索结果数量**：单次搜索最多返回的条数（50–10000）
- **文件管理器与终端**：双击默认动作（Finder / QSpace Pro / 自定义应用）与默认终端（Terminal / WezTerm / 自定义应用）
- **更新**：自动检查更新开关、手动立即检查

「高级功能」弹窗（进阶配置集中在这里，避免主页面过长）：

- **索引控制**：手动构建 / 重建，以及开始·停止监听
- **索引维护**：重建后按条件自动 `VACUUM`
- **诊断日志**：默认关闭且不产生任何日志文件；开启后立即生效，可限定记录某个路径范围
- **监听根目录**：指定 FSEvents 监听范围
- **排除目录**：完整目录规则 + 正则/通配符规则（优先按正则解析，失败再按通配符）
- **排除文件**：跳过隐藏文件开关 + 按文件名排除

### 检查更新

MacHunt 会与 GitHub Releases 上的最新版本号进行比对。该功能默认开启，可在设置中关闭，也提供「立即检查」按钮。请求中只携带当前版本号作为 User-Agent——不会发送任何关于你的文件、索引或搜索内容的信息。

## 功能概览

| 分类 | 能力 |
|------|------|
| 搜索模式 | 子串、通配符、模糊（多 token 子串） |
| 大小写 | CLI 和 GUI 均可切换 |
| 路径过滤 | 前缀、下拉建议、Finder 选取 |
| 应用筛选 | 186 条扩展名 → 默认应用映射 |
| 时间/大小筛选 | 自定义日历范围 / 数值 + 单位 |
| 实时更新 | FSEvents 监听，EventID 持久化；事件被丢弃时按约定重扫补回 |
| 元数据 | 大小与修改时间随索引存储，筛选与排序下推到 SQL，无需逐个 stat |
| 外置卷 | 挂载即自动索引，卸载即清理 |
| 文件分类 | 8 个分类标签（扩展名自动归类） |
| 收藏 | 星标按钮，专属收藏页，localStorage 持久化 |
| 预览 | 原生 Quick Look（空格，支持多文件） |
| 拖拽导出 | 结果可直接拖到访达 / 桌面 / 其他应用，只提供拷贝语义 |
| 导出 | 拷贝为文件对象、CLI JSON 输出 |
| 菜单栏 | 托盘图标：搜索 / 收藏 / 设置 / 退出 |
| 更新 | 可选的 GitHub Releases 版本检查，自动或手动 |
| 设计 | Liquid Glass 设计系统，跟随系统/浅色/深色 |
| 国际化 | 中文 / English |
| 启动 | 开机自启、静默模式、Dock 与菜单栏图标开关 |
| 性能 | EventID 淘汰检测、惰性死路径清理 |
| 诊断日志 | 默认关闭；可开关并限定路径范围，总量有上限（10 个文件 × 32 MiB） |
| 隐私 | 索引与搜索全部本地，数据不出本机 |

## 对比

| | MacHunt | Spotlight | Raycast | uTools |
|---|---|---|---|---|
| **全盘扫描** | 是（300万文件 ~10s） | 是（`mdfind`） | 插件形式 | 插件形式 |
| **搜索延迟** | <5ms（CLI，FTS5 trigram） | 50–200ms+ | 视插件 | 视插件 |
| **索引格式** | SQLite FTS5（开放） | 私有 | 私有 | N/A |
| **CLI** | 是 | 是（`mdfind`） | 否 | 否 |
| **模糊搜索** | 是（多 token 子串） | 部分 | 否 | 否 |
| **增量更新** | FSEvents | FSEvents | 视情况 | N/A |
| **开源** | 是 | 否 | 否 | 部分 |

## 开发

### 技术栈

- **核心**：Rust
- **CLI**：Clap
- **GUI 前端**：React 18 + TypeScript + Vite
- **GUI 容器**：Tauri 2
- **全局快捷键**：`tauri-plugin-global-shortcut`
- **存储**：SQLite FTS5（`rusqlite`，WAL，trigram tokenizer）
- **扫描**：WalkDir + Crossbeam channels
- **监听**：macOS FSEvents（CoreServices FFI）
- **设计**：macOS 26 Liquid Glass —— 原生 `NSVisualEffectView` 作为背景，上层为 CSS 自定义属性体系；内联主题检测消除首帧闪烁

### 构建命令

| 命令 | 说明 |
|------|------|
| `npm run build` | 仅构建前端（TS + Vite → `dist/`） |
| `npm run tauri build` | 完整构建：前端 + Rust → `.app` / `.dmg` |
| `npm run tauri dev` | 开发模式（热重载） |
| `cargo build --release` | 仅 CLI 二进制 |

### `npm run build` 与 `npm run tauri build` 的区别

- `npm run build` 只构建前端资源，**不**编译 Rust，**不**生成 `.app` / `.dmg`。
- `npm run tauri build` 先执行 `beforeBuildCommand`（即 `npm run build`），再编译 Rust 后端，最终输出可安装产物。

## 项目结构

```
mac_find/
├── src/                      # Rust 核心引擎与 React 前端共用同一目录
│   ├── main.rs               # CLI 入口（clap）
│   ├── lib.rs                # 库入口，导出 Engine
│   ├── engine.rs             # 引擎：构建 / 搜索 / 监听调度
│   ├── db.rs                 # SQLite FTS5：建表、插入、搜索、模糊
│   ├── builder.rs            # WalkDir 文件系统扫描器
│   ├── watcher.rs            # FSEvents FFI 监听器
│   ├── search.rs             # 通配符转正则
│   ├── filters.rs            # 排除规则（精确 + 通配符）
│   ├── apps.rs               # 扩展名 → 默认应用映射（186 条）
│   ├── model.rs              # 共用类型（FileEntry、SearchOptions）
│   ├── utils.rs              # 路径规范化、跳过逻辑、日志
│   ├── App.tsx               # React 应用外壳：状态、视图、键盘处理
│   ├── App.css               # Liquid Glass 设计系统（CSS 自定义属性）
│   ├── main.tsx              # React 入口
│   ├── i18n.ts               # 中文 / English 文案
│   ├── types.ts              # 前端类型
│   ├── utils.ts              # 前端工具（筛选、格式化、本地存储）
│   └── components/
│       ├── SearchView.tsx    # 结果表格、筛选器、状态栏
│       ├── SettingsView.tsx  # 设置页
│       ├── ContextMenu.tsx   # 右键菜单
│       └── CustomSelect.tsx  # 自定义下拉框
├── src-tauri/                # Tauri GUI 后端
│   ├── src/
│   │   ├── main.rs           # Tauri 应用入口
│   │   ├── lib.rs            # 应用初始化、窗口生命周期、命令注册
│   │   ├── commands/mod.rs   # 全部 #[tauri::command] 处理器
│   │   ├── window.rs         # 窗口显隐、Liquid Glass 背景、窗口外观
│   │   ├── settings.rs       # settings.json 持久化（GuiSettings、AppState）
│   │   ├── file_ops.rs       # 打开 / 定位 / 预览 / 剪贴板 / 拖拽会话 / 废纸篓
│   │   ├── tray.rs           # 菜单栏图标及其菜单
│   │   ├── menu.rs           # macOS 应用菜单
│   │   ├── startup.rs        # 开机自启（SMAppService + AppleScript 降级）
│   │   └── ffi.rs            # 激活策略桥接
│   ├── macos/
│   │   └── quicklook_bridge.m  # ObjC 桥接：Quick Look、剪贴板、Dock
│   ├── capabilities/         # Tauri 权限配置
│   ├── icons/                # 应用图标
│   ├── build.rs              # 构建脚本（编译 ObjC 桥接代码）
│   ├── Info.plist            # macOS Bundle 元数据
│   └── tauri.conf.json       # Tauri 配置
├── public/fonts/             # 自托管字体（Hanken Grotesk、Geist）
├── screenshots/              # 本文档截图
├── .github/workflows/        # 发布 CI
├── index.html                # HTML 外壳，内联主题检测脚本
├── Cargo.toml                # Rust crate 配置
├── package.json              # 前端依赖
├── tsconfig.json             # TypeScript 配置
├── vite.config.ts            # Vite 配置
└── VERSION                   # 当前版本号
```

> Rust 引擎与 React 前端刻意放在同一个 `src/` 目录下：CLI 与 GUI 链接的是同一个 crate，搜索、索引与监听永远只有一份实现。

## 运行时数据

| 路径 | 内容 |
|------|------|
| `~/Library/Caches/MacHunt/index.db` | FTS5 搜索索引 |
| `~/Library/Application Support/MacHunt/settings.json` | GUI 配置（含主题偏好） |
| `~/Library/Caches/MacHunt/logs/` | 诊断日志。默认**不产生任何文件**；开启后最多保留 10 个、单个上限 32 MiB |
| `~/Library/Caches/MacHunt/watch-log-on` | 诊断日志的开关标记。存在即开启，内容可写一个路径前缀以限定记录范围 |

> 诊断日志默认关闭，因此正常使用不会积累任何日志文件。需要排查「文件没有被索引」这类问题时，可在「设置 → 高级功能 → 诊断日志」中开启，无需重启即可生效。

## 为什么索引文件会很大

- macOS 上百万级文件很常见
- 默认索引目录条目
- 路径字符串较长，文本存储成本高
- 写入期间 `index.db-wal` 会临时增大

维护建议：

```bash
machunt optimize --vacuum
```

## 许可证

MIT
