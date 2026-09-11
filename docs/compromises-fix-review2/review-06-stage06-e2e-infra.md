# Review2 · Stage 06 e2e 设施（TE-04/05/06/08/09）

> commit: 857dca0；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查；e2e 静态 review（不实跑）。
> 仅登记问题，不分严重度；已核验通过项不列出。

## 问题清单

### R2-TE-01 · WARN 计数按 chunk 独立匹配，跨 chunk 断行漏计（且多字节 UTF-8 截断乱码）
- **关联修复项**: TE-09
- **位置**: `e2e-tests/run-wdio.cjs:40-46`
- **问题**: `wireWarnCounting` 对每个 `data` chunk 独立 `s.match(/core\.invoke not available after 5s/g)`——目标 WARN 字符串若恰好跨 chunk 边界（前半在上一 chunk 尾部、后半在下一 chunk 头部），两个 chunk 都不命中，计数静默漏计；该计数是「focus 命令面扩大」回归感知锚点，漏计直接削弱其观测价值。同处 `chunk.toString()` 对跨 chunk 的多字节 UTF-8 序列会产生替换字符，转发日志乱码。
- **证据**: 实读 run-wdio.cjs:40-46——无尾部残串拼接缓冲；stdio 改 pipe 后子进程输出按 64KB 高水位切 chunk，长行 WARN 恰可跨块。
- **建议**: 维护上一次 chunk 尾部残串（保留 `目标串长度-1` 字符），与本 chunk 拼接后再匹配计数。

### R2-TE-02 · fallback 通道 `process.exit(code)` 未做 `?? 1`，信号杀死被掩为 exit 0
- **关联修复项**: TE-09（存量形态沿留，本次重写未修）
- **位置**: `e2e-tests/run-wdio.cjs:353-356`
- **问题**: `'close'` 事件在子进程被信号杀死时 `code === null`，`process.exit(null)` 实测退化为 exit 0（已实跑验证：`node -e "process.exit(null)"` → 0；SIGKILL 子进程 close code=null → 同样退 0）——wdio 子进程被外部杀死时启动器报成功。同 commit 内 `runWdio` 通道已写 `process.exit(code ?? 1)`（:318），两通道行为不一致。
- **证据**: 实读 run-wdio.cjs:355 vs :318；node 实跑两条命令确认 null → 0。
- **建议**: fallback close 回调对齐 runWdio 形态改 `process.exit(code ?? 1)`。

### R2-TE-03 · workspace CLAUDE.md 右键菜单工厂签名登记过期（第 4 参形态已改回调）
- **关联修复项**: —（连带修复面 docViewer/workspace 文档同步漏网，存量文档失真）
- **位置**: `src/workspace/CLAUDE.md:75`
- **问题**: 该处仍登记 `createTabMenuItems(getApi, pageId|null, onRenameRequest, projectRootPath?)`，本次连带修复已将第 4 参改为 `getProjectRootPath?: (pageId: string|null) => string | undefined` 回调（tabChrome.tsx:254）——签名文档与实现漂移，后续按文档调用即错。
- **证据**: 实读 src/workspace/CLAUDE.md:75（`projectRootPath?` 静态值形态）vs tabChrome.tsx:247-255（回调形态）。
- **建议**: workspace CLAUDE.md 该处签名同步为回调形态并注明 S11 回归缘由。
