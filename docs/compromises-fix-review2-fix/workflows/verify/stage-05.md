# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-07**：Read `src/panels/editor/largeFileViewer/blockCache.ts` invalidateFile 确认：除 cache 前缀清除外含 inflight 前缀清除段（语义式——失效后新扫描的 readBlock 不复用旧代际在途任务）；头注口径已更新（在途调用方仍收响应、旧任务 finally delete 幂等）。
- **FE-09**：`rg "scannedBlocks" src/panels/editor/largeFileViewer/useLineIndex.ts src/panels/editor/largeFileViewer/LargeFileViewer.tsx` 两文件各命中；Read useLineIndex return 确认含 `scannedBlocks` 字段；Read LargeFileViewer 首挂 stat effect 确认：基线回填后 `scannedBlocksRef.current > 0` → invalidateFile + setFileRev 保守失效段存在；事件 effect 基线 null 跳过注释已修正（不再声称「重挂基线兜底」）。
- **FE-10**：`rg "sampleFingerprint" src/panels/editor/largeFileViewer/LargeFileViewer.tsx` ≥ 3 命中（定义 + 首挂/事件两调用点）；Read 确认：baseMetaRef 含 fingerprint 字段；事件比对 = mtime/size 变化先失效、mtime/size 未变时抽样指纹复核、指纹异亦失效；取样 = 首/中/末三段各 4KB。
- **FE-08**：`rg "setLargeFile" src/panels/editor/useCodeMirror.ts` ≥ 4 命中（打开路径 2 + 重载路径 2）；Read applyExternalChange 确认：statFile 预检在 readFile 前、超限置 largeFile + filePathRef 清空 + return；读后 `content.length > MAX_FILE_SIZE_BYTES` 复核同形态；脏分支确认弹窗仍在 stat 预检之前（顺序未重排）。
- **FE-07/09/10/08（文档面）**：Read src/panels/editor/CLAUDE.md 大文件节确认含：抽样指纹复核口径（首/中/末三段 4KB + 残余已知边界登记）、首挂基线竞态封闭（FE-09）、外部修改重载路径 stat 预检 + 读后复核（FE-08 重载面）三处口径。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npx vitest run large-file-viewer`
4. `npx vitest run use-code-mirror`
5. `npm test`
