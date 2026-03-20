# MacHunt

一个同时提供 CLI 和桌面 GUI 的 macOS 本地文件/文件夹搜索工具（纯本地索引与检索，不依赖 HTTP 后端服务）。

[English](README.md)

## 版本（2026-03-21）

- GUI：`v1.1.0`
- CLI / Core：`v0.2.1`

> 历史版本：GUI `v1.0.0`、CLI/Core `v0.2.0`

## 本次更新（v1.1.0 / v0.2.1）

### 新增

- GUI 启动后默认自动开启 Watch（监听）
- 搜索结果右键菜单：
  - 打开
  - 打开于 ...（子菜单：在Finder中查看 / 在QSpace Pro中查看）
  - 拷贝名称 / 拷贝路径
  - 移到废纸篓
- 搜索结果支持双击打开（文件按系统关联打开，目录在 Finder 打开）
- 路径筛选支持“下拉选择 + 手动输入 + Finder 选取路径”
- 表头四列支持点击排序（升序/降序切换，当前列显示三角标识）
- 列宽拖拽后自动记忆，下次启动恢复

### 修复

- 修复“打开于 ...”子菜单鼠标移动时容易消失的问题
- 修复窗口缩小时列表布局挤压/遮挡问题，表格列改为稳定省略显示
- 修复列拖拽命中区过小导致“看起来无法拖拽”的问题
- 修复深色主题下操作按钮高亮不明显的问题
- 修复输入框鼠标样式不正确（输入框恢复文本编辑光标）
- 修复重复索引/重复结果相关问题（构建与内存更新路径做去重和增量控制）

### 优化

- 数据库模型优化为 `dirs + files`（目录去重）
- 删除旧索引 `idx_name`，降低冗余索引占用
- `build --path` 小范围构建时改为增量更新内存，避免每次全量回填
- 增加启动验证清理（清除索引中已不存在路径）
- 优化查询路径：子串与通配符匹配都先做低成本预过滤

## 技术栈

- 核心语言：Rust
- CLI 参数解析：Clap
- GUI：Tauri 2 + React 18 + TypeScript + Vite
- 索引存储：SQLite（WAL）
- 内存索引：DashMap
- 扫描：WalkDir + Crossbeam
- 监听：macOS FSEvents（CoreServices）

## 架构与原理

### 架构分层

- `Core Engine`（Rust）
  - 负责扫描、建库、加载内存索引、搜索、监听增量更新
- `CLI`（Rust）
  - 调用同一套 Core Engine
- `GUI`（Tauri + React）
  - 前端通过 Tauri command 直接调用本地 Rust 能力
  - 不需要 REST API，不启动 1420 之类的后端端口

### 数据流

1. `build` 扫描磁盘路径，批量写入 SQLite
2. 将索引加载/更新到内存结构 `DashMap<String, Vec<PathBuf>>`
3. `search` 在内存索引检索后返回路径
4. `watch` 接收 FSEvents 变更，增量更新 SQLite + 内存索引

## 为什么 DB 会很大

即使已做优化，DB 在百万级文件场景仍可能很大，主要原因：

- 文件数量本身巨大（行数高）
- 仍需存储名称与目录关系，并维护必要索引
- WAL 文件在高频写入期间会临时增长
- 目录索引默认开启（会比只索引文件更大）
- 路径和文件名长度分布不均（长路径会放大占用）

建议定期执行：

```bash
machunt optimize --vacuum
```

## 项目目录

- `src/`：CLI 与核心引擎
  - `main.rs`：CLI 入口
  - `engine.rs`：引擎编排
  - `builder.rs`：扫描与构建
  - `search.rs`：搜索逻辑
  - `watcher.rs`：FSEvents 监听
  - `db.rs`：SQLite 持久化
- `src-tauri/`：GUI 后端（Tauri commands）
- `src/App.tsx`：GUI 前端主界面

## 环境要求

- macOS 10.15+
- Rust 1.70+
- Node.js 18+
- npm 9+

## 安装

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
```

## 构建与运行

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

## CLI 全参数说明

CLI 总体语法：

```bash
machunt [OPTIONS] [QUERY] [COMMAND]
```

### 顶层参数（适用于直接搜索与 watch 内交互搜索）

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---:|---|
| `-p, --path <PATH>` | string | `.` | 搜索路径前缀过滤。`'.'` 表示不过滤 |
| `-r, --regex` | flag | `false` | 使用通配符模式（不是 PCRE 完整正则） |
| `--folder` | flag | `false` | 仅搜索目录 |
| `--file` | flag | `false` | 仅搜索文件 |
| `--logs` | flag | `false` | 打开日志输出（写入 `~/.machunt/logs`） |
| `[QUERY]` | string | `""` | 直接搜索的关键词（不带子命令时必填） |

筛选逻辑说明：

- 未设置 `--file` 且未设置 `--folder`：默认同时搜索文件和目录
- 仅 `--file`：只返回文件
- 仅 `--folder`：只返回目录
- 同时设置：等效为“都搜索”

### 子命令与参数

#### 1) `build`

```bash
machunt build [OPTIONS]
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---:|---|
| `-p, --path <PATH>` | string | 无 | 仅构建指定路径范围 |
| `--rebuild` | flag | `false` | 先清空旧索引再重建 |
| `--include-dirs <true\|false>` | bool | `true` | 构建时是否包含目录项 |

示例：

```bash
# 全量构建
machunt build

# 构建指定路径
machunt build --path "/Volumes/Tools"

# 全量重建
machunt build --rebuild

# 只索引文件（不索引目录）
machunt build --include-dirs false
```

#### 2) `watch`

```bash
machunt watch
```

说明：

