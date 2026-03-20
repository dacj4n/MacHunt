# MacHunt

一个同时提供 CLI 和桌面 GUI 的 macOS 本地文件搜索工具。

[English](README.md)

## 版本信息

- CLI / 核心引擎：`v0.2.0`
- GUI（Tauri + React）：`v1.0.0`

## 项目概览

MacHunt 提供三部分能力：

- Rust 实现的索引与搜索引擎。
- 面向终端脚本场景的 CLI。
- 基于 Tauri + React 的原生桌面 GUI。

## 组件说明

- `CLI`：在终端完成索引构建、搜索、文件监听。
- `Core Engine`：CLI 与 GUI 共享的 Rust 核心逻辑。
- `GUI`：通过 Tauri command 直接调用本地搜索能力。

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

## 快速开始

### 1）构建 CLI

```bash
cargo build --release
```

### 2）启动 GUI（开发模式）

```bash
npm install
npm run tauri dev
```

`tauri dev` 会读取本地 `dist` 静态资源（由 `npm run build` 生成），
不需要额外的 HTTP 后端服务。

### 3）打包 GUI（生产构建）

```bash
npm run build
npm run tauri build
```

## CLI 使用

### 构建索引

```bash
# 全盘构建
machunt build

# 仅构建指定路径
machunt build --path "/Volumes/Tools"

# 强制重建
machunt build --rebuild
```

### 搜索

```bash
# 搜索文件和文件夹
machunt "测试"

# 仅搜索文件
machunt --file "测试"

# 仅搜索文件夹
machunt --folder "测试"

# 指定路径前缀过滤
machunt --path "/Volumes/Tools" "测试"
```

### 模式搜索

```bash
machunt --regex "*.pdf"
machunt --regex "*.{pdf,docx}"
machunt --regex "*.mp{3,4}"
```

### 监听模式

```bash
machunt watch
```

## GUI 说明

- GUI 与 CLI 共用同一套 Rust 核心引擎。
- 搜索请求走本地 Tauri invoke，不走 REST/HTTP。
- 索引与监听语义与 CLI 保持一致。

## 权限说明

如果需要全盘索引/监听，请授予**完全磁盘访问权限**：

1. 打开 **系统设置 → 隐私与安全性 → 完全磁盘访问权限**。
2. CLI 场景为终端应用授权。
3. GUI 场景为生成的 MacHunt 应用授权。
4. 授权后重启相关应用。

未授权时，可访问目录范围可能受限。

## 通配符规则

- `*`：匹配任意字符（不含 `/`）
- `**`：匹配任意字符（含 `/`）
- `?`：匹配单个字符（不含 `/`）
- `{a,b}`：匹配 `a` 或 `b`

## 性能参考

- 索引构建：约 10-15 秒（约 200 万文件）
- 查询延迟：典型场景 <50ms
- 内存占用：约 200-300MB（约 200 万文件）

## 架构

- 索引存储：SQLite（WAL）
- 文件监听：macOS FSEvents
- 并发索引：DashMap
- 并行扫描：Crossbeam + WalkDir
- 桌面栈：Tauri + React

## 许可证

MIT
