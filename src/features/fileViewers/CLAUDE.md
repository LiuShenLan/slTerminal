# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件查看器注册表——策略模式实现，根据文件扩展名决定用哪个面板类型打开文件。高内聚低耦合，新增文件类型只需注册扩展名 + 实现面板组件。

## 架构决策

**策略模式**：`FileViewerStrategy` 接口 → `ExtensionBasedViewerStrategy`（扩展名→面板类型映射）→ `FileViewerRegistry`（组合多个策略，链式调用）。

**链式调用 + 短路**：多个策略按 `addStrategy` 顺序依次调用 `resolve()`，首个非 null 结果立即返回，后续策略不执行。所有策略返回 null 时回退默认编辑器。

**模块级单例**：`fileViewerRegistry` 全局唯一实例（同 `ShortcutRegistry` / `titleManager` 模式），预注册 `html`/`htm` → `"htmlviewer"`。

## 文件

| 文件 | 职责 |
|------|------|
| `FileViewerRegistry.ts` | 策略接口 + ExtensionBasedViewerStrategy + FileViewerRegistry + fileViewerRegistry 单例 |
| `index.ts` | barrel export |

## 扩展新文件类型

三步注册：

```typescript
// 1. 在 FileViewerRegistry.ts 的 extensionStrategy 上注册
extensionStrategy.register("md", "markdownviewer");

// 2. 实现面板组件 → src/panels/markdown/
// 3. 在 panelRegistry.ts 注册 → PANEL_TYPES 追加 → FILE_PANEL_TYPES 追加（如适用）
```

## 关键约束

- **扩展名大小写不敏感**：注册和解析时统一 `toLowerCase()`
- **隐藏文件排除**：`.` 开头的文件名（如 `.gitignore`）不参与扩展名匹配
- **路径分隔符处理**：支持 `/` 和 `\`，取最后一个分隔符之后的文件名部分
- **_reset() 测试隔离**：`ExtensionBasedViewerStrategy._reset()` 和 `FileViewerRegistry._reset()` 仅测试用，清空内部状态
