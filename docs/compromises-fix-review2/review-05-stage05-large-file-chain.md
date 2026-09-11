# Review2 · Stage 05 大文件链路（BE-05+FE-03/04/05/08）

> commit: f945ed0；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查。
> 仅登记问题，不分严重度；已核验通过项不列出。

## 问题清单

### R2-BE-01 · Stage 进度表登记错位——f945ed0 未登记本 Stage 行（行 04 反由本提交捎带补做）
- **关联修复项**: —（流程保真度）
- **位置**: docs/compromises-fix-review-fix/execution-plan.md:84-85（f945ed0 树内）
- **问题**: f945ed0 提交时进度表行 05 仍为「未开始」，且本提交实际改动的是行 04（Stage 04 的登记，应由 2ecd7cb 自己完成）——S04 迟登记由 S05 捎带、S05 自身登记再由后续提交（75f7d74）回补，登记动作与 Stage 提交错位两级。S01~S03 均为各自提交自登，此处破坏既定收尾节奏；当前 HEAD 已补齐，属已回补的流程欠账。
- **证据**: `git show f945ed0:docs/compromises-fix-review-fix/execution-plan.md | grep "| 05"` → `| 05 | 未开始 | | | TE-03 守卫首次真实触发 |`；同提交 diff 仅改行 04；当前 HEAD 行 85 = `| 05 | 完成 | f945ed0 | …`。
- **建议**: systematic-changes-execute 收尾约定中明确「提交即登记本 Stage 行」，迟登记不得跨 Stage 捎带。

### R2-FE-01 · 在途旧代际单飞结果可被复位后的新扫描直接消费——陈旧文本进新行索引，且此后不再触发失效
- **关联修复项**: FE-05
- **位置**: src/panels/editor/largeFileViewer/blockCache.ts:46-47（inflight 命中直返）+ :58（gen 比对只门控回填）
- **问题**: `invalidateFile` 只清 cache 与推进代际，`inflight` 保留。失效 + setFileRev 复位后，useLineIndex 重建工作区重扫，首个 `readBlock` 命中仍在途的旧任务（`inflight.get(key)` 直返，无 gen 复核）——旧任务代际不符只是不写缓存，其返回文本（失效前字节的快照）仍被新扫描用来构建行索引。更糟的是事件处理器已先把 `baseMetaRef` 更新为修改后的 meta，此后 mtime/size 未再变的事件不会触发第二次失效 → 陈旧索引可滞留到下一次磁盘修改。窗口虽窄（失效须恰好落在块读在途期），但代际机制本意是「旧代际结果不生效」，当前实现只保住了缓存、没保住消费端。
- **证据**: 读码 blockCache.ts:37-69——:46-47 inflight 直返无 gen 校验；:58 gen 比对仅包 `cache.set`。现有测试「读取途中失效的旧代际结果不回填缓存」只断言 `peekCachedBlock` 未回填 + 调用方收响应，未覆盖「复位后二次 readBlock 消费同一在途任务」的交错。
- **建议**: readBlock 命中 inflight 时按当前 gen 复核（gen 不符则挂起等待其 finally 清除后另起新任务），或 invalidateFile 顺带删除该文件前缀 inflight 条目（在途调用方仍收响应，新扫描不再复用）。

### R2-FE-02 · 外部修改重载路径（applyExternalChange）全量 readFile 无任何大小防线——FE-08 语义链的既有缺口
- **关联修复项**: FE-08（存量问题，本次未触碰该路径）
- **位置**: src/panels/editor/useCodeMirror.ts:558-568
- **问题**: FE-08 为打开路径建立了「stat 预检 + 读后复核 → 超限引导只读浏览」的语义链，但外部修改触发的重载路径仍 `fs.readFile(path)` 全量读盘直接灌进 CM：打开时 5MB 的文件被外部工具改大到 50MB 后，一次 Modify 事件即可把 50MB 全量载入内存并建 EditorView，无任何 stat 预检/读后复核/largeFile 引导。内存保护链路在重载面出现断口，且与 editor/CLAUDE.md 新补的「四层防线」表述不自洽。
- **证据**: useCodeMirror.ts:560 `content = await fs.readFile(path);`——所在函数 `applyExternalChangeRef.current`（:546）经 fs-event（:631）与 CP-029 打开后核对（recheck）双路径调用，全链无 statFile/MAX 判定；git show f945ed0 该函数零改动。
- **建议**: applyExternalChange 读盘前补 fs.statFile 预检（>MAX 置 largeFile 信号引导只读浏览，与打开路径同语义），至少保留读后 doc.length 复核。

### R2-FE-03 · 首挂 stat 基线时序竞态——base null 期间事件被跳过 + 基线后设于修改之后时，陈旧索引失去再触发器
- **关联修复项**: FE-05
- **位置**: src/panels/editor/largeFileViewer/LargeFileViewer.tsx:81-125
- **问题**: 索引扫描在首渲染即开始（getLine 探针），首挂 stat effect 后于其完成。交错窗口：扫描读到修改前字节 → 修改发生 → Modify 事件到达时 `baseMetaRef.current` 仍为 null 被跳过 → 首挂 stat resolve 于修改之后，基线直接 = 新 meta。此后同 mtime/size 的事件全部判「未变化」，陈旧索引无任何再失效触发点。注释声称「下次事件或重挂基线兜底」——「重挂基线」在同文件不换 filePath 时不会发生（挂载 effect dep 仅 `[filePath]`，不重跑），兜底说法不成立。editor 域有 CP-029 打开后核对定时器补偿，viewer 域无对应补偿。
- **证据**: LargeFileViewer.tsx:84（基线先清）/ :88（resolve 才回填）；:112-113 基线 null 即 return；useEffect dep :97 `[filePath]`。无测试覆盖该交错。
- **建议**: 首挂 stat resolve 时若行索引已有推进（nextBlock>0）则保守触发一次 invalidateFile + setFileRev+1（成本一次重扫，换窗口封闭），或对 base null 期间到达的 Modify 做 pending 标记、基线就绪后补一次比对。

### R2-FE-04 · 失效触发依赖 mtime+size 比对，「同 size 同 mtime 改写」假阴性窗口未登记
- **关联修复项**: FE-05（边界登记缺口）
- **位置**: src/panels/editor/largeFileViewer/LargeFileViewer.tsx:113；src/panels/editor/CLAUDE.md（大文件四层防线节「外部修改失效」句）
- **问题**: 失效判定 = `mtimeMs` 或 `sizeBytes` 变化。存在静默漏检面：定长原位改写（改内容不改大小）且 mtime 粒度未变（FAT/exFAT 2s 粒度、或同毫秒内两次写）→ 比对相等 → 不失效，viewer 展示旧内容直到下次可感知修改。editor 域 useCodeMirror 的先例是读全文判等（无此滤口），该比对层是 viewer 新增；CLAUDE.md 边界登记只写了「事件丢失残余窗口」，未覆盖比对口径自身的假阴性。
- **证据**: LargeFileViewer.tsx:113 比对条件；editor/CLAUDE.md「外部修改失效：…事件丢失残余窗口为已知边界」——无同 size+mtime 条目；useCodeMirror.ts:575 先例为内容判等短路。
- **建议**: CLAUDE.md 边界句补「定长改写 + mtime 粒度未变场景漏检」口径（或在句内说明与 editor 域内容判等先例的差异及取舍理由）。
