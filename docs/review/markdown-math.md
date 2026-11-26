# Markdown 投影、安全资源与公式审查

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`，只读审查当前工作树中的 Markdown 投影、渲染视图、资源安全链、数学引擎与相关文档/测试。

## 覆盖与局限

- 已读：`slterm_document/src/markdown.rs`、`slterm_document/src/markdown_html.rs`、`slterm_document/src/markdown_svg.rs`；`slterm_app/src/app_shell/markdown/` 全模块及 `view/` 子模块；`slterm_app/src/math/` 全模块；`slterm_app/src/scientific_corpus.rs`；`slterm_app/src/platform/document_paths.rs`、`slterm_path/src/policy.rs`、`slterm_app/src/app_shell/documents.rs`、`slterm_app/src/app_shell/app_root/markdown_links.rs` 及 Markdown 相关 AppRoot 测试。
- 已读文档：根 `CLAUDE.md`、`slterm_app/CLAUDE.md`、`slterm_app/src/app_shell/CLAUDE.md`、`slterm_app/src/app_shell/app_root/CLAUDE.md`、`slterm_app/src/app_shell/markdown/CLAUDE.md`、`slterm_app/src/math/CLAUDE.md`、`slterm_document/CLAUDE.md`、`slterm_app/src/app_shell/text_view/CLAUDE.md`；`docs/featureSpec/panels/markdown.md`；`docs/devPlan/design/markdown.md`；`docs/devPlan/plan.md`、`progress.md`、`traceability.md` 的 Markdown 行；`.claude/docs/testing/exemptions.md`。
- 已抽样：`assets/corpus/math-rendering-test.md`、`assets/corpus/scientific-rendering-acceptance-paper.md` 的结构、公式标记与危险内容分布；GPUI 依赖源码中的图片尺寸、列表滚动与文本布局语义。
- 局限：按合同未运行 cargo、测试、构建或网络；未在真实 DPI、真实 SMB 目标或真实桌面会话中观察；未读取真实用户数据目录。涉及运行期外观、网络副作用与性能的结论只到代码路径与依赖语义层。

## 问题

### 阻断｜本地图片路径在授权前被解析，UNC 路径可自动触发 SMB 认证

- 置信度：确认（代码路径与 Windows 路径语义闭环；网络副作用需真实共享目标才可观测）
- 位置：`slterm_app/src/app_shell/markdown/local_resource.rs` 的 `LocalMarkdownImageRequest::path_from_markdown`、`normalize_resource_path`、`MarkdownImageService::load`；`slterm_app/src/platform/document_paths.rs` 的 `SystemDocumentPathResolver::resolve`；`slterm_path/src/policy.rs` 的 `CanonicalPath::new`／`validate_absolute`；`slterm_app/src/app_shell/markdown/view/resources.rs` 的 `ensure_image`、`schedule_image`；`slterm_app/src/app_shell/markdown/view/render.rs` 的 `rich_text` 图片分支；`slterm_app/src/app_shell/markdown/link.rs` 的 `dispatch` 本地路径分支。
- 现象：打开含 `![x](//server/share/x.png)` 或 `![x](\\server\share\x.png)` 的 Markdown 文档，可见图片即被 `ensure_image` 自动调度；`MarkdownImageService::load` 先调用 `resolver.resolve` 做 `canonicalize` 并以 `FILE_FLAG_BACKUP_SEMANTICS` 打开目标，之后才执行 `PathPolicy::authorize`。`//server/share` 与 `\\server\share` 在 Windows 上被识别为 UNC 绝对路径，`CanonicalPath` 只拒绝 Verbatim/Device 前缀，不拒绝普通 UNC；因此打开文档这一浏览动作即可让进程访问任意 SMB 共享并完成当前用户的 NTLM 认证。链接点击路径同样先解析后授权，只是多一次点击门槛。
- 根因：资源授权被设计为“先解析出真实身份、再按真实路径授权”，但解析本身是文件系统（以及 UNC 网络）副作用；`path_from_markdown` 只按 URL scheme 分类远程地址，未把 UNC/设备命名空间归入不可自动解析的资源类别。`ensure_image` 对本地图片的自动加载放大了这一点：不需要用户点击。
- 影响：恶意或被污染的 Markdown 可直接造成未授权网络访问、凭据（NTLM）泄漏向量，并可能因共享不可达长时间阻塞共享图片服务锁；“文档可读或路径可解析均不等于资源可读”的红线在解析阶段被绕过。
- 证据：`MarkdownImageService::load` 的调用顺序是 resolver → `PathPolicy::authorize`；`SystemDocumentPathResolver::resolve` 内 `dunce::canonicalize` 与 `OpenOptions::open`；`slterm_path` 的 `containment_is_component_aware_and_unc_is_not_implicit` 表明 UNC 路径确实可被 `CanonicalPath` 接受，只是需要显式授权；`ensure_image` 对非 `http(s)`、非 `data:` 的 URL 直接调度后台 load；`rich_text` 渲染图片时不区分 UNC。
- 优化方向：先按词法与命名空间拒绝不可自动访问的路径形态（UNC、设备、重解析远端命名空间等），并让授权先于任何文件系统解析；解析端口对不可信资源路径只应在授权通过后可达，避免“解析即网络”。

### 重要｜行内公式与行内图片被移出文本流

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/render.rs` 的 `rich_text`；`slterm_app/src/app_shell/markdown/view.rs` 的 `schedule_parse`、`render_block` 公式分支。
- 现象：`rich_text` 把公式与图片从文本 run 中剔除，先把剩余文字拼成一段 `StyledText`，再把所有公式位图和图片作为 `v_flex` 子项追加在文本之后。`$x$` 行内公式因此不在原词位置，而是掉到段落末尾换行显示；前后文字直接拼在一起。图片同理。
- 根因：渲染模型只有文本 run 与块级图片两种呈现原语，没有与文本基线对齐的行内盒子；公式/图片只能走块级容器。
- 影响：`featureSpec/panels/markdown.md` 的“支持 `$...$` 行内公式”与语义不成立；中英混排、公式两侧空格与词序被改变，复制/选区映射也与屏幕内容不一致。
- 证据：`rich_text` 中公式 `continue` 后收集到 `formulas`，图片收集到 `images`，最终 `container` 先放 `content` 再顺序追加 `container.child(gpui::img(...))`；行内文字序列不包含公式源码或图片占位。
- 优化方向：为行内公式/图片提供行内盒子并与文本 run 同序排版，避免把行内元素提升为块级子项。

### 重要｜引用、列表与脚注内的块结构被压平，嵌套代码与 HTML 丢失

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs` 的 `Projection::block`、`project_inlines`、`project_inline_node`、`block_kind`、`block_structure`、`build_list`；`slterm_app/src/app_shell/markdown/view/render.rs` 的 Quote／List 渲染分支。
- 现象：投影只为根节点的直接子节点生成块。引用或列表项内部的多个段落被直接拼接（段间无分隔）；引用/列表里的围栏代码块与块级 HTML 不进入 `visible_text` 也不进入行内运行，内容完全消失；嵌套表格、标题、提示块被压成普通文本。列表项嵌套列表夹在段落之间时，`build_list` 把嵌套项移出当前项文本，而块级可见文本仍按 AST 顺序包含它，两边顺序不一致。
- 根因：块模型是单层的，`project_inline_node` 只处理行内 `NodeValue`，对 `CodeBlock`／`HtmlBlock`／`Table` 等块值没有分支，也没有递归块投影；`build_list` 另行拼装项文本，形成第二套顺序。
- 影响：常见 Markdown（引用里放代码、列表项多段说明）在渲染态静默缺失内容或把段落粘成一句；查找会在本应分段处产生错误匹配；引用/列表的语义与渲染结果不一致。
- 证据：`project_inline_node` 的匹配只覆盖 Text/Code/Math/LineBreak/Image/FootnoteReference/Link/Emph/Strong/Strikethrough，其余节点直接 `_ => {}` 后递归 children，代码与 HTML 节点无子节点；`block_structure` 只识别根级 `List`／`Table`；`build_list` 对 `NodeValue::List` 子项执行 `continue`，不并入当前项 `visible_offset`。
- 优化方向：让投影支持递归块或至少为引用/列表项保留分段与嵌套块边界；代码与 HTML 块必须作为块内容进入同一可见文本与渲染链，而不是被行内投影跳过。

### 重要｜脚注引用不渲染、不可点击

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs` 的 `project_inlines` 脚注引用分支与 `collect_links`；`slterm_app/src/app_shell/markdown/view/render.rs` 的 `rich_text` 脚注跳过与 `MarkdownBlockKind::Footnote` 分支。
- 现象：正文中的 `[^x]` 被投影为 `MarkdownInlineKind::FootnoteReference`，`visible_range` 为空，渲染时 `rich_text` 直接 `continue`；投影同时把它记成 `#fn-x` 链接与锚点，但屏幕没有任何标记或可点击对象。脚注定义块按普通段落显示在定义位置，没有编号或返回链接。
- 根因：脚注只完成了锚点/链接事实的投影，没有补上对应的呈现原语；渲染层把该 inline 当作不可见元数据。
- 影响：`featureSpec` 的“脚注”支持与“文档内链接、大纲和脚注共用锚点”只在数据层成立，用户无法读取或激活脚注；查找/选择也看不到引用标记。
- 证据：`rich_text` 对 `MarkdownInlineKind::FootnoteReference` 无条件跳过；`collect_links` 为 FootnoteReference 生成 `#fn-...`；`MarkdownBlockKind::Footnote` 落到通用段落渲染分支，没有 marker。
- 优化方向：为脚注引用渲染可见上标 marker 并绑定同一锚点链接；定义块应呈现为脚注区而非普通正文。

### 重要｜安全 HTML 白名单只做文本降级，`img`／`srcset`／`details` 等结构不可达

- 置信度：确认
- 位置：`slterm_document/src/markdown_html.rs` 的 `MarkdownHtmlNode`、`MarkdownHtmlElement`、`MarkdownHtmlAttribute`、`sanitize_attributes`；`slterm_app/src/app_shell/markdown/view/render.rs` 的 `MarkdownBlockKind::Html` 分支；`slterm_app/src/app_shell/markdown/view/render.rs` 的 `rich_text` 对 `MarkdownStyle` 的使用。
- 现象：HTML 块渲染只取 `document.visible_text()` 的扁平字符串。块级 `<a>` 不是可点击链接；`<img>`／`<picture>`／`<source>` 不产生占位、alt 或资源请求；`<details>`／`<summary>` 无结构；`<sub>`／`<sup>`／`<kbd>` 的样式在 HTML 块中丢失。行内 HTML 的 sub/sup 标志虽然被投影，`rich_text` 只消费 strong/emphasis/strikethrough/kbd/code，未消费 subscript/superscript。`srcset` 仅被记录为候选字符串，没有任何消费者，因此“每个候选 URL 独立判定”在现有产品路径上不可达；候选拆分按逗号进行，还会破坏含逗号的 `data:` URL 与带描述符的候选。
- 根因：安全投影已经建立节点树，但视图缺少节点树渲染；HTML 块被当作纯文本处理，行内样式也未覆盖 sub/sup。
- 影响：`featureSpec` 的安全 HTML 子集与实际渲染不一致；安全内容不显示、链接不可用、srcset 授权契约无法验证。`MarkdownHtmlFragment::is_degraded` 亦无视图消费者，超限降级对用户不可见。
- 证据：`MarkdownHtmlElement`／`MarkdownHtmlNode` 除 `markdown_html.rs` 与 `slterm_document/lib.rs` 导出外无任何消费点；`render.rs` 的 Html 分支只有 `document.visible_text()[block.visible_range()]`；`rich_text` 未读取 `MarkdownStyle::subscript`／`superscript`；全仓检索 `srcset` 只有投影与 `safe_url`，没有资源加载或测试。
- 优化方向：为净化后的节点树提供只读安全渲染（文本 run 与受限块结构），把图片与 srcset 候选接入逐 URL 授权；若本轮不渲染这些节点，应在设计/追溯中明确收窄安全子集表述。

### 重要｜程序化滚动与恢复位置不走视口调度，目标块只显示占位

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view.rs` 的 `schedule_parse`、`schedule_blocks`、`scroll_to_block`、`scroll_to_anchor`、`scroll_to_fragment`；`slterm_app/src/app_shell/markdown/view/render.rs` 的 `render_document` 占位分支与 `render_outline`。
- 现象：解析完成后只对固定的文档起始块调度渲染任务；列表的滚动回调只在用户滚轮滚动时触发。`ListState::scroll_to` 不调用滚动回调。因此大纲点击、查找导航、跨文件 `#fragment` 与重启恢复阅读位置落到这些起始块之外时，视图虽然滚到了目标索引，但该块没有 outcome，显示 `min_h(24px)` 的空占位，直到用户手动滚动一次才出现内容。
- 根因：调度入口绑定“上一次滚动事件”的可见范围，而不是“当前实际视口”；初始调度范围是硬编码常量，不读取恢复后的滚动位置。
- 影响：`featureSpec` 的大纲激活、查找导航、跨文件锚点与关闭重开/重启恢复在长文档上表现为目标处空白；这是核心阅读导航路径，不只是性能问题。
- 证据：`schedule_parse` 以 `document.blocks().len().min(32)` 调用 `schedule_blocks(0..initial)`；`scroll_to_block` 只调用 `ListState::scroll_to`；`scroll_to` 的实现只写状态，不触发 `set_scroll_handler`；`render_document` 对 `renderer.outcome` 为 `None` 的块只画占位。
- 优化方向：以布局后的真实可见范围驱动调度，程序化滚动、恢复与大纲/查找跳转都要在同一次状态转换中进入调度。

### 重要｜每次滚动清空全部已渲染 outcome，可见块回退占位并重排

- 置信度：确认（状态清空与异步回填路径确定；实际闪烁程度取决于帧时序）
- 位置：`slterm_app/src/app_shell/markdown/progressive.rs` 的 `ProgressiveMarkdownRenderer::schedule`；`slterm_app/src/app_shell/markdown/view.rs` 的滚动回调与 `schedule_blocks`；`slterm_app/src/app_shell/markdown/view/render.rs` 的占位分支。
- 现象：滚动回调每次调用 `schedule_blocks`，`ProgressiveMarkdownRenderer::schedule` 无条件清空 `outcomes`／`pending`／`inflight` 并推进代次；仍在视口内的块也丢失已渲染结果，须等后台批次回填。占位高度固定为 `24px * zoom`，与真实块高度不同，列表会在占位与真实内容之间反复测量。
- 根因：渲染结果的生存期与“最近一次滚动事件”绑定，而不是与“当前文档 revision 的块”绑定；没有对同一 revision 已渲染块做保留。
- 影响：滚动时可见内容短暂变为空白/矮占位，列表高度跳变，长文档滚动观感不稳定；也造成重复的后台派发和渲染代次增长。
- 证据：`schedule` 开头的清空逻辑；滚动回调直接调用 `schedule_blocks`；`schedule_blocks` 在调度前仅 prune 资源，不保留 outcome；`spawn_render_batch` 在异步完成后才回填。
- 优化方向：按 revision 缓存已批准块结果，只增量调度新进入视口的块；占位尺寸应尽量匹配真实块或保留最后一次渲染结果。

### 重要｜高 DPI 下公式位图被缩放因子放大后再按 1:1 逻辑像素显示

- 置信度：确认（GPUI 依赖源码已核对 `RenderImage::new` 的 scale factor 与 `img` 的固有尺寸语义）
- 位置：`slterm_app/src/app_shell/markdown/view/resources.rs` 的 `ensure_formula`、`formula_key`；`slterm_app/src/app_shell/markdown/formula.rs` 的 `FormulaBitmapKey::new`；`slterm_app/src/math/bitmap.rs` 的 `geometry`、`compose`；`slterm_app/src/app_shell/markdown/view/render.rs` 的公式 `gpui::img` 调用。
- 现象：位图尺寸按 `layout 逻辑尺寸 × scale_factor × zoom` 生成；`RenderImage::new` 的 scale factor 固定为 1.0，`gpui::img` 以位图像素数作为逻辑尺寸。文本按 `名义字号 × zoom` 绘制，于是 125%/150%/200% DPI 下公式相对正文分别放大约 1.25/1.5/2 倍。
- 根因：把“为高分屏生成更清晰位图”和“位图的逻辑尺寸”混在同一个缩放因子中；渲染层没有再用 `scale_factor` 校正显示尺寸，也没有传入显式宽高。
- 影响：高 DPI 是 Windows 常态，公式与正文基线/字号不一致，可能裁切或挤压行距；这不是纯像素观感豁免能覆盖的几何错误。
- 证据：`compose` 的 `raster_scale` 包含 `scale_factor`；`RenderImage::new` 固定 `scale_factor: 1.0`；`Img` 使用 `data.render_size` 作为固有尺寸，而 `render_size` 仅除以该固定 1.0；`render.rs` 未给公式图片设置显式逻辑尺寸。
- 优化方向：明确区分设备像素位图与逻辑显示尺寸，按窗口 scale factor 设置图片的逻辑尺寸或等价缩放，保证公式与文本同字号。

### 重要｜已加载的远程图片再次点击仍会发起新的网络请求

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/render.rs` 的图片元素 `on_click`；`slterm_app/src/app_shell/markdown/view/resources.rs` 的 `load_image_on_click`、`ensure_image`；`slterm_app/src/app_shell/markdown/remote_resource.rs` 的 `fetch_remote_on_click`。
- 现象：图片外层点击容器在占位与已显示图片两种状态下都安装同一 `load_image_on_click`。远程 URL 每次点击都把授权代次加一，新代次在服务缓存中必然 miss，随后发起新的 HTTP 请求。
- 根因：点击处理没有区分“失败重试”和“已成功显示”；授权代次按点击递增而不是只在新授权/失败重试时推进。
- 影响：违反 `featureSpec` 的“用户点击后仅请求该图片一次”和“失败保留重试”语义；重复点击造成重复网络访问、缓存条目增长与授权代次漂移。
- 证据：`rich_text` 的图片 `div().on_click(...)` 包裹 `if let Some(image)` 与占位两种分支；`load_image_on_click` 对 remote URL 无条件 `generation + 1`；`fetch_remote_on_click` 先 `peek_remote` 新代次，miss 后 `self.remote.fetch(url)`。
- 优化方向：成功加载后的图片不再安装网络点击行为；只有失败、未加载或显式“重新加载”入口才递增代次并重试。

### 重要｜渲染查找在前台按匹配数乘块数扫描

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs` 的 `MarkdownDocument::find_literal`；`slterm_app/src/app_shell/markdown/view/search.rs` 的 `refresh_search`。
- 现象：`find_literal` 对每个 `match_indices` 结果线性遍历全部块寻找可见范围。长文档中单字符查询可产生大量匹配，复杂度随匹配数与块数相乘；`refresh_search` 直接由输入变更事件在前台调用，并分配全部匹配结果。
- 根因：查找没有块区间索引或单次线性归并，块定位按匹配逐个进行。
- 影响：在明确支持的大文档区间（数 MB、十万行）输入查找词会造成前台卡顿，违反前台有界与“渲染查找只作用于可见文本”的可用性预期。
- 证据：`find_literal` 内 `self.blocks.iter().find(...)` 位于 `filter_map` 中；`refresh_search` 在 `InputEvent::Change` 订阅里同步执行。
- 优化方向：预建块可见区间索引并按长度二分，或按块顺序一次归并匹配；大文档查找应离开前台。

### 重要｜每次滚动在前台复制整篇文档文本用于块切片

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view.rs` 的 `schedule_blocks`。
- 现象：滚动回调每次调用 `self.renderer.schedule(self.revision, &self.text.to_string(), ...)`，即使调度只使用可见块及其源范围，也会在 GPUI 前台把整篇 Rope 转成 `String`。
- 根因：调度接口接收 `&str` 而不是按块/区间借用，调用方用整篇复制来满足类型。
- 影响：5 MB 乃至“仍要渲染”允许的更大文档在滚动时产生持续的整篇分配与拷贝，造成可感知卡顿；这与“前台绘制有界”“视口只生成有限块任务”的约束相悖。
- 证据：`schedule_blocks` 中的 `self.text.to_string()`；`ProgressiveMarkdownRenderer::schedule` 随后仅用 `source.get(block.source_range())`。
- 优化方向：让调度接受 Rope/按块源切片视图，或预先缓存块文本，避免每次滚动复制全文。

### 重要｜大纲无层级且常驻挤占内容区

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/render.rs` 的 `render_outline`；`slterm_document/src/markdown.rs` 的 `MarkdownHeading::depth`；`docs/featureSpec/panels/markdown.md` 的大纲章节。
- 现象：大纲把所有标题渲染成同一级别的按钮列表，未使用 `MarkdownHeading::depth` 做缩进或层级标识；大纲开启时以固定宽度侧栏常驻在内容行内，而不是浮层。
- 根因：大纲只消费标题文本与块索引，没有消费深度；布局把大纲作为内容行的兄弟列。
- 影响：`featureSpec` 的“大纲按标题层级显示”和“大纲浮层不常驻挤占内容区”均未满足；长文档阅读区被永久压缩，层级信息丢失。
- 证据：`render_outline` 的循环只创建 `Button::new(("markdown-heading", index)).label(...)`；`depth()` 在 Markdown 视图内没有任何读取点；大纲容器 `w(px(240.0)).h_full()` 嵌入内容 `h_flex`。
- 优化方向：按深度呈现层级并采用浮层/可收起覆盖层，不改变正文可用宽度。

### 重要｜渲染态缺少键盘滚动、大纲与链接激活

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/mod.rs` 的 `init` 键位表；`slterm_app/src/app_shell/markdown/view/render.rs` 的链接 `on_click` 与大纲按钮；GPUI `list` 的滚轮处理。
- 现象：`ToggleMarkdownOutline` 有 action 与按钮，但没有键位绑定；渲染链接只实现鼠标 `on_click`，没有焦点/Enter 激活；虚拟列表只处理滚轮，没有键盘滚动绑定。
- 根因：键位与可聚焦元素只覆盖形态切换、查找与缩放，未覆盖大纲、链接和滚动。
- 影响：`featureSpec` 的“键盘可完成形态切换、滚动、链接、大纲、查找和重置”不成立，键盘与辅助技术用户无法完成基本阅读操作。
- 证据：`init` 只绑定 `ToggleMarkdownMode`、查找、缩放、保存、复制、全选；视图无滚动键位；`InteractiveText` 只注册 `on_click`；大纲按钮无键盘入口。
- 优化方向：补齐渲染态的键位与可聚焦交互，链接与大纲项提供键盘激活路径。

### 重要｜安全与授权关键路径只有服务/投影测试，视图门控未被验证

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/tests.rs`；`slterm_app/src/app_shell/markdown/remote_resource.rs` 测试；`slterm_app/src/app_shell/markdown/local_resource.rs` 测试；`slterm_document/src/markdown_html.rs` 无独立测试。
- 现象：“点击前零请求”实际由视图 `ensure_image` 的授权代次门控决定，但测试只验证服务层 `peek_remote` 与服务层点击；`srcset` 独立判定没有任何测试；UNC/设备路径与“解析先于授权”没有测试；HTML 净化属性（srcset、width/height、open、src 的本地/远程分类）没有测试。追溯矩阵却把远程图片行标为已自动化。
- 根因：这些声明被拆在投影、服务和视图三层，测试只覆盖中间层；视图层需要 GPUI 虚拟窗口与注入服务，现有测试没有搭该组合。
- 影响：授权边界与安全承诺的回归不会被发现；追溯矩阵的“已自动化”不代表行为真值被覆盖。
- 证据：`markdown_remote_image_click_authorization` 只调用 `MarkdownImageService::peek_remote`／`fetch_remote_on_click`；`srcset` 全仓无测试；本地资源测试直接构造请求，不经过 `MarkdownView::ensure_image`；无 UNC/解析顺序用例。
- 优化方向：在视图层注入图片/远程端口，断言渲染可见图片不触发网络、点击仅一次、拒绝的 URL 形态不进入 resolver；为 srcset 与 HTML 属性补投影测试。

### 一般｜其余视图排版与降级声明缺少测试锚

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/tests.rs`、`render_selection_tests.rs`、`slterm_app/src/app_shell/markdown/progressive.rs` 测试、`slterm_app/src/app_shell/markdown/formula.rs` 测试。
- 现象：行内公式/图片的实际排版、公式失败时的视图降级、高 DPI 尺寸、大文档“当次仍要渲染”动作、隐藏 Render 视图不重解析、空选区不覆盖剪贴板、Mermaid 不产生渲染器/网络请求等声明均无视图级断言；`single_block_failure_falls_back_to_source_without_blocking_neighbors` 只测试调度器合成失败。
- 根因：现有测试以服务/投影/状态单元为主，视图渲染断言集中在文本与选择，未覆盖资源与排版组合。
- 影响：上述行为错误可在门禁全绿时存在；追溯与验收文字强于测试证据。
- 证据：`view/tests.rs` 无公式/图片渲染断言；`render_selection_tests.rs` 只断言拖选复制；`progressive.rs` 测试直接注入 `BlockCompletion::failure`；搜索不到大文档按钮或隐藏视图用例。
- 优化方向：补视图级组合用例，覆盖行内排版、失败原因、DPI、当次授权与隐藏视图不调度。

### 一般｜单块渲染任务总是直接成功，失败降级路径在生产中不可达

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view.rs` 的 `spawn_render_batch`；`slterm_app/src/app_shell/markdown/progressive.rs` 的 `BlockCompletion::success`／`failure`、`MarkdownBlockFailure`。
- 现象：`spawn_render_batch` 在后台把每个任务直接包装成 `BlockCompletion::success`，没有执行任何可能失败的块工作；`MarkdownBlockFailure` 在生产代码中没有构造点。所谓“单块失败回退源码”只在测试直接调用 `failure` 时出现。
- 根因：渐进调度器目前只是占位任务队列，真实块呈现仍在渲染帧内同步完成；失败原语先于真实块工作落地。
- 影响：测试给出了“失败隔离已覆盖”的错误安全感；若未来加入真实块渲染，降级路径与预算没有经过生产接线验证。
- 证据：`spawn_render_batch` 内的 `map(|task| BlockCompletion::success(...))`；全仓 `BlockCompletion::failure` 只在 `progressive.rs` 测试与定义中出现。
- 优化方向：要么把真实可失败的块准备工作接入任务，要么明确删除“生产单块失败”表述并保留测试为纯调度器契约。

### 一般｜资源与链接失败不向用户显示原因

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/resources.rs` 的 `complete_image`、`complete_formula`；`slterm_app/src/app_shell/markdown/formula.rs` 的 `FormulaFailure`、`FormulaFailureKind`；`slterm_app/src/app_shell/markdown/view/render.rs` 的图片占位与公式回退；`slterm_app/src/app_shell/markdown/link.rs` 与 `slterm_app/src/app_shell/app_root/markdown_links.rs` 的错误处理。
- 现象：图片失败只记录 URL 键，占位显示 alt 或路径，不区分未加载、失败与失败原因；公式失败只把 key 放入 `formula_failures` 并回退源码，`FormulaFailure` 的 kind/source 被丢弃；“立即打开/不支持类型/目录/不可读/越权”等链接失败在 AppRoot 只记录 `log::warn!`，没有非阻塞提示。
- 根因：失败原因在后台任务返回时被压缩为布尔失败，视图层没有面向用户的错误通道。
- 影响：`featureSpec` 的“资源失败保留 alt、路径与简短原因”“公式失败显示源码和错误”“目录、不存在或不可读目标显示非阻塞原因”均未满足；用户无法区分未加载、失败与待点击。
- 证据：`complete_image` 的 `Err(_) => self.image_failures.insert(...)`；`complete_formula` 的 `Err(())`；渲染占位无原因文本；`open_markdown_link` 的 `Err(error) => log::warn!`；`finish_document_open` 的失败只 `log::warn!`。
- 优化方向：把封闭失败类别与 i18n 短原因带入块级资源状态；链接失败在面板/通知中给出非阻塞原因，且不泄漏路径与正文。

### 一般｜超过 5 MB/100,000 行的源码降级不显示原因

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/render.rs` 的 `render_markdown`、`render_toolbar`；`slterm_app/src/app_shell/markdown/progressive.rs` 的 `MarkdownRenderGate::evaluate`。
- 现象：命中渐进阈值但未获得当次授权时，决策为 Source，工具栏只出现“仍要渲染”按钮；没有说明为何没有进入渲染态。超过 50 MB 的 Forbidden 分支才有固定提示。
- 根因：门禁只返回三态，视图没有把 `Source` 的两种成因（用户选择源码 vs 大文档降级）分开呈现。
- 影响：`featureSpec` 的“超过 5 MB 或 100,000 行时默认强制源码态，显示原因并允许用户显式仍要渲染”缺少原因部分；用户无法判断是设置、模式还是规模导致未渲染。
- 证据：`render_toolbar` 只在 `view.mode == Render && decision == Source` 时加 `RenderLargeMarkdown` 按钮，没有对应说明文本；`effective_mode` 合并了模式与大文档两个因素。
- 优化方向：区分降级原因并在源码态给出可见说明与当次授权入口。

### 一般｜隐藏的渲染态视图仍随共享 Document 编辑全量重解析

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/documents.rs` 的 `apply_document_edit` 广播；`slterm_app/src/app_shell/markdown/view.rs` 的 `apply_edit_snapshot`、`schedule_parse`、`adopt_snapshot`。
- 现象：Document 编辑会广播给所有项目中所有引用该 Document 的 Markdown 面板，不检查可见性或激活状态。Render 态 Markdown 视图收到编辑后立即复制整篇文本并在后台重新解析，解析完成还会重置滚动、清块滚动句柄、清资源与查找状态。源码态视图因 `schedule_parse` 的前置判断不会解析，但隐藏的 Render 态视图会。
- 根因：Markdown 视图没有可见性/活跃输入，调度只按 `mode` 与大小判断；AppRoot 广播不区分展示状态。
- 影响：后台标签页或隐藏项目在另一视图编辑时持续做不可见渲染，违背 `featureSpec` “源码编辑时不做不可见渲染，切回渲染态再计算”；造成 CPU/内存 churn 与恢复位置重置。
- 证据：编辑广播遍历全部 `PanelContent::Markdown`；`apply_edit_snapshot` 无条件 `adopt_snapshot` + `schedule_parse`；`schedule_parse` 只检查 `revision`、`mode` 与大小门禁，无可见性条件。
- 优化方向：把可见/激活状态纳入调度闸门，隐藏视图只标脏，重新可见时再解析。

### 一般｜公式编译之外的布局缓存没有生产消费者

- 置信度：确认
- 位置：`slterm_app/src/math/cache.rs` 的 `MathLayoutCache`、`LAYOUT_CACHE_BUDGET`；`slterm_app/src/app_shell/markdown/formula.rs` 的 `FormulaBitmapCache::render`；`slterm_app/src/math/CLAUDE.md` 的操作章节。
- 现象：`MathLayoutCache` 只被数学语料测试使用；生产公式路径每次都调用 `compile_formula_source`，没有布局缓存。`math/CLAUDE.md` 把 `MathLayoutCache` 列为消费面。
- 根因：位图缓存先落地，布局缓存未接入视图；模块整体 `allow(dead_code)` 掩盖了无消费者事实，文档也未同步。
- 影响：缩放/DPI 变化带来的同公式新位图键会重复排版；文档把未使用 API 描述为生产消费面，后续维护会误判覆盖。
- 证据：全仓检索 `MathLayoutCache` 只命中 `math/cache.rs` 与 `scientific_corpus.rs`；`formula.rs` 直接 `compile_formula_source`；`app_shell/mod.rs` 对 `markdown` 模块整体关闭 dead_code。
- 优化方向：接入或删除布局缓存，并让模块文档只描述真实消费面。

### 一般｜文档与实现的光学补偿规则冲突

- 置信度：确认
- 位置：`slterm_app/src/math/CLAUDE.md` 的“单点编译”；`slterm_app/src/math/compile.rs` 的 `compile_formula`、`compile_formula_source`。
- 现象：文档写明“光学补偿（`OPTICAL_SCALE`）只在 `compile_formula` 生效”，但 `compile_formula_source` 同样乘以 `OPTICAL_SCALE`；测试还要求两条入口产生相同布局。
- 根因：文档未随 source 入口收编更新，或代码保留了第二处补偿点。
- 影响：未来只按文档修改 `compile_formula` 会破坏 Markdown 公式字号或测试；缓存键/名义字号约定也会被误读。
- 证据：`compile.rs` 两个入口都调用 `layout_formula(..., pixel_size * super::OPTICAL_SCALE, ...)`；`original_multiline_formulas_keep_operators_and_alignment_rows` 断言两者相等。
- 优化方向：统一规则与文档，明确 optical scale 的唯一执行点及其对 Markdown 路径的影响。

### 一般｜追溯矩阵把阅读状态恢复锚到仅状态存储单元测试

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的 Markdown 节；`slterm_app/src/app_shell/text_view/view_state.rs` 的 `markdown_view_state_restore`；`slterm_app/src/app_shell/app_root/tests/documents.rs` 的 `markdown_panel_restores_persistent_reading_state`。
- 现象：追溯行“形态、大纲、缩放和阅读位置按同一 ViewKey 恢复”指向 `markdown_view_state_restore`，该用例只验证 `EditorViewStateStore` 的编解码与字段合并，不构造 `MarkdownView`、不经过 SQLite 重启；真正的真实恢复用例 `markdown_panel_restores_persistent_reading_state` 已在 `progress.md` 被引用，却没有进入追溯矩阵。
- 根因：追溯锚未随实现更新；弱用例先落地后被当作最终证据。
- 影响：追溯矩阵低估了已有覆盖、也高估了弱用例的语义；后续维护者无法从矩阵找到真实恢复路径的回归锚。
- 证据：`markdown_view_state_restore` 只对 `ViewStateRegistry` 与 JSON 断言；`markdown_panel_restores_persistent_reading_state` 使用 `WorkspaceWriterRuntime`、关窗、重启并断言 `reading_top`；追溯表该行仍写前者。
- 优化方向：追溯行指向真实 SQLite／重启用例，或同时列出两层并按层级说明职责。

### 一般｜公式观感豁免引用不存在的兜底测试与过期模块归属

- 置信度：确认
- 位置：`.claude/docs/testing/exemptions.md` 的 Markdown 数学公式渲染行；`slterm_app/src/scientific_corpus.rs` 测试；`slterm_app/src/math`。
- 现象：豁免行把兜底测试之一写作 `code_and_math_edit_bodies_without_touching_authored_fences`，全仓找不到该测试函数；归属模块仍写 `slterm_app/src/app_shell/file_editor`，而当前 Markdown 公式消费在 `slterm_app/src/app_shell/markdown`。
- 根因：豁免条目未随 Markdown 视图迁移与测试改名同步维护。
- 影响：豁免的兜底层级不成立，违反“新增/修改豁免行时必须核验兜底用例名与路径仍存在”的纪律；复审无法确认覆盖真实存在。
- 证据：全仓检索不到该测试名；现有兜底为语料测试与公式缓存测试，均不覆盖编辑正文时围栏保持；归属路径与当前模块不一致。
- 优化方向：恢复或替换缺失测试锚，并把归属与兜底描述更新到当前 Markdown 路径。

### 一般｜视图测试夹具使用固定 `D:/work` 路径并在渲染时触发真实解析

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/tests.rs` 的 `view_for_source`、`markdown_ctrl_wheel_zooms_and_ctrl_zero_resets`；`slterm_app/src/app_shell/markdown/view/render_selection_tests.rs` 的 `selection_view`。
- 现象：测试文档位置固定为 `D:/work/README.md`／`D:/work/selection.md`，而非临时根。`markdown_ctrl_wheel_zooms_and_ctrl_zero_resets` 会真实绘制视图，图片 `pixel.png` 因而进入 `ensure_image` 的后台本地解析，目标解析为开发机 `D:\work\pixel.png`；机器人环境若存在该目录会读到仓库外文件。
- 根因：测试夹具以固定路径替代临时双根，且没有为图片资源注入假解析端口。
- 影响：测试非封闭、依赖开发机文件系统；真实存在的同名图片或目录会改变加载结果与耗时，违反“临时双根、无共享全局状态、隔离”的测试纪律。
- 证据：`view_for_source` 的 `CanonicalPath::new("D:/work/README.md")`；`markdown_ctrl_wheel...` 调用 `visual.refresh()` 与 `window.draw`；`rich_text` 在渲染图片时调用 `ensure_image`；`SystemDocumentPathResolver` 会真实解析该路径。
- 优化方向：统一使用临时数据根与可注入的路径解析/图片端口，禁止视图测试触碰固定真实路径。

### 一般｜图片服务全局互斥锁覆盖远程网络与本地解码

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/resources.rs` 的 `schedule_image` 后台闭包；`slterm_app/src/app_shell/markdown/remote_resource.rs` 的 `UreqRemoteImageClient::fetch`；`slterm_app/src/app_shell/markdown/local_resource.rs` 的 `MarkdownImageService::load`。
- 现象：后台任务获取 `MarkdownImageService` 的互斥锁后，在锁内执行最多 10 秒的远程 HTTP、SMB/本地文件读取和位图解码。所有图片加载（包括其他文档、其他视图的本地/data 图片）被串行化；一次慢远程请求或挂起 UNC 会阻塞全部图片。
- 根因：把“缓存与授权索引”的状态锁和“网络/解码 I/O”放在同一个临界区。
- 影响：后台图片加载互相拖慢，滚动到新图片时的可见延迟放大；远程失败超时会成为全局图片路径的停顿点。
- 证据：`schedule_image` 的 `services.lock().map_err(...).and_then(|mut service| { if fetch_remote { service.fetch_remote_on_click(...) } ... })`；`fetch_remote_on_click` 在持锁期间调用 `self.remote.fetch`；`load` 在持锁期间读文件与解码。
- 优化方向：把网络/解码移出共享状态锁，锁只保护缓存与索引；按资源键做在途去重与结果回填。

### 一般｜合同冲突：Mermaid 静态渲染与普通代码块

- 置信度：确认
- 位置：`docs/featureSpec/panels/markdown.md` 的“安全 HTML、公式与 Mermaid”与验收判据；`docs/devPlan/design/markdown.md` 的“解析与块投影”与“当前实现验收”；`docs/devPlan/traceability.md` 的 Mermaid 说明。
- 现象：行为真值要求 `mermaid` 围栏静态图渲染并给出失败源码/错误；设计明确本轮只保留普通代码块、无专用 renderer；追溯将其标为延期并注明按用户裁决不实现。代码与设计一致：`mermaid` 作为普通代码块显示。
- 根因：featureSpec 终态与当前实现范围被显式裁开，冲突登记在追溯说明中。
- 影响：按合同的行为真值层级，featureSpec 的验收场景不成立；按设计，当前行为不是缺陷。需要在交付口径上保持“延期”而不是“已验收”。
- 证据：`markdown_options` 开启所需扩展但无 Mermaid renderer；`block_kind` 对 `mermaid` 只产生 `LanguageId::Mermaid` 代码块；`gfm_alerts_footnotes_math_and_fallbacks_share_the_projection` 与 `one_projection_drives_outline_visible_search_links_and_mermaid_code` 验证的是代码块；追溯 Mermaid 两行均为延期。
- 优化方向：保持追溯的延期状态并在验收说明中不与普通代码块混淆；是否需要提前实现由用户裁决，不在代码侧静默选择。

### 提示｜公式缓存键的 `theme_revision` 生产值恒为零

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/resources.rs` 的 `ensure_formula`；`slterm_app/src/app_shell/markdown/formula.rs` 的 `FormulaBitmapKey`。
- 现象：生产调用 `FormulaRequest::new(..., 0, color)` 永远传主题 revision 0；位图缓存实际只靠颜色、字号、DPI、缩放与源码散列失效。文档却把“主题 revision”列为缓存键组成部分。
- 根因：主题 revision 尚无真实来源，字段先占位。
- 影响：当前主题只改前景色的常见情况下无行为错误；若主题变化保持同色而依赖 revision 失效，缓存不会失效；文档与实现不一致。
- 证据：`ensure_formula` 的 `FormulaRequest::new(source, display, font_size, scale_factor, zoom, 0, color)`；`FormulaBitmapKey::new` 读取 `request.theme_revision`。
- 优化方向：接入真实主题 revision，或删除该键字段并修正文档。

### 提示｜长公式不进入块内滚动

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view/render.rs` 的 `MarkdownBlockKind::Math` 分支；`docs/devPlan/design/markdown.md` 的渲染调度条目。
- 现象：块级公式以 `max_w_full()` 缩放到容器宽度；设计要求“长公式在块内滚动”。超宽公式因此被整体缩小到不可读，而不是保持字号横向滚动。
- 根因：公式块只用了 `gpui::img` 的包含适配，没有套横向滚动容器。
- 影响：长推导/矩阵公式在高缩放下可读性下降，与代码块、表格的既定交互不一致。
- 证据：Math 分支 `gpui::img(image).max_w_full().object_fit(Contain)`，无 `overflow_x_scroll`；代码块与表格均有 `overflow_x_scroll`。
- 优化方向：为超宽公式提供与代码块一致的块内横向滚动，而不是等比缩小。

### 提示｜嵌套列表项的 GPUI 元素 id 冲突与选区映射偏移

- 置信度：确认（映射错误由代码路径确定；具体可见效果需真实交互）
- 位置：`slterm_app/src/app_shell/markdown/view/render.rs` 的 `render_list_item`、`rich_text`；`slterm_app/src/app_shell/markdown/view/selection.rs` 的 `selection_cursors`。
- 现象：列表项内容 id 由块 id、深度与同级序号组成；不同父项下同深度的子项会得到相同 id，后注册的 `TextLayout` 覆盖前者。`rich_text` 对列表/表格使用“在块可见文本中查找本项序列”的游标推进；当项内含嵌套列表夹在两个段落之间时，本项序列不是可见文本的连续子串，查找失败后段落的可见偏移会落到嵌套文本上。
- 根因：项 id 不是完整路径；可见范围在渲染时用文本查找倒推，而不是投影阶段直接给出每项的稳定可见区间。
- 影响：嵌套列表中的拖选高亮/复制可能对应到其他项或错误文字；元素状态也可能串扰。
- 证据：id 格式只含 `block.id`、`depth`、`index`；`rich_text` 的 `visible[*cursor..].find(&sequence).unwrap_or(0)` 在非连续序列时回退 0 并继续推进长度。
- 优化方向：用完整项路径生成 id；在投影阶段为每个列表项/表格单元格记录真实可见范围，消除渲染期文本搜索。

### 提示｜脚注锚点可能与标题锚点重名

- 置信度：确认
- 位置：`slterm_document/src/markdown.rs` 的 `Projection::unique_anchor`、`FootnoteDefinition` 锚点生成、`Projection::finish`、`MarkdownDocument::resolve_anchor`。
- 现象：标题锚点经过 `unique_anchor` 去重，脚注锚点直接使用 `fn-{anchor_base(name)}` 且不参与去重；标题文本本身为 `fn-x` 时会产生相同 id。`finish` 与 `resolve_anchor` 都取第一个匹配。
- 根因：锚点命名空间未按类型隔离，去重只覆盖标题。
- 影响：脚注链接或文档内链接可能跳到标题而非脚注，反之亦然；大纲与脚注共享锚点表时冲突。
- 证据：脚注锚点生成不调用 `unique_anchor`；`resolve_anchor` 使用 `find` 返回首个匹配；`finish` 的 target 解析同样取首个。
- 优化方向：为脚注与标题分配不重叠的 id 空间，或对全部锚点统一去重。

### 提示｜`MarkdownView::read_only` 只写不读

- 置信度：确认
- 位置：`slterm_app/src/app_shell/markdown/view.rs` 的 `MarkdownView` 字段与 `adopt_snapshot`。
- 现象：字段在构造与快照更新时赋值，但 Markdown 视图没有任何读取点；源码子视图自行从快照读取只读态。
- 根因：容器层保留了不再需要的状态副本；模块整体 `allow(dead_code)` 隐藏了该事实。
- 影响：读者会误以为渲染态另有一套只读门控；维护时可能错误地依赖该字段。
- 证据：全模块检索只有字段声明与两处赋值，无读取；`DocumentTextView` 的 `from_parts` 从快照取 `read_only`。
- 优化方向：删除该字段或让它成为唯一的只读判定来源。

## 跨域观察

- 相同的“先解析、后授权”顺序也存在于 `slterm_app/src/app_shell/documents.rs` 的 `DocumentOpenService::resolve_with_grants`／`resolve_link`：普通打开与本地链接都会先 `canonicalize`/打开真实路径，再交给 `PathPolicy`。本篇的自动图片路径是最严重入口，但该模式属于资源与链接授权链的共性问题。
- 远程图片允许 `http(s)` 到任意主机（包括 loopback/LAN），仅受用户点击约束；这不是 featureSpec 的违约（规范明确允许 http/https），但若未来把远程图片授权扩大到自动加载，会形成内网探测面。

## 存疑未证

- `SanitizedSvg` 限制输入节点、深度与属性，但 `usvg` 的 `<use>` 展开发生在其内部，依赖库自身的 1,000,000 节点与深度 1024 上限；一个 4,096 节点的恶意 SVG 理论上可展开到远超净化预算的 usvg 节点数。需要真实内存/耗时测量确认是否构成资源风险。
- `selection.rs` 的 `begin_text_selection`／`extend_text_selection`／`end_text_selection` 均未调用 `cx.notify()`；拖选过程中的高亮是否即时重绘取决于 GPUI 对指针事件的默认重绘行为。需要用真实窗口观察确认是否存在“拖动时高亮滞后到下一帧/下一次通知”的问题。
- 真实 SMB 目标上的 NTLM 认证行为、`dunce::canonicalize` 对 UNC 的具体网络触发时机（canonicalize 本身还是随后的 `OpenOptions`）需要实机抓包确认；代码层面的“解析先于授权”与“UNC 未被分类拒绝”已确定。
