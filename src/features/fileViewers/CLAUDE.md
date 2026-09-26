# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

文件查看器注册表——策略模式实现，根据文件扩展名决定用哪个面板类型打开文件。高内聚低耦合，新增文件类型只需注册扩展名 + 实现面板组件。

## 关键约束与决策

### 策略模式 + 链式短路

- 策略/注册表结构与链式短路语义读码即得；模块级单例 `fileViewerRegistry`（同 `ShortcutRegistry` / `titleManager` 模式）。
- 默认注册抽为 `registerDefaultViewers(strategy)` 导出（TQ-B-11）——生产初始化与测试 `_reset()` 后恢复共用同一真值源。

### 测试隔离

`ExtensionBasedViewerStrategy._reset()` 和 `FileViewerRegistry._reset()` 仅测试用，清空内部状态。

## 测试模式

- 注册/解析全分支、链式短路、隐藏文件排除、大小写不敏感、无扩展名边界、`_reset` 后预注册恢复（EXP-12）。
- `handleOpenFile` 命中策略面板 / 回退 `"editor"`。
