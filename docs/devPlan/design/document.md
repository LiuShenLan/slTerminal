# Document、编辑与文本视图

> 规定唯一 DocumentSession 的文本、编码、撤销、保存、分页、语法和 GPUI 呈现，禁止视图自持第二份实时文本。

## 决策与红线

### 内容与加载

`DocumentContent` 封闭为：

```text
Editable   { rope, encoding, bom, eol, disk_baseline, ... }
Paged      { source_identity, page_index, encoding, ... }
Unavailable { location, cause, retry_generation, ... }
```

- 打开流程先经 PathAuthority 与 PathResolver，再读取元数据和有限前缀；显式打开失败不留空 Panel。已有 Document 重读失败保留原内容和位置，记录 `reload_error`，不以空 Rope 替换。
- 前 8192 字节含 NUL 的文件按二进制处理，不建立文本编辑基线。无法判定编码时提供编码重开或系统打开，不以乱码建立可保存基线。
- 普通文件按真实字节数分流：不超过 10,000,000 字节正常编辑；10,000,001–50,000,000 字节可编辑但关闭文档内自动补全并提示；超过 50,000,000 字节进入 Paged。Paged 内容超过 200,000,000 字节关闭语法高亮，保留分页、查找和复制。
- 已打开文件外部增长时，干净 Document 可切换 Paged；脏 Document 保留可编辑文本与保存出口，不能因阈值销毁用户内容。只读分页不提供编辑、替换、Git 行标或原地保存。
- 内存文本统一 UTF-8。原始编码、BOM、首个 CRLF／LF、末尾换行事实和磁盘字节基线独立保存；新建文件默认 CRLF，末尾换行保持原文件策略。

### EditTransaction 与撤销

所有文本变化只经 DocumentService 的 `EditTransaction`：

```text
EditTransaction {
    document_id,
    expected_text_revision,
    edits: [ { utf8_range, replacement } ],
    origin,
    merge_policy,
}
```

- 应用前校验 DocumentId、revision、UTF-8 边界和权限；后台替换还校验原始区间文本，失配即丢弃。事务成功产生单调 `text_revision` 和稳定 TextStateId，并同步通知所有引用视图。
- Rope 差量保存逆操作、前后 TextStateId 和必要的磁盘基线关系；普通相邻输入按 600 毫秒合并，撤销总预算 8 MiB。替换全部、编码重读、行尾切换和外部重载不合并。
- 行尾切换是一次 Document 修改；保存时统一转换磁盘字节。撤销／重做按状态身份恢复文本，不恢复搜索、折叠、光标或焦点。
- 崩溃快照不恢复撤销树；异常恢复只恢复文本和必要元数据，正常重启按磁盘基线重建。

### 保存、另存为与外部变化

- 保存请求捕获 DocumentId、TextStateId、Rope 快照、磁盘基线、授权代次和保存代次，在后台完成编码、写临时文件和原子替换；前台仍可继续编辑。
- 写前重新解析真实目标并比较磁盘基线、文件身份和权限。外部版本变化提供覆盖、另存为或取消；覆盖前再次校验，自动保存不得自动选择覆盖。
- 编码无法表示当前文本时保存失败，不替换字符、不截断。目标 ACL 无法保留或原子替换不可用时同样失败。
- 保存成功只更新磁盘基线和 saved_state_id；如果保存期间又编辑，当前 TextStateId 不同，Document 仍为脏。撤销回 saved_state_id 才清脏。
- 另存为只写用户选定的新位置，原文件不变；成功后同一 Document 与全部视图重绑，DocumentId 不变。目标已关联其他打开 Document 时写前拒绝。
- 干净外部修改自动重载并保留各视图阅读锚点；脏外部修改进入共享冲突，重载、保留本地或查看差异对全部引用一致。外部删除保留文本，重新创建须显式确认。
- 自动保存为应用级开关：停止修改 1 秒、所有编辑视图失焦、最后视图关闭和退出前触发；外部冲突、失权、只读和错误状态暂停。保存失败保留脏状态并聚合提示。

### 分页、查找与替换

