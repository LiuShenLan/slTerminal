# Document 核心（slterm_document）

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`；仅审查当前工作树中的 `slterm_document` 及其契约文档、追溯锚，未运行构建、测试、网络或真实文件操作。

## 覆盖与局限

- 已覆盖：`slterm_document/src` 下编辑事务与撤销历史、TextStateId／DocumentId、DocumentRegistry 聚合与身份索引、保存回执与代次、外部变化与重授权、编码解码与尺寸路由、分页索引与页缓存、后台查找／复制／替换计划、LanguageRegistry、SyntaxWorker 与高亮、Diff、Markdown 投影与 HTML／SVG 安全投影、Repository 与 CommitId／RepoPath。
- 已覆盖文档：`slterm_document/CLAUDE.md`、`docs/devPlan/design/document.md`、`docs/devPlan/design/domain-model.md`、`docs/featureSpec/panels/document.md`、`docs/featureSpec/panels/editor.md`、`docs/devPlan/traceability.md` 的 Document／Editor 行、`.claude/docs/testing/strategy.md`。
- 未覆盖：应用层 GPUI 视图、真实 I/O 与路径解析、`PagedDocument` 的实际页边界生成、快照信封写入器与恢复接线、`usvg`／`resvg` 的真实光栅化行为；这些仅在与本 crate 契约交界处取证。未运行任何测试，测试结论均来自源码与测试体语义。

## 问题

### 阻断｜SVG 清洗重建 XML 时未转义属性值，可注入白名单外属性与元素

- 置信度：确认
- 位置：`slterm_document/src/markdown_svg.rs` 的 `clean_element`、`SanitizedSvg::parse`；依赖 `quick-xml` 的 `Attribute::decoded_and_normalized_value`、`BytesStart::push_attribute` 与 `Writer::write_event`；测试 `safe_static_svg_preserves_local_paint_and_predefined_entities`、`svg_dtd_external_references_and_active_content_are_rejected` 未覆盖该路径。
- 现象：`decoded_and_normalized_value` 会把 `&quot;`、`&lt;`、`&gt;`、`&amp;` 等预定义实体解析成真实字符，随后 `cleaned.push_attribute((raw_key, value))` 把解析后的值交给 `quick-xml` 写出。`quick-xml` 的 `BytesStart::push_attr` 直接把值拼进双引号属性且不转义（依赖源码在该处保留未转义的 FIXME），因此单引号源属性中的字面双引号，或 `&quot;`／`&#34;` 形式的引用，都会在输出中变成未转义双引号。例如一个允许标签上的 `id='x" onload="evil'` 会重建为带 `onload` 属性的元素；值中包含 `">` 时还可提前闭合起始标签并注入新的子元素。
- 根因：清洗器只校验属性名与属性值形状，重建 XML 时没有对实体解析后的值做 XML 转义，也没有把属性值中的 XML 元字符视为不安全事实。
- 影响：`SanitizedSvg` 的封闭白名单承诺失效，事件属性、滤镜等被整图拒绝的构造可以重新进入输出；当前应用虽通过 `usvg` 的 `image_href_resolver` 关闭图片外部解析，但滤镜仍可能绕过像素预算，任何复用该核心 API 且不额外关闭脚本／事件处理的消费方会直接接收注入后的 XML。
- 证据：`clean_element` 使用 `decoded_and_normalized_value` 后原样 `push_attribute`；`quick-xml` 的 `push_attr` 注释明确属性内容尚未转义；现有 SVG 测试只使用普通属性值或文本实体，没有引号实体、单引号属性内双引号或闭合标签注入用例。
- 优化方向：在实体解析后拒绝或转义 `<`、`>`、`"`、`&` 等元字符；把任何需要重写的属性值视为不可信并走安全转义写回；补充针对引号实体、单引号属性和闭合标签注入的负例。

### 重要｜同一规范路径的外部重载在文件身份变化时覆写其他存活 Document 的身份索引

