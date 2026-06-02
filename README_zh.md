# MacHunt

一个纯本地运行的 macOS 文件/文件夹搜索工具，提供 CLI 与原生 GUI（Tauri + React）两种入口，不依赖 HTTP 后端服务。

[English](README.md)

## 文件搜索工具对比

| | MacHunt | macOS 聚焦 | Raycast | uTools |
|---|---|---|---|---|
| **全文件系统扫描** | 是（300万文件 ~10s） | 是（通过 `mdfind`） | 文件搜索插件 | 插件形式 |
| **搜索延迟** | <5ms（FTS5 trigram） | 50–200ms+ | 视插件 | 视插件 |
| **索引格式** | SQLite FTS5（开放、可审计） | 私有格式 | 私有格式 | N/A |
| **CLI** | 是（`machunt search`） | 是（`mdfind`） | 否 | 否 |
| **原生 GUI** | 是（Tauri） | 系统内建 | 是（Electron） | 是（Electron） |
| **模糊搜索** | 是（Levenshtein） | 部分 | 否 | 否 |
| **增量更新** | FSEvents | FSEvents | 视情况 | N/A |
| **排除/监听目录配置** | 是（GUI + 通配符） | 有限（系统偏好设置） | 否 | 否 |
| **程序坞可选** | 是（开关） | N/A | 否 | 否 |
| **开源** | 是 | 否 | 否 | 部分 |

**MacHunt 的核心优势：**

- **搜索迅速** — SQLite FTS5 trigram 索引，常规查询 5<ms，无需内存索引。
- **资源占用低** — 总计 ~200 MB（后端 ~70 MB + WebView）。App Nap 开启，空闲 CPU 接近零。
- **CLI 完整** — `machunt search` 支持子串、通配符、模糊搜索和 JSON 输出。
- **配置透明** — 标准 SQLite 数据库，可审计、可查询。配置均为 JSON 文件。
- **完全开源** — 不依赖私有服务，所有代码可审查、可自行构建。

## 核心能力

### CLI

- 本地索引构建/重建
- `search` 子命令：子串、通配符、模糊搜索 + JSON 输出
- 子串搜索（`machunt search "关键词"`）
- 通配符搜索（`machunt search -p "*.rs"`）
- 模糊/容错搜索（`machunt search -F "redme"` 可找到 "README"）
- JSON 输出（`machunt search --json "关键词"`）
- 文件/文件夹类型筛选（`-f` / `-d`）
- 路径前缀过滤（`-P ~/projects`）
- FSEvents 增量监听（`watch`）
- 索引维护（`optimize --vacuum`）

### GUI

- 本地即时搜索（无服务端）
- 正则开关 + 大小写匹配开关
- 路径过滤：手动输入 + 下拉建议 + Finder 选取
- 启动自动开启监听
- 构建 / 重建
- 分类筛选（全部、文件、文件夹、文档、图片、音视频、代码、压缩包）
- 表头排序（名称/路径/类型/大小/修改时间）
- 列宽拖拽 + 宽度记忆
- 单选/多选（`Shift` 连选、`Cmd` 多选）
- 键盘上下移动选中（`↑` / `↓`）
- 空格触发 Quick Look（支持多选）
- 双击打开文件或目录
- 右键菜单：
  - 打开
  - 打开于...（Finder / QSpace Pro / Terminal / WezTerm）
  - 拷贝结果 / 拷贝所有结果（原生文件对象剪贴板）
  - 拷贝名称 / 路径
  - 拷贝所有名称 / 所有路径
  - 移到废纸篓
- `Cmd/Ctrl + C` 复制当前选中搜索结果（文件对象）
- 主题设置（浅色 / 深色 / 跟随系统）
- 语言设置（中文 / English）
- 快捷键设置（全局唤起/隐藏窗口）
- 启动设置（开机自启 + 静默启动）
- 索引维护设置（重建后自动 `VACUUM` 开关）
- 排除目录设置：
  - 完整目录规则 + 正则/通配符规则
  - 完整目录支持 Finder 选取
  - 规则持久化并在构建/重建时生效