- Paged 以源字节范围读取，单页 2 MiB；页边界按编码和行边界调整，页索引只保存源范围与偏移，不复制完整正文。全应用解码页 LRU 上限 128 MiB。
- 查找、复制和大段替换在后台流式执行，携带 Document revision、请求代次和取消令牌；旧结果不得覆盖新文本。替换全部确认后作为一个撤销步骤，正则支持捕获组，非法正则只显示行内错误。
- 查找选项、当前匹配、折叠、搜索栏和滚动中的临时状态为瞬态。Editor 的语言覆盖、换行和滚动位置按 ViewKey 持久化；Markdown 源码态不另建语言覆盖或折叠规则。
- 编辑器补全只使用当前 Document 标识符与已注册来源，不依赖外部语言服务；超过大文件提示阈值后关闭文档内自动补全。

### LanguageRegistry 与语法

- LanguageRegistry 使用封闭 LanguageId 和静态 LanguageDescriptor，统一扩展名、固定文件名、Markdown 别名、grammar、highlight query 与折叠规则。JSONC 仅作 JSON5 高亮超集，不用于验证 JSONC。
- 每种规格语言必须有实际 grammar 和查询资产；缺失阻塞并报告，只有未注册类型使用纯文本。语言手动覆盖只改变高亮，不改扩展名、内容或保存行为。
- 每个可见 Document 由 SyntaxWorker 接收完整编辑对应的 `InputEdit`，只采纳当前 revision 的树；隐藏视图释放语法树，重新可见从最新 Rope 快照重建。
- Tree-sitter 0.27 使用 `ParseOptions::progress_callback` 在新代次或关闭时真正中止解析。解析任务有界，旧树和旧高亮不能回填新 revision。
- Markdown 语义只走 Comrak；Tree-sitter Markdown 只用于源码高亮，渲染代码块独立高亮，不建立第二套 Markdown injection 语义。

### GPUI 文本视图

- gpui-component 只保留按钮、菜单和普通单行输入等通用控件。Document 编辑器、Markdown、Diff 和只读分页查看器使用应用内专用 GPUI 视图。
- DocumentSession 是唯一可编辑文本所有者；编辑视图只持选区、光标、滚动、折叠和布局缓存。应用级 `TextSource` 分别适配 Rope 与分页只读内容，渲染只取可见行。
- 文本布局缓存键包含 Document、TextStateId、语言 revision、字体度量、换行宽度、DPI 和主题 revision；主题或字体变化只使布局失效，不重建 Document 或 PTY。
- IME 经 GPUI `InputHandler` 接入，明确 UTF-8 字节、Rope 字符与显示列转换；组合期间不派发命令。IME、CJK、组合字符和 emoji 的光标与选区按显示宽度处理。
- Markdown 使用虚拟化块视图，Diff 使用对齐行对视图，复用文本测量与选择能力，不引入 HTML、WebView 或第二实时缓冲。

## 验收与测试

- Rope 属性测试覆盖随机编辑、撤销／重做、合并预算、Unicode 边界、编码往返和 TextStateId 清脏。
- 保存并发测试在写前、写入、rename 和回前台之间插入编辑，验证基线更新与当前脏状态；外部覆盖、另存为和 ACL 失败均保留正文。
- 真实文件测试覆盖 UTF-8 BOM、UTF-16、GB18030／GBK、不可表示字符、NUL、行尾和外部增长阈值。
- 分页测试验证源页边界、LRU、后台查找取消、复制不截断和超过 200,000,000 字节后的高亮降级。
- SyntaxWorker 测试验证完整 InputEdit、revision 丢弃、隐藏释放和 progress_callback 中止；每种注册语言有实际样例。
- GPUI 虚拟窗口验证 Editor／Markdown／Diff 共享文本、IME 组合、选区、布局缓存失效和无第二 `String` 实时缓冲。

## 操作

- 新增编辑能力必须进入 EditTransaction 或明确只读 TextSource；不得在 Panel 内直接修改 Rope 或另存草稿。
- 新增语言先登记描述符、资产、测试样例和 Markdown 别名，再接入视图；缺失资产按阻塞报告。
- 修改保存或快照格式时同步磁盘基线、TextStateId、维护期对账和追溯测试。