- 置信度：确认
- 位置：`slterm_document/src/external.rs` 的 `apply_external`／`DocumentExternalChange::Reload`；`slterm_document/src/registry.rs` 的 `refresh_identity`、`identities`、`existing`；相关测试未覆盖“同路径换身份且新身份属于另一存活 Document”的组合。
- 现象：`Reload` 只有在请求位置与新位置的规范路径不同时才检查目标占用与文件身份；规范路径相同、文件身份已随原子替换或硬链接替换而变化时，两个检查都被跳过。随后 `refresh_identity` 先删除本 Document 拥有的身份项，再用 `identities.insert` 写入新身份，不检查该身份是否已属于另一聚合；`existing` 又优先按身份命中，于是另一个 Document 的文件身份会被本 Document “偷走”。
- 根因：身份变化只在跨路径时被当作冲突候选；身份索引写入是无条件覆盖，缺少“身份已有其他 owner”的 fail-closed 分支。
- 影响：同一文件身份可同时被两个实时聚合持有，违反“全应用同一文件身份最多一个聚合”；硬链接别名场景下两个 Document 可各自保存到不同路径并静默覆盖，且新打开会解析到错误 DocumentId。
- 证据：`Reload` 同路径分支未调用 `existing`／身份比较；`refresh_identity` 使用 `insert` 而非带 owner 校验的插入；`existing` 身份优先于路径。应用侧 `Document::load` 只拒绝读取期间身份变化，不拒绝请求快照之后发生的同路径替换。
- 优化方向：任何身份变化都先对新身份做全局 owner 检查，已有其他 owner 时返回目标占用／冲突；身份索引插入必须 fail-closed，不能覆盖既有归属。

### 重要｜可编辑 Document 缺行尾切换、编码重开与手动只读的活体变更通道

- 置信度：确认
- 位置：`slterm_document/src/registry.rs` 的 `DocumentMetadata`／`DocumentSession`、`slterm_document/src/edit.rs` 的 `EditOrigin::LineEndingChange`、`slterm_document/src/save.rs` 的 `begin_save`；应用侧 `slterm_app/src/app_shell/documents.rs` 的 `DocumentOpenService::load` 与 `slterm_app/src/app_shell/file_editor/document.rs` 的 `from_save_snapshot`；追溯锚 `editor_encoding_and_binary_routing`。
- 现象：编码、BOM、行尾与只读元数据只在 `open` 与外部 `Reload` 时被整体写入，没有“对存活可编辑 Document 修改行尾／重新按编码解释／切换手动只读”的入口；`LineEndingChange` 仅声明，无任何生产者；保存请求虽然捕获 `session.metadata.encoding`，应用保存链却从磁盘基线重新解码并重发现 BOM／行尾，不使用请求中的行尾与 BOM 事实。真实打开链对 GBK／GB18030 传入 `None`，严格 UTF-8 解码失败后拒绝建立 Document，也没有“编码重开”命令。
- 根因：元数据被建模为打开／重载时的一次性事实，而非可经 Document 修改与撤销的状态；规格要求的编码重开、行尾切换和手动只读没有对应的核心状态转换。
- 影响：Editor 规格中的“GBK 等本地编码以编码重开进入编辑”“行尾切换写入共享 Document”“手动只读仅在可编辑且干净时开启”无法成立，GBK 文件在真实产品链中不可打开；即使未来接入行尾切换，保存仍可能使用旧磁盘行尾。追溯锚 `editor_encoding_and_binary_routing` 只直接调用核心 `DocumentContent::decode(..., Some(Gbk))` 并断言非法／二进制打开报错，没有经过真实打开或重开链，也没有断言失败原因，不能证明该验收场景。
- 证据：`DocumentMetadata` 没有公开修改路径，`DocumentSession::metadata` 仅 crate 内可见；`LineEndingChange` 在全仓无生产者；应用没有查找／替换之外的编码选择或手动只读命令；`from_save_snapshot` 以基线解码结果构造编码，忽略 `DocumentSaveRequest::encoding()` 的行尾／BOM 字段；应用测试的 GBK 断言发生在核心解码，而非 `DocumentOpenService` 结果。
- 优化方向：定义唯一的 Document 修改路径来切换编码解释、行尾与手动只读，明确脏确认、撤销组、保存编码来源与失败回滚；把编码重开与行尾选择接入真实打开／保存链；修正追溯锚为端到端断言。

### 重要｜查找与替换只实现字面量查找和全部替换，且没有生产接线

