# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件系统操作模块——封装 `fs_read_dir`、`fs_read_file`、`fs_write_file`、`fs_create_dir`、`fs_delete`、`fs_rename` Tauri 命令，适配 Windows 行尾和路径沙箱。

## 测试模式

测试位于 `fs/mod.rs` 的两个 `#[cfg(test)]` 模块：`read_dir_tests`（10 用例）+ `write_file_tests`（5 用例）。

### tempfile 隔离

所有文件系统测试使用 `tempfile::tempdir()` 创建隔离的临时目录，测试结束时自动清理：

```rust
let dir = tempfile::tempdir().unwrap();
let file = dir.path().join("test.txt");
std::fs::write(&file, "hello").unwrap();
// ... test operations ...
// dir 被 drop，所有临时文件自动删除
```

### 测试组织

- `read_dir_tests`：列出子项、过滤 `.git`（仅此一个硬编码过滤，`node_modules`/`target` 等依赖懒加载控制性能）、创建目录（单层/嵌套）、删除（文件/递归目录）、重命名、空目录边界、构建产物目录可见
- `write_file_tests`：CRLF 行尾保留（CRLF→CRLF、LF→LF）、新文件平台默认行尾（Windows CRLF / Unix LF）、混合行尾归一化

### CRLF 行尾保留测试

项目约束：编辑器保存文件时必须保持原行尾格式。测试验证：

```rust
// CRLF 文件改写后仍是 CRLF
let content = b"line1\r\nline2\r\n";
std::fs::write(&path, content).unwrap();
write_file(&path, content)?; // 项目自己的 write
assert_eq!(std::fs::read(&path)?, content);

// LF 文件改写后仍是 LF
let content = b"line1\nline2\n";
// ... 同上 ...
```

### 平台分支

- `new_file_defaults_to_crlf_on_windows`：使用 `#[cfg(windows)]` / `#[cfg(not(windows))]` 守护不同断言
- 其余测试平台无关（tempfile 跨平台工作）
