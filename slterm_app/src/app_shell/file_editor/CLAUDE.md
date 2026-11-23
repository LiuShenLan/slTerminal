# file_editor

> Document 的真实读盘、条件保存、自动保存协调与 Git 行标端口。

## 决策与红线

- **保存链单点**：`DocumentSaveService` 消费 core 捕获的不可变保存请求，统一执行「重新解析与 Write 授权 → 真实句柄身份、权限和磁盘字节复核 → core 编码 → `atomic_file` 原子替换」。保存只写当前绑定位置；Changed/ReadOnly/Unauthorized/TargetOpen/TargetExists/Unrepresentable/Io 均上抛，失败不破坏磁盘。原因：重定向、失权、外部修改与编码失败必须可观测且不得产生半份文件。
- **另存为边界**：`save_as` 使用用户选定目标的单文件 Write 授权，允许尚不存在的目标但拒绝悬空链接；目标关联其他打开 Document 时写前拒绝，既有目标必须取得 OverwriteConfirmed。原子替换前再次解析并比较目标，成功回执保持 DocumentId。原因：另存为不得覆盖其他实时聚合或跟随编码期间的重定向。
- **自动保存单点**：`interaction.document_autosave` 默认关闭，经全局 Settings 注入唯一 `DocumentAutosaveCoordinator`；接入统一 Registry 的编辑视图按 DocumentId 共享停止修改 1 秒、全部编辑视图失焦、最后引用与退出触发。只读、在途保存、外部冲突与 ReadRevoked 暂停；失败按请求 revision 抑制，新编辑解除。原因：自动保存不得按面板分叉或自动覆盖冲突。边界：协调器只产生待保存 DocumentId，实际写盘仍经统一保存服务；显式 Discard 不得被自动保存覆盖。
- **后台读盘**：读盘与解码只在后台执行体；`app_shell::documents` 依次执行解析、PathPolicy 授权、聚合查重、有界读取与身份复核。普通源码由 `DocumentTextView` 直接消费 Session，Markdown 由 `MarkdownView` 直接消费同一 Session；既有聚合的新视图不得再次读盘。原因：前台不得阻塞 I/O，授权、正文与文件身份不得因重定向错绑。
- **外部变化单点**：后台服务重新解析、授权并读取，回投只由 core Session 判定重载、冲突、删除、失权或改名；视图不得另持草稿覆盖规则。纯改名遇在途保存先在 AppRoot 排队，保存回执后再提交，避免同文件保存被旧位置误判失败。原因：watcher 迟到结果不得覆盖新编辑或错绑保存位置。
- **大文件语义**：真实字节数与内容分流由 core `DocumentContent` 决定；超过 50,000,000 字节直接进入完整只读分页，不把有限前缀注册为 Rope。超过 10,000,000 关闭文档内补全，超过 200,000,000 关闭语法。未知编码、非法序列和 NUL 不得建立可保存正文基线。
- **git gutter**：数据源只经统一 GitFacade；行标颜色只引用 `EditorThemeTokens`，不在组件写裸色值。只读文档不挂 gutter。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support`。
- 必测面：条件保存与原子失败、共享 Document 的 idle／失焦／最后引用／退出自动保存、保存失败 revision 抑制、外部修改与改名排队、大文件分页、真实重启后的 Editor／Markdown ViewState、gutter 映射。
- 像素级对齐与真实手势边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 保存只经统一保存链；不得从视图直接写盘。
- 新增编辑视图必须接入 AppRoot 的同步编辑、广播、自动保存与关闭链，不新建正文或撤销栈。
