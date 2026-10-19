# 应用壳组合层

> GPUI 壳的组合根：窗口/工作区装配、视觉语言、恢复注入闸门与子模块边界。

## 决策与红线

- **壳即前端**：壳内状态权威 = `SltermWorkspace` 直持 pane 实体与展示态，无投影层。原因：单进程直持状态，避免跨层同步。边界：子模块不得另建平行状态树。
- **诊断宏拦截**：`println!`/`eprintln!` 在本模块文本域内编译期报错；诊断统一走 `try_write_stderr`。原因：无效标准句柄上裸打印会 panic。
- **视觉语言**：仅暗色；Linear 为默认；暖黑明度阶梯、发丝线分隔、单强调色、等宽字体、无动效。原因：产品定位与「界面消失」的阅读优先。边界：色值以主题表为真值，不在壳内硬编码；状态用圆点而非 emoji。
- **恢复注入闸门**：恢复命令在首个提示符渲染完成（`TerminalRegistry.promptReady`）之后注入；cmd 无 shell integration → 固定 500ms 延迟兜底；10 秒超时兜底仍注入不劣于现状。原因：恢复命令与 shell 启动竞争 stdin 会污染输入。边界：不使用「输出静默窗」猜测就绪。
- **子模块边界**：`doc_tabs` 拥有打开路由单判定；`diff_tab` 拥有双栏只读 diff；`merge_tab` 拥有冲突三栏合并；`code_tab`/`file_editor` 拥有源码与预览；`file_drop` 拥有拖放载荷；`fs_watch` 拥有外部文件事件；`session_restore` 拥有布局转换对；`settings_pane` 拥有设置 UI；`theme`/`wallpaper` 拥有外观；`windowing` 拥有回窗 facade；`workspace` 拥有 tab/pane 生命周期。原因：组合层只装配，不复制子域语义。
- **布局单点**：会话布局只经 session schema 的 layout 转换对存取；壳不得第二套序列化。
- **静默启动**：首窗 `show=false`/`focus=false` 创建；本层任何路径不得 show/activate/聚焦窗口。原因：启动不得抢焦点或闪现。

## 验收与测试

- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：壳挂载与绘制、打开路由、布局快照/恢复、恢复注入闸门、静默启动不 show/activate、诊断宏拦截。
- 真实 GPU/桌面会话行为由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增 tab 或视图：先扩封闭枚举与单一注册点，再在组合层接线。
- 新增启动副作用：接入 `lib.rs::run()` 的同一启动链，不在子模块私开入口。
- 调整视觉语言：改主题表与语义 token，不在壳内写颜色字面量。