- 启动文件系统监听（FSEvents）
- 进入交互模式，终端持续输入关键词实时搜索
- 退出时会保存当前 EventID，便于下次增量续跑

可与顶层参数组合：

```bash
# 监听 + 仅文件 + 路径前缀过滤 + 通配符模式
machunt --file --path "/Users/dcj" --regex watch
```

#### 3) `optimize`

```bash
machunt optimize [OPTIONS]
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---:|---|
| `--vacuum` | flag | `false` | 除了 WAL checkpoint，还执行 VACUUM 回收空间 |

示例：

```bash
# 仅做 WAL checkpoint
machunt optimize

# checkpoint + VACUUM
machunt optimize --vacuum
```

### 直接搜索示例

```bash
# 子串搜索（默认）
machunt "test"

# 指定路径前缀
machunt --path "/Volumes/Tools" "test"

# 仅文件
machunt --file "app"

# 仅目录
machunt --folder "java"

# 通配符模式
machunt --regex "*app*.*"
```

### 通配符规则（`--regex`）

- `*`：匹配任意字符（不含 `/`）
- `**`：匹配任意字符（可含 `/`）
- `?`：匹配单个字符（不含 `/`）
- `{a,b}`：匹配 `a` 或 `b`

## GUI 使用说明

### 基础使用

1. 启动 GUI 后会自动开启 Watch（默认监听）
2. 在顶部输入关键词开始搜索
3. 使用路径过滤框缩小范围（支持输入/下拉/Finder 选取）
4. 选择子串或通配符模式，必要时开启大小写匹配
5. 结果区支持双击打开、右键操作、列宽拖拽与排序

### GUI 控件与参数语义

| 控件 | 对应语义 | 说明 |
|---|---|---|
| 搜索框 | `query` | 关键词 |
| 路径过滤 | `pathPrefix` | 路径前缀过滤 |
| 子串/通配符 | `mode` | `Substring` / `Pattern` |
| `Aa` | `caseSensitive` | 大小写敏感开关 |
| 构建 | `build_index(rebuild=false)` | 增量/常规构建 |
| 重建 | `build_index(rebuild=true)` | 清空后重建 |
| Start/Stop Watch | `start_watch_auto` / `stop_watch` | 监听启停 |
| 分类 Tab | include files/dirs + 类型筛选 | 文件、文件夹、文档等 |
| 表头点击 | `sortKey + asc/desc` | 四列排序，当前列显示箭头 |

### 结果列表交互

- 双击行：打开文件/目录
- 右键菜单：
  - 打开
  - 打开于 ...
    - 在Finder中查看
    - 在QSpace Pro中查看
  - 拷贝名称
  - 拷贝路径
  - 移到废纸篓
- 列宽拖拽：支持并会记忆
- 过长文本：省略号 + hover 提示完整路径/名称

### 设置页

- 主题：浅色 / 深色 / 跟随系统
- 菜单项：macOS 菜单固定显示 `Preferences`

## Tauri 命令（面向二次开发）

| 命令 | 参数 | 作用 |
|---|---|---|
| `initialize` | - | 初始化加载索引 |
| `search` | `SearchRequest` | 执行搜索 |
| `build_index` | `path?, rebuild, include_dirs?` | 构建索引 |
| `start_watch_auto` | - | 自动策略启动监听 |
| `stop_watch` | - | 停止监听 |
| `watch_status` | - | 获取监听状态 |
| `list_path_suggestions` | - | 路径下拉建议 |
| `pick_path_in_finder` | - | 打开 Finder 选路径 |
| `open_search_result` | `path` | 打开文件或目录 |
| `reveal_in_finder` | `path` | Finder 定位 |
| `open_in_qspace` | `path` | QSpace Pro 打开 |
| `copy_to_clipboard` | `text` | 写入剪贴板 |
| `move_to_trash` | `path` | 移到废纸篓 |
| `set_menu_language` | `language` | 菜单文案同步 |
| `persist_watch_cursor` | - | 持久化 EventID |

`SearchRequest` 字段：

- `query: string`
- `mode: "Substring" | "Pattern"`
- `caseSensitive?: boolean`
- `pathPrefix?: string`
- `includeFiles?: boolean`
- `includeDirs?: boolean`
- `limit?: number`（GUI 当前请求默认 2500）

## 数据与日志位置

- 索引数据库：`~/.machunt/data/index.db`
- 日志目录：`~/.machunt/logs/`
- 列宽记忆（GUI）：浏览器本地存储 `machunt.table.column.widths`

## 默认跳过路径（构建/监听）

- `/dev`
- `/proc`
- `/sys`
- `/private/var/vm`
- `/private/var/run`
- `/private/var/folders`
- `/System/Volumes/Data`
- `/System/Volumes/Preboot`
- `/System/Volumes/Recovery`
- `/System/Volumes/VM`
- 包含 `/.Spotlight-V100`、`/.fseventsd` 的路径

## 权限要求（macOS）

为保证全盘搜索/监听覆盖率，请授予“完全磁盘访问权限”：

1. 系统设置 → 隐私与安全性 → 完全磁盘访问权限
2. CLI 场景：给终端应用授权
3. GUI 场景：给 MacHunt 应用授权
4. 授权后重启终端或应用

## 常见问题

### 1）为什么有些路径搜不到？

- 路径可能在默认跳过列表中
- 未授权完全磁盘访问
- 索引尚未覆盖该目录（先构建/重建）

### 2）为什么会感觉重复结果？

- 旧索引残留、并发构建或历史数据可能造成体感重复
- 建议先执行：

```bash
machunt build --rebuild
machunt optimize --vacuum
```

### 3）为什么 build/rebuild 有时看起来卡住？

- 大规模索引时 DB 写入与内存更新需要时间
- watch 模式下首次无索引会后台构建

## 许可证

MIT