- 置信度：确认
- 位置：`slterm_document/src/search.rs` 的 `DocumentTextRequest::search`、`/copy`、`/copy_range`，`slterm_document/src/replace.rs` 的 `DocumentReplaceRequest`／`DocumentReplacement::into_transaction`；应用侧 `slterm_app/src/app_shell/text_view/editor.rs`、`slterm_app/src/app_shell/markdown/view/search.rs`；追溯锚 `editor_replace_all_single_undo`。
- 现象：核心查找只做大小写敏感的字面量 `match_indices`，没有大小写、整词或正则查找选项，也没有“替换当前匹配”的计划；替换只有“正则全量匹配后一次事务”这一种形态。应用 Rope Editor 没有查找／替换 UI，Markdown 搜索使用 `MarkdownDocument::find_literal` 而不是核心搜索；核心 `replace_request` 与 `DocumentTextRequest::search` 在应用中没有生产调用方，追溯锚 `editor_replace_all_single_undo` 指向核心单测，产品中并不存在“确认后替换全部”的入口。
- 根因：核心把后台查找契约限定为字面量、把替换限定为全部替换；视图侧未接入搜索栏与替换命令。
- 影响：Editor 规格中的 `Ctrl+F` 选项、增量匹配、当前匹配导航、替换当前／全部、回绕与非法正则行内错误均未形成产品行为；核心已实现的取消通道、代次与正则捕获能力没有生产消费者；追溯矩阵把只存在于核心单测的行为标为已自动化验收。
- 证据：`search` 直接对 `pending` 调用 `match_indices`；替换计划只由 `regex.captures_iter` 全量产生；应用测试与源码中没有 Find／Replace action 的注册；Markdown 搜索走 `find_literal`；核心搜索测试只覆盖切块与取消。
- 优化方向：把匹配选项纳入唯一的后台查找契约或在核心暴露可组合的匹配器，补齐替换当前与查找导航的单一执行路径；接入 Editor 搜索栏并让追溯锚覆盖真实命令链。

### 一般｜Relocated 无条件清除既有外部冲突

- 置信度：确认
- 位置：`slterm_document/src/external.rs` 的 `DocumentExternalChange::Relocated` 处理分支；测试 `stale_external_result_cannot_touch_relocated_document`、`read_revoke_and_relocation_preserve_document_identity` 未构造“先冲突后改名”的前置状态。
- 现象：位置重绑分支在复核目标占用与身份后直接设置 `external_conflict = None`。若此前的磁盘外部修改已形成 Modified 冲突且用户尚未解决，一次纯改名事件会把冲突事实与独立磁盘快照一并清除，而本地正文仍是脏的。
- 根因：把位置重绑当成了冲突解决；清除冲突与更新位置被绑定在同一状态转换中。
- 影响：共享冲突在改名后静默消失，视图失去冲突横幅与磁盘快照；虽然保存前的基线复核仍能阻止盲写，但 Document 规格要求冲突由显式重载／保留／查看差异解决且对所有引用一致，该状态不一致会误导用户。
- 证据：`Relocated` 分支对 `external_conflict` 的赋值；现有重定位测试只覆盖无冲突状态。
- 优化方向：纯改名保留既有冲突（并按需更新其位置展示），只有显式重载或用户决定才清除冲突。

### 一般｜撤销／重做绕过只读判定

- 置信度：确认
- 位置：`slterm_document/src/edit.rs` 的 `DocumentSession::travel`、`DocumentRegistry::undo`／`redo`；测试 `read_only_and_exhausted_revisions_reject_changes`。
- 现象：`apply` 经 `validate_transaction` 拒绝只读 Document，但 `travel` 只检查是否存在 Rope 与 revision 是否耗尽，不检查 `is_read_only()`。一个曾编辑并保存、随后因外部重载获得只读元数据（例如磁盘权限变为只读）的 Editable Document，撤销／重做仍会改写正文、推进 revision 并产生只读且脏的状态。
- 根因：只读门禁放在编辑事务校验内，没有覆盖历史移动这条文本变更路径。
- 影响：核心层违反“只读状态不接受编辑”；正文被改写且无法原地保存。当前 Editor 的 `travel_history` 自身按视图只读状态拒绝，因此用户界面暂时挡住了该路径，但核心单一权威不成立，任何绕过视图或未来 Runtime 入口都会触发。
- 证据：`travel` 的实现无 `is_read_only`；只读测试只断言 `apply` 与保存入口；应用 `travel_history` 有独立只读守卫。
- 优化方向：在核心 travel 入口执行同一只读判定，或在只读状态建立时清理不可用历史。

### 一般｜相邻输入合并会把覆盖式替换并入同一撤销组

