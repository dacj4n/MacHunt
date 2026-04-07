# MacHunt

一个纯本地运行的 macOS 文件/文件夹搜索工具，提供 CLI 与原生 GUI（Tauri + React）两种入口，不依赖 HTTP 后端服务。

[English](README.md)

## 版本

- GUI：`v0.2.5`
- CLI/Core：`v0.2.5`

## 最新更新（v0.2.5）

- 新增“重建后条件化自动 `VACUUM`”能力，控制索引库长期膨胀。
- 设置页新增自动 `VACUUM` 开关（默认开启），并可持久化保存。
- 限制搜索展示数量为500

## 核心能力

### CLI

- 本地索引构建/重建
- 子串与通配符搜索
- 文件/文件夹类型筛选
- 路径前缀过滤
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
  - 拷贝名称 / 路径
  - 拷贝所有名称 / 所有路径
  - 移到废纸篓
- 主题设置（浅色 / 深色 / 跟随系统）
- 语言设置（中文 / English）
- 快捷键设置（全局唤起/隐藏窗口）
- 启动设置（开机自启 + 静默启动）
- 索引维护设置（重建后自动 `VACUUM` 开关）
- 排除目录设置：
  - 完整目录规则 + 正则/通配符规则
  - 完整目录支持 Finder 选取
  - 规则持久化并在构建/重建时生效

## 技术栈

- 核心：Rust
- CLI：Clap
- GUI 前端：React 18 + TypeScript + Vite
- GUI 容器：Tauri 2
- 全局快捷键：`tauri-plugin-global-shortcut`
- 存储：SQLite（`rusqlite`，WAL）
- 内存索引：`DashMap<String, Vec<PathBuf>>`
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

## 图标生成（Tauri）

可通过以下命令，用一张 1024 图生成多尺寸图标资源：

```bash
npm run tauri icon src-tauri/icons/app-icon-1024.png
```

说明：

- 建议输入图片为正方形，最佳为 `1024x1024`。
- 命令会在 `src-tauri/icons/` 下生成各平台需要的图标文件。
- 之后执行 `npm run tauri build` 时，会自动打包这些图标（依据 `src-tauri/tauri.conf.json` 的 icon 配置）。

## CLI 参数速查

总体语法：

```bash
machunt [OPTIONS] [QUERY] [COMMAND]
```

顶层参数：

- `-p, --path <PATH>`：路径前缀过滤（`.` 表示不过滤）
- `-r, --regex`：通配符搜索模式
- `--folder`：仅目录
- `--file`：仅文件
- `--logs`：输出日志到 `~/.machunt/logs`
- `[QUERY]`：不带子命令时的查询关键词

筛选规则：

- 未传 `--file` 且未传 `--folder`：同时搜索文件和文件夹
- 仅 `--file`：只搜索文件
- 仅 `--folder`：只搜索目录

子命令：

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

通配符规则（`--regex`）：

- `*`：匹配任意字符（不含 `/`）
- `**`：匹配任意字符（可含 `/`）
- `?`：匹配单个字符（不含 `/`）
- `{a,b}`：匹配 `a` 或 `b`

## 运行时数据位置

- 索引库：`~/.machunt/data/index.db`
- GUI 配置：`~/.machunt/gui/settings.json`（快捷键 + 开机自启 + 静默启动 + 重建后自动 VACUUM + 排除目录规则）
- 排除目录规则：保存在 `~/.machunt/gui/settings.json`（`excludeExactDirs` / `excludePatternDirs`）
- 日志：`~/.machunt/logs/`

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
