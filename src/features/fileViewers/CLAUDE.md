# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

文件查看器注册表——策略模式实现，根据文件扩展名决定用哪个面板类型打开文件。高内聚低耦合，新增文件类型只需注册扩展名 + 实现面板组件。

## 关键约束与决策

### 策略模式 + 链式短路

- `FileViewerStrategy` 接口 → `ExtensionBasedViewerStrategy`（扩展名→面板类型映射）→ `FileViewerRegistry`（组合多个策略，链式调用）。
- 多个策略按 `addStrategy` 顺序依次调用 `resolve()`，首个非 null 结果立即返回；全部 null 时回退默认编辑器。
- 模块级单例 `fileViewerRegistry`（同 `ShortcutRegistry` / `titleManager` 模式）。
- 默认注册抽为 `registerDefaultViewers(strategy)` 导出（TQ-B-11）——生产初始化与测试 `_reset()` 后恢复共用同一真值源。

### 解析规则

- **扩展名大小写不敏感**：注册和解析时统一 `toLowerCase()`。
- **隐藏文件排除**：`.` 开头的文件名不参与扩展名匹配（如 `.gitignore`）。
- **路径分隔符处理**：支持 `/` 和 `\`，取最后一个分隔符之后的文件名部分。

### 测试隔离

`ExtensionBasedViewerStrategy._reset()` 和 `FileViewerRegistry._reset()` 仅测试用，清空内部状态。

## 测试模式

- 注册/解析全分支、链式短路、隐藏文件排除、大小写不敏感、无扩展名边界、`_reset` 后预注册恢复（EXP-12）。
- `handleOpenFile` 命中策略面板 / 回退 `"editor"`。