- 置信度：确认
- 位置：`slterm_document/src/edit.rs` 的 `EditHistory::should_join`；测试 `adjacent_typing_merges_only_inside_the_time_window`。
- 现象：合并条件只要求事务为单编辑、`UserInput`、`MergePolicy::Adjacent`，且新编辑的原文区间与上一组的 `after` 区间相交；新编辑的起点与上一组起点的关系没有限定为“在上一组产生的尾部继续输入”。用户在合并窗口内选中刚键入的内容并用一次替换覆盖它，会被并入同一撤销组，一次撤销跳过整次替换。
- 根因：用区间相交近似“相邻输入”，没有区分插入／删除继续输入与覆盖式替换。
- 影响：撤销粒度与“普通相邻输入按 600 毫秒合并”的声明不一致；用户可能丢失一次独立的编辑步骤。现有测试只覆盖字符插入，未覆盖选中替换、剪贴板式大段替换与删除后输入。
- 证据：`should_join` 的区间相交条件；`adjacent_typing_merges_only_inside_the_time_window` 仅构造插入。
- 优化方向：按编辑形状与光标／文本尾部关系收紧合并条件，并把覆盖式替换与删除后的独立输入排除在合并之外。

### 一般｜外部重载撤销组物化全文，并可被预算立即淘汰

- 置信度：确认
- 位置：`slterm_document/src/edit.rs` 的 `EditHistory::record_reload`、`EditHistory::push`、`MAX_HISTORY_BYTES`；测试 `external_reload_is_an_independent_undo_group`。
- 现象：干净外部重载以单个 `Change { start: 0, before, after }` 记录整篇前后文本；`before`／`after` 都由 `Rope::to_string()` 物化。较大但仍在可编辑阈值内的文档，其撤销组本身即可超过固定预算，进入 `push` 后会被“淘汰最旧组”的逻辑立即删除，或清空既有历史；转换在调用 `apply_external` 的线程上完成，应用若在前台回投即形成界面停顿。
- 根因：差量撤销历史对重载退化为全文快照，而预算与淘汰策略按差量大小设计；没有区分重载的存储形态或采用分段差异。
- 影响：大文档的干净外部重载不具备声明的独立撤销组，且产生前台内存与 CPU 峰值；自动重载越频繁，历史越容易整体丢失。
- 证据：`record_reload` 的全文 `to_string` 与单 `Change`；`push` 的预算淘汰；测试只使用短文本。
- 优化方向：以差分或可追加的分段形态记录重载，或将重载组设为独立预算外的受控条目；保证淘汰不会静默吞掉刚产生的组，并明确后台化全文转换。

### 一般｜Markdown 源范围按块从文档头累加，投影呈二次复杂度

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs` 的 `source_position`、`source_range` 与 `Projection::block`。
- 现象：每个块的起止位置都通过 `split_inclusive('\n').take(line - 1).map(str::len).sum()` 从源文本开头重新累加；块数量与源行数同时增大时，整体投影成本按块数与源字节数的乘积增长。应用在渲染阈值附近的大文档上仍会执行该投影。
- 根因：没有预先构建行首偏移表或单调游标，逐节点重复扫描源码。
- 影响：接近渲染阈值、段落／列表项密集的 Markdown 会在解析阶段消耗大量 CPU，放大前台或后台解析停顿；现有测试只使用极短源码，无法暴露规模效应。
- 证据：`source_position` 的整源扫描；`source_range` 每个块调用两次；`Projection::block` 对每个顶层块调用 `source_range`。
- 优化方向：解析前一次性建立行首索引，按行号直接映射字节偏移；补充大规模块语料测试。

### 一般｜Markdown 可见文本查找按块线性扫描，逐键击查找可冻结

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs` 的 `MarkdownDocument::find_literal`；应用侧 `slterm_app/src/app_shell/markdown/view/search.rs` 的 `refresh_search`。
- 现象：每个字面匹配都通过 `blocks.iter().find` 线性扫描全部块来定位所属块；匹配数与块数相乘。应用在搜索输入每次 `InputEvent::Change` 时于前台调用该方法。跨块匹配只按起点归块，返回的全局范围可能越过该块的 `visible_range`，而块与块之间还插入了不属于任何块的可见文本分隔换行，起点落在分隔处的匹配会被直接丢弃。
- 根因：没有利用块 `visible_range` 的有序性做二分或区间索引；跨块语义未定义。
- 影响：常见单字符查询在大型渲染文档上会产生极大扫描量并冻结界面；跨块／分隔处匹配的结果归属与范围不自洽，消费方若按块切片高亮会越界或错配。
- 证据：`find_literal` 的 `blocks.iter().find`；应用 `refresh_search` 的同步调用；测试只覆盖单块短文本与不存在查询。
- 优化方向：按有序块范围建索引查找；明确并测试跨块匹配策略（丢弃、拆分或归并），避免返回超出块范围而不说明。