- 监听根目录设置：
  - watcher 监听范围可配置，不再固定监听 `/`
  - 支持在设置页增删根目录
  - 支持 Finder 选取根目录

## 技术栈

- 核心：Rust
- CLI：Clap
- GUI 前端：React 18 + TypeScript + Vite
- GUI 容器：Tauri 2
- 全局快捷键：`tauri-plugin-global-shortcut`
- 存储：SQLite FTS5（`rusqlite`，WAL，trigram tokenizer）
- 扫描：WalkDir + Crossbeam
- 监听：macOS FSEvents（CoreServices）

## 架构总览

- `src/`：CLI 与 GUI 共用的核心引擎
- `src-tauri/`：Tauri 命令层 + 窗口生命周期 + 菜单
- `src/App.tsx`：GUI 交互层

## 构建与运行

### 环境要求

- macOS 10.15+
- Rust 1.70+
- Node.js 18+
- npm 9+

### 安装

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
```

### CLI

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

## 构建命令区别

### `npm run build`

- 只构建前端资源（TypeScript + Vite）
- 产物是 `dist/` 静态文件
- 不会编译 `src-tauri` 的 Rust 代码
- 不会生成 `.app` / `.dmg`

### `npm run tauri build`

- 构建完整桌面应用安装包
- 会先执行 `src-tauri/tauri.conf.json` 中的 `beforeBuildCommand`（当前为 `npm run build`）
- 会编译 `src-tauri/` 下的 Rust 代码
- 最终输出可安装产物（如 `.app` / `.dmg`）

## CLI 参数速查

总体语法：

```bash
machunt <COMMAND>
```

子命令：

### `search`

```bash
machunt search [OPTIONS] <QUERY>
```

- `<QUERY>`：搜索关键词
- `-p, --pattern`：通配符/正则模式（`*.rs`、`test?.txt`）
- `-F, --fuzzy`：模糊/容错搜索（Levenshtein 编辑距离）
- `-c, --case-sensitive`：区分大小写
- `-n, --limit <N>`：最大结果数（默认 100）
- `-P, --path <PATH>`：路径前缀过滤
- `-f, --files`：仅文件
- `-d, --dirs`：仅目录
- `--json`：JSON 输出

### `build`

```bash
machunt build [OPTIONS]
```

- `-p, --path <PATH>`：只构建指定范围
- `--rebuild`：先清空再重建
- `--include-dirs <true|false>`：是否索引目录（默认 `true`）

### `watch`

```bash
machunt watch
```

- 启动 FSEvents 监听
- 有 EventID 时从上次位置续跑
- 进入终端交互搜索

### `optimize`

```bash
machunt optimize [--vacuum]
```

- 默认执行 WAL checkpoint
- 可选 `--vacuum` 回收 DB 文件空间

通配符规则（`--pattern`）：

- `*`：匹配任意字符不含 `/`（单层目录）
- `**`：匹配任意字符含 `/`（所有层级）
- `?`：匹配单个字符不含 `/`
- `{a,b}`：匹配 `a` 或 `b`

## 运行时数据位置

- 索引库：`~/Library/Caches/MacHunt/index.db`
- GUI 配置：`~/Library/Application Support/MacHunt/settings.json`（快捷键 + 开机自启 + 静默启动 + 显示程序坞图标 + 重建后自动 VACUUM + 排除目录规则 + 监听根目录）
- 排除目录规则：保存在 `settings.json`（`excludeExactDirs` / `excludePatternDirs`）
- 监听根目录：保存在 `settings.json`（`watchRoots`），并同步到 DB meta（`watch_roots`）
- 日志：`~/Library/Caches/MacHunt/logs/`

## 为什么 DB 会很大

常见原因：

- 文件数量巨大（百万级很常见）
- 默认索引目录，记录数进一步增加
- 路径字符串较长，文本存储成本高
- `dirs + files` 结构虽然降低冗余，但总量大时体积依然可观
- 写入期 `index.db-wal` 会临时变大

维护建议：

```bash
machunt optimize --vacuum
```

## 许可证

MIT
