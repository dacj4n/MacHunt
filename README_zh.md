# MacHunt

macOS 全局文件搜索工具

[English](README.md)

## 功能特性

- ⚡ **高性能**: 数秒内完成全盘搜索
- 🔍 **实时监控**: 实时监控文件变化
- 📂 **灵活搜索**: 支持子串搜索、正则表达式、文件/文件夹过滤
- 🔄 **增量索引**: 基于事件ID的快速重建
- 🗄️ **持久化存储**: 基于 SQLite 的索引存储

## 系统要求

- macOS 10.15+
- Rust 1.70+

## 安装

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
cargo build --release
```

## 使用方法

### 首次使用

```bash
# 构建文件索引（首次使用必须执行）
machunt build          # 全盘扫描，约 15-20 秒

# 或构建指定路径的索引
machunt build --path "/Volumes/Tools"
```

### 搜索

```bash
# 搜索包含"测试"的文件和文件夹
machunt "测试"

# 只搜索文件
machunt --file "测试"

# 只搜索文件夹
machunt --folder "测试"
```

### 正则搜索

```bash
# 搜索所有 pdf 文件
machunt --regex "*.pdf"

# 搜索 pdf 和 docx 文件
machunt --regex "*.{pdf,docx}"

# 搜索特定模式的文件
machunt --regex "*.mp{3,4}"
```

### 路径过滤

```bash
# 在指定目录下搜索
machunt --path "/Volumes/Tools" "测试"
```

### 实时监控

```bash
# 启动实时监控（后台保持运行）
machunt watch

# 然后可以随时搜索
machunt "测试"
```

## 权限说明

**重要**：如需监控所有目录，需要授予终端完整的磁盘访问权限：

1. 打开**系统设置 → 隐私与安全性 → 完全磁盘访问权限**
2. 点击锁图标并输入密码
3. 点击 **+** 添加您的终端应用（Terminal.app、iTerm2 等）
4. 重启终端

未授予权限时，监控仅对 `/Users` 目录有效。

## 通配符规则

- `*` - 匹配任意字符（不含 `/`）
- `**` - 匹配任意字符（含 `/`）
- `?` - 匹配单个字符（不含 `/`）
- `{a,b}` - 匹配 `a` 或 `b`

## 使用示例

```bash
# 搜索所有视频文件
machunt --regex "*.{mp4,mov,avi}"

# 搜索所有 PDF 文件
machunt --regex ".*\.pdf"

# 在指定路径下搜索测试文件
machunt --path "/Volumes/工作" "测试"

# 监控文件变化
machunt watch
```

## 性能指标

- 索引构建: 200万文件约 10-15 秒
- 搜索响应: 典型查询 <50ms
- 内存占用: 200-300MB（200万文件）

## 架构设计

- **索引存储**: SQLite 数据库，启用 WAL 模式
- **文件监控**: macOS FSEvents API
- **并发访问**: DashMap 无锁访问
- **并行处理**: Crossbeam 多线程扫描

## 许可证

MIT