### 一般｜`DocumentContent::decode` 对有限前缀做严格整段解码，大文件非 UTF-8 外部刷新可能失败

- 置信度：疑似
- 位置：`slterm_document/src/encoding.rs` 的 `DocumentContent::decode`、`DocumentEncoding::decode_prefix`；应用侧 `slterm_app/src/app_shell/file_editor/document.rs` 的 `Document::load` 与 `slterm_app/src/document_io.rs` 的 `read_prefix`。
- 现象：`decode` 明确按传入的 `bytes` 整段严格解码；`Paged` 分支仍把该缓冲解码后的 `String` 与原始前缀字节写入 `DecodedText`。应用对超过可编辑阈值的文件只读有限前缀再调用 `decode(bytes, total_bytes, None)`，而 `read_prefix` 的“回退到完整字符边界”只针对 UTF-8；UTF-16 的后半个代理对、GBK／GB18030 的多字节字符在 2 MiB 边界被截断时，`decode` 会返回 `InvalidEncoding`，外部刷新在到达真实分页读取前即失败。
- 根因：路由 API 同时承担“尺寸分流”和“完整文本严格解码”，没有截断语义，也没有复用专为前缀设计的 `decode_prefix`；调用方以有限前缀调用时，前缀事实与完整基线事实混在同一返回类型中。
- 影响：超过阈值且以 UTF-16 等编码保存的文件，在外部刷新边界命中半个字符时可能持续刷新失败，正文停留在旧快照；Paged 返回值的文本与前缀基线也容易被误当作完整事实。
- 证据：`decode` 的严格完整解码与 `disk_baseline: Arc::from(bytes)`；`read_prefix` 的 UTF-8 专用回退；应用 `Document::load` 对大文件使用 `PAGED_PREFIX_BYTES` 前缀。缺少真实大文件与 watcher 路径的证据，故置信度为疑似。
- 优化方向：把“完整解码”和“前缀路由”拆成明确 API，前缀路由使用 `decode_prefix` 并显式携带截断事实；补充真实大文件 UTF-16／GB18030 边界用例。

### 一般｜SyntaxWorker 的“过期命令仍推进旧树”测试没有覆盖旧树

- 置信度：确认
- 位置：`slterm_document/src/syntax_tests.rs` 的 `superseded_edits_still_advance_the_old_tree`。
- 现象：测试在初始解析的第一个 `Progress` 处阻塞，随后提交两次编辑再放行。初始解析因代次被超越而中止，`tree` 从未建立；后续过期命令进入 `else if let Some(old_tree)` 时没有旧树可推进，最终由最新命令全量重建。断言只比较最终树与期望树，即使删除“把过期编辑应用到旧树”的实现，该测试仍会通过。
- 根因：前置状态构造错误；没有先完成一次可见解析并采纳树，再制造过期编辑。
- 影响：CLAUDE 明确要求的“过期命令仍须推进正文与旧树”缺少有效回归证据，后续优化可能在无测试保护下破坏旧树同步。
- 证据：观察者在初始解析中阻塞；初始解析中止后 `tree` 为空；最终断言不区分全量重建与增量推进。
- 优化方向：先完成并采纳初始树，再阻塞于下一次解析，提交多编辑事务并让新代次覆盖，最后断言旧树在该命令后仍被推进或新命令仍能增量解析。

### 一般｜外部变化与状态身份的关键失败路径无测试

