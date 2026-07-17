# Stage 5 逐项验证断言（唯一真值源）

> stage-05-frontend-critical.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-01**：`useCodeMirror.ts` 存在 generation 比对（`genRef.current !== gen` 或 AbortController），异步回调先比对再写 `viewRef`；有"快速切换 A→B 最终显示 B 且 A 已 dispose"测试用例
- **FE-19**：`repoDir` 计算无 `slice(0, lastIndexOf("/"))` 裸逻辑；无有效父目录时跳过 gitDiff
- **FE-02**：`injectScript.ts` 含 `</script>` → `<\/script>` 转义逻辑（大小写不敏感）；`inject-script.test.ts` 有对应用例
- **SEC-03**：`HtmlPanel.tsx` 的 `handleMessage` 校验 `e.origin`（预期 `"null"`，代码注释记录该假设来源——opaque origin 规范，正确性由收尾 L4 验证）且校验 `e.source` 为本面板 iframe.contentWindow；注入脚本无 `postMessage(..., "*")`；`html-panel.test.tsx` 有"伪造 origin 消息被忽略"用例

## 全量测试（5 条全部通过）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
