# mac_find

macOS 全局文件搜索工具 - 类似 Windows 的 Everything

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
git clone https://github.com/dacj4n/mac_find.git
cd mac_find
cargo build --release
```

## 使用方法

### 基础搜索

```bash
# 搜索包含"测试"的文件和文件夹
mac_find "测试"

# 只搜索文件
mac_find --file "测试"

# 只搜索文件夹
mac_find --folder "测试"
```

### 正则搜索

```bash
# 搜索所有 pdf 文件
mac_find --regex "*.pdf"

# 搜索 pdf 和 docx 文件
mac_find --regex "*.{pdf,docx}"

# 搜索特定模式的文件
mac_find --regex "*.mp{3,4}"
```

### 路径过滤

```bash
# 在指定目录下搜索
mac_find --path "/Volumes/Tools" "测试"
```

### 索引管理

```bash
# 构建文件索引
mac_find build

# 实时监控文件变化
mac_find watch
```

## 通配符规则

- `*` - 匹配任意字符（不含 `/`）
- `**` - 匹配任意字符（含 `/`）
- `?` - 匹配单个字符（不含 `/`）
- `{a,b}` - 匹配 `a` 或 `b`

## 使用示例

```bash
# 搜索所有视频文件
mac_find --regex "*.{mp4,mov,avi}"

# 搜索所有 PDF 文件
mac_find --regex ".*\.pdf"

# 在指定路径下搜索测试文件
mac_find --path "/Volumes/工作" "测试"

# 监控文件变化
mac_find watch
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