- 置信度：确认
- 位置：`slterm_document/src/external.rs` 的 `ExternalChangeError` 与 `DocumentExternalChange` 分支、`slterm_document/src/recovery.rs` 的 `ReauthorizationError`、`slterm_document/src/edit.rs` 的状态复用测试、`slterm_document/src/search.rs` 的 `SearchError`、`slterm_document/src/syntax.rs` 的队列／关闭路径。
- 现象：`ExternalChangeError::TargetOccupied`、`IdentityMismatch`、`StateConflict`、`RevisionExhausted`、`SourceGenerationExhausted` 在核心测试中没有触发用例；`ReauthorizationError::LocationConflict` 与 `DocumentNotFound` 没有用例；`SearchError::SourceReaderRequired`、`ReadFailed`、`ChunkOffsetMismatch` 没有用例；`SyntaxWorker` 的 `flush`／`set_visible` 队列满、结果队列满时 `shutdown` 排空后 join 没有用例。`new_edit_clears_redo_and_new_state_ids_cannot_be_reused` 只验证 redo 清空后的复用，`history_budget_evicts_oldest_undo_groups` 未尝试复用被淘汰组的状态。
- 根因：测试集中在已覆盖的成功路径和少数手工选择的错误；CLAUDE 与测试策略要求的失败路径、边界与状态淘汰语义没有逐项落测。
- 影响：本报告确认的同路径身份覆写、重定位清冲突、只读撤销等缺陷正位于这些无测试分支；后续修改无法被门禁发现。
- 证据：对上述错误变体、队列满与关闭排空路径的符号检索只在实现中出现，没有对应测试名或断言。
- 优化方向：为每个公开错误变体与每种代次／队列耗尽、取消、关闭路径补最小触发用例；为历史淘汰后的 TextStateId 复用补断言。

### 一般｜设计要求的属性测试覆盖缺失

- 置信度：确认
- 位置：`slterm_document/src/edit.rs` 的 `random_unicode_edits_round_trip_through_undo_and_redo`、`slterm_document/src/diff_tests.rs` 的属性测试；`docs/devPlan/design/document.md` 的验收与测试段。
- 现象：设计验收要求 Rope 属性测试覆盖随机编辑、撤销／重做、合并预算、Unicode 边界、编码往返和 TextStateId 清脏；现有编辑属性测试固定使用 `ReplaceAll`／`Disallowed`，因此不触及合并策略与预算淘汰；编码往返只有单元测试，没有生成式输入。
- 根因：验收条目被拆成单元测试，属性测试只覆盖了其中一部分。
- 影响：合并窗口、预算淘汰与任意编码序列的组合边界没有生成式证据；这类边界恰是历史状态与保存字节最容易漂移的区域。
- 证据：属性测试的构造器固定 origin 与 merge policy；编码测试为固定样例。
- 优化方向：增加生成式事务序列，随机 origin／merge policy／时间间隔与历史预算，并生成多编码文本做往返断言。

### 提示｜追溯锚 `editor_large_file_dirty_preserves_buffer` 重名且分工覆盖不完整

- 置信度：确认
- 位置：`slterm_app/src/app_shell/document_external.rs` 与 `slterm_document/src/edit.rs` 中同名的 `editor_large_file_dirty_preserves_buffer`；`docs/devPlan/traceability.md` 的 Editor 行。
- 现象：追溯矩阵引用唯一例名，但仓库中存在两个同名测试。应用侧只构造干净文件真实增长并切 Paged；脏状态保留正文由核心侧用人工拼装的 `PageIndex`／`PagedLineIndex` 与直接构造的 `Reload` 验证，没有端到端的真实增长、脏缓冲、保存出口链路。
- 根因：同一验收场景被拆成两个不同层次的同名用例，但没有区分名称，也没有把脏路径接到真实读取。
- 影响：追溯锚无法无歧义定位；“文件跨过阈值时脏保留缓冲”的真实链路没有自动化证据，真实页边界或外部服务接线变化可能不被发现。
- 证据：两个同名测试的构造方式；应用测试只断言干净切换与二次刷新无变化；核心测试使用手工 Paged 体。
- 优化方向：为两层用例取区分性名称，并把脏增长场景放到真实读取与外部刷新链上验证。

### 提示｜编辑层不强制内存文本 LF，末尾换行事实无核心消费者

- 置信度：确认
- 位置：`slterm_document/src/edit.rs` 的 `TextEdit::new`／`TextEdit::background`／`DocumentSession::apply`，`slterm_document/src/encoding.rs` 的 `TextEncoding::encode` 与 `trailing_newline`。
- 现象：`TextEdit` 接受任意替换文本，应用层没有校验或归一化 `\r\n`；`TextEncoding::encode` 对 CRLF 模式直接执行 `text.replace('\n', "\r\n")`，若内存中已有 CRLF，会生成 `\r\r\n`。`trailing_newline` 只在解码时记录并被快照信封持久化，保存编码不读取它。应用 Editor 的粘贴路径会归一化 CRLF，但正则替换计划、未来的替换 UI 或非 Editor 调用方可直接构造带 CRLF 的事务。
- 根因：“内存统一 LF”与“末尾换行独立保留”被声明为不变量，却没有在唯一编辑入口或编码回写处强制。
- 影响：一旦有非归一路径写入 CRLF，保存会产生损坏的行尾；末尾换行事实没有行为消费者，无法验证“保持原文件策略”是否被执行。
- 证据：`apply` 只校验 UTF-8 边界与原文；`encode` 的条件替换；`trailing_newline` 在核心除测试外无读取点；应用 Editor 的 `replace` 对 CRLF 做归一化，但该归一化不在核心契约内。
- 优化方向：在事务校验或应用时统一拒绝／归一化 CRLF，并把末尾换行事实纳入保存策略或明确其仅为展示事实。

### 提示｜内嵌测试模块命名未遵循测试策略

- 置信度：确认
- 位置：`slterm_document/src/edit.rs`、`encoding.rs`、`paged.rs`、`recovery.rs`、`replace.rs`、`search.rs`、`syntax.rs`、`language.rs`、`markdown.rs`、`markdown_svg.rs` 的内嵌测试模块。
- 现象：这些文件使用 `mod tests`，而 `.claude/docs/testing/strategy.md` 约定“内嵌测试模块 = `mod <领域>_tests`，一文件多测试模块用功能具名”。同 crate 内 `identity_tests`、`registry_tests`、`repository_tests` 已按约定命名。
- 根因：测试模块命名未随策略统一。
- 影响：定向过滤与测试归属的命名约定不一致，降低追溯与筛选的一致性；不影响运行语义。
- 证据：模块声明与策略原文。
- 优化方向：按领域重命名测试模块，保持定向过滤与追溯命名一致。

## 跨域观察

- `SyntaxWorker` 的隐藏与关闭能力没有生产接线：`slterm_app/src/app_shell/app_root/documents.rs` 的 `document_syntax` 按布局引用缓存 `DocumentSyntax`，隐藏项目与隐藏面板仍保留 worker；应用中没有 `SyntaxWorker::set_visible` 调用，也没有 `shutdown` 调用，最后引用移除时仅依赖 `SyntaxWorker` 的 `Drop`（不 join、不排空结果）。这与 Document 设计中“隐藏释放旧树、重新可见全量重建”和线程显式 shutdown 纪律不符；核心能力已实现并有测试，但产品链路未消费。
- 分页选区复制从源文件起点顺序读取：`slterm_app/src/app_shell/paged_document.rs` 的 `PagedReaderChunks` 从页起点开始，而 `DocumentTextRequest::copy_range` 要求块偏移从零连续验证，因此复制靠近文件末尾的选区仍会读取并解码全部前序页。核心可考虑支持经页索引校验的起始偏移，避免大文件复制代价随位置增长。
- 应用保存链按磁盘基线重新解码 BOM／行尾，而不是直接使用 `DocumentSaveRequest::encoding()` 的完整 `TextEncoding`；当前因行尾切换入口缺失而未暴露，但与上述“活体元数据变更”缺陷叠加后会立即产生保存行尾／BOM 不一致。

## 存疑未证

- `markdown_svg` 允许任意属性名（仅拒绝事件名前缀、`style`、`filter`、`target` 与不安全 URL）；`xml:base` 之类改变相对引用基准的属性未被拒绝，理论上可能把 `url(#fragment)` 或 `href="#fragment"` 解析到外部文档。需要 `usvg`／`resvg` 的真实光栅化用例确认其是否实际跟随外部基准；未证前不排除外部引用规则仍可被绕过。
- SVG 属性注入已在清洗源码层确认；注入后的 `<filter>`、超大 `stdDeviation` 等是否在当前 `usvg` 选项下形成可行的像素／CPU 预算绕过，仍需真实渲染证据。事件属性即使被 `usvg` 忽略，白名单承诺仍已失效。
- 大文件外部刷新在 2 MiB 前缀边界截断 UTF-16 代理对或 GB18030 多字节字符的失败，需要真实大文件与 watcher 刷新路径验证；源码路径已存在，但尚未取得运行时证据。
- 同路径硬链接替换触发身份索引覆写需要应用 watcher 与外部分类路径的真实组合验证；核心索引覆写行为已由源码确认，实际触发频率取决于替换方式与 watcher 事件分类。
