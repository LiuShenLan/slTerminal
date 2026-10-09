# 06 设置 / 主题 / i18n 详细设计

> pebrel-design 分片 06/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/06-settings-theme-i18n.md`(spec 分片,含采纳点编号);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **`RuntimeSettings`、主题类型(`TermTheme`/`ReviewedPalette`/`ThemeDefinition` 族)、设置键域枚举、`Rgb` 颜色类型** 的锚点归属篇(01 篇类型锚点表已登记):上述类型在此签名级定义,他篇(02/03/04/05/07/09 等)只许 `use` 或经 facade 传参,禁重定义。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

设置 / 主题 / i18n 在 GPUI 单进程世界重建为「**一 crate 管值、一库管格式、一壳管界面、一 build 管语言**」四件套:

- **`slterm_settings` crate**(core,零生产依赖契约,M1 首迁):设置值模型权威——`RuntimeSettings` 单一权威大结构、键域枚举族、宽容解析合同、`languages!` 语言注册表(裁 `System`/`ZhCn`/`EnUs` 三家)、`AgentHook` 偏好段、主题值模型(`ThemeDefinition` 族 + `ReviewedPalette` 语义槽 + WCAG 校验纯函数)与 JSON 持久化契约。模块头分层原则照抄 pebrel `nebula_settings/src/custom_theme.rs`:**值模型零序列化零文件格式,文件格式归应用侧适配器,crate 是无依赖权威**。
- **`slterm_app::theme_library`**(application,归 M6 点亮):应用侧文件格式层——版本化 JSON 信封(`ThemeDocument`/`SCHEMA_VERSION`)、有界导入导出适配器、带 `RevisionPrecondition` 乐观并发的持久化 store、主题包 ZIP。「设置层管值、应用层管格式」单方向依赖照抄 pebrel `nebula_app/src/theme_library/mod.rs`。
- **`slterm_app::gpui_shell::settings_pane`**(application,归 M6 点亮):GUI 设置页照抄 pebrel 形态——左导航 + 右表单、路由式搜索、统一 `try_persist` 写通道;导航元数据(路由 id + 双语搜索别名 + 分组)吸收进 slTerminal `SettingsPageRegistry` 注册表家族契约(硬约束 #13 跨语言保留)。
- **i18n 构建管线**(application):build.rs 编译期从 `slterm_app/i18n/*.json`(只留 en-US/zh-CN 两目录)静态生成 typed `Message` 枚举 + `MESSAGES` 零分配二维表;en-US 与 zh-CN 键集完全相同且非空是构建期硬合同(不等即 panic);独立合同 workspace(`tools/i18n-contract` 等价物)经 `#[path]` 复编生成器与查询核心,合同测试可脱离应用壳运行。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_settings/src/lib.rs` | pebrel 照抄改名+JSON 化改造 | `RuntimeSettings`/`RawSettings`(JSON 形态)/键域枚举族/`parse_hex_rgb` 族/JSON 段级浅合并写通道契约(本篇锚点) |
| `slterm_settings/src/themes.rs` | pebrel 照抄裁剪 | 内置主题目录(`ThemeName::BUILTIN`/`available`)+ `ReviewedPalette`/`FreshPalette` 语义槽(仅暗色成员) |
| `slterm_settings/src/custom_theme.rs` | pebrel 照抄裁剪 | `ThemeDefinition` 值模型全家 + WCAG 对比度校验族(`ThemeAppearance` 不进模型) |
| `slterm_settings/src/language.rs` | pebrel 照抄裁剪 | `languages!` 宏 + `LanguagePref`/`LanguageInfo`/`from_locale`(11 家裁 3 家) |
| `slterm_settings/src/agent_hooks.rs` | pebrel 照抄改名 | `AgentHook` 九变体偏好段(`settings_key` → `slterm_*` 命名空间) |
| `slterm_settings/src/scrolling.rs` / `notifications.rs` / `ligatures.rs` / `cursor_motion.rs` | pebrel 照抄 | 档位约束 + 归一钳制小域 |
| `slterm_settings/src/reset.rs` | pebrel 照抄改造(JSON 化) | `restore_default_settings`:备份 → 删已知键 → 未知键/用户数据保留;重置覆盖对账 |
| `slterm_settings/src/keys.rs` | 新建 | **设置注册表**:键名/默认值/域归属/搜索别名单源,白名单与重置清单由注册表驱动 |
| `slterm_settings/src/paths.rs` | 缝合改写(归 01 篇锚) | `settings_dir`/`settings_path`(`SLTERM_CONFIG_DIR` 优先,默认 `%APPDATA%\slterm`,叶子名 = 01 篇 D2 裁决值) |
| `slterm_app/src/theme_library/` | pebrel 照抄 | `ThemeDocument`/`ThemeLibraryStore`/`RevisionPrecondition`/主题包 ZIP |
| `slterm_app/src/gpui_shell/settings_pane.rs` + `settings_pane/` | pebrel 照抄裁剪 | 设置页壳 + 各节页(页集合按分片归属重排) |
| `slterm_app/src/gpui_shell/theme.rs` | pebrel 照抄裁剪 | 主题解析单点(`resolve_theme_name` 仅暗色化)+ 调色板 → GPUI token 注入 |
| `slterm_app/build/i18n.rs` + `build/i18n/catalog.rs` | pebrel 照抄 | 静态生成器全家 |
| `slterm_app/src/i18n/` | pebrel 照抄 | `UiLanguage::text`/`tr`/`pick`/`format` 查询 API + `format::substitute` + `locale::system_locale` |
| `slterm_app/tests/i18n_contract.rs` + `tools/i18n-contract/` | pebrel 照抄 | 零分配/小栈/payload 预算合同测试 + 独立合同 workspace |
| `slterm_app/src/platform/locale.rs` | pebrel 照抄裁剪 | Windows `GetUserDefaultLocaleName` + POSIX 环境变量回退(macOS 分支砍) |

**裁剪不迁**(spec 不采纳点):`RawSettings::from_text` 行级 txt 协议整族、keybind 多行协议(键位归 07,JSON 段内数组)、`quick_terminal.rs` 整支、`paths/migration.rs` 迁移本体(零迁移,D06-2 已裁,无服务对象)、亮主题全套、`app_icon.rs` 25 色板产品功能(宏形态吸收)、SSH 代理三键、SVN 档、`tabs_position` 键(不采纳「tabs 位置配置化」)、`BlurModeName` 五档材质枚举的 pebrel 形态(blur 值域已裁 = 五材质枚举(默认 None)+ opacity 标量,D06-4/D09-1,本篇字段一次到位锚定)、其余九语目录、`nebula.toml` 层级、mobile/backup/providers 设置页。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri 命令 / Dockview / xterm.js / vitest / CM6 / CSS 变量的词一律只在「消亡 / 映射 / 来源」语境。
2. **类型锚点纪律**:本篇唯一定义 `Rgb`/`Rgb8`/`Rgba8`、`RuntimeSettings`、键域枚举族(`CursorShapeName`/`AcceptKeyName`/`CompletionStyleName`/`DensityName`/`NewTabPositionName`/`TabRevealName`/`VcsDisplayName`/`BellModeName`/`CellWidthModeName` 等)、`ThemeName`、`TermTheme`/`ExactTermColors`、`ReviewedPalette`/`FreshPalette`、`ThemeDefinition`/`TerminalThemeColors`/`IndexedPalette`/`ThemeUiColors`/`ThemeTypography`/`ThemeLayout`/`ThemeEffects`/`ThemeValidationError`、`LanguagePref`/`LanguageInfo`、`AgentHook`、`Ligatures`、`CursorMotion`、`NotificationDuration`、scrolling 档位族(`SCROLLBACK_VALUES` 等)。他篇引用纪律:只许 `use` / facade 传参;**05 篇 session schema 的 `Rgb` 消费本篇**;**03 篇 `AgentHook`/`ai_hooks` 开关键消费本篇**(03 篇已登记);**02 篇 `conpty_input_modes` 键域枚举消费本篇**(02 篇已登记,键名常量引用纪律不变)。
3. **JSON 单源**:设置持久化唯一权威 = 数据目录 `settings.json`(路径承 01 篇 D2/D3);无 txt/Lua/toml 层,无双层优先级;「未知键保留、读-改-写」语义由 JSON 段级浅合并继承(spec 采纳点 4)。
4. **仅暗色**:主题模型只带暗色语义;`ThemeAppearance`/`is_light`/`LIGHT_FOREGROUND`/`LIGHT_ANSI`/`follow_system_theme`/亮暗双权重混合全部不进 slterm 模型;内置主题目录只留暗色成员(亮成员见 D06-3)。
5. **写通道纪律**(00-roadmap 跨领域不变量 1 的设置域落地):一切设置写盘 = 保存锁串行化「读-合并-写」+ 临时文件 + rename + `.bak`;损坏文件禁覆盖(读侧回退 .bak 并标记 corrupted,写侧拒绝落盘);1MB 上限;白名单经设置注册表驱动(见改造节 2)。
6. **设置页三纪律**:预览不脏盘(滑条拖动 = 内存态即时生效,松手才归一化提交)、提交可回滚(写失败读回已存值复位控件)、失败可感知(写盘失败 toast + stderr 记录)——照抄 pebrel `gpui_shell/settings_pane.rs` 合同,JSON 化后落点换成新写通道。
7. **「显式值权威」三元模式不可违反**:默认值可来自主题/回退层,用户显式写过(哪怕恰是默认值)必须保持权威——`opacity_is_explicit`/`blur_is_explicit` 与 `pane_card_radius`/`pane_card_gutter`/`pane_card_shadow`/`pane_card_divider` 四键 `None`=跟随主题,是设置/主题优先级体系的地基;自定义主题的 `ThemeTypography`/`ThemeEffects` Option 字段同构。
8. **i18n 零分配合同**:无参查询(`text`/`tr`/`pick` 命中路径)零分配、零文件读、零锁;`format` 是唯一允许创建 `String` 的翻译路径;en-US/zh-CN 键集硬合同在构建期 panic 强制,不靠注释自律。
9. **凭据纪律**(SEC-18)延申:设置 JSON 与主题文档一律不得承载真实凭据值;providers 供应商配置归 10 篇凭据域,不进设置页全局组。
10. **布局恢复归 session,禁混入 settings**(05 篇边界承接):分屏比例/tab 集合快照一律走 session v4;settings 只存全局偏好;侧栏 collapsed/width/active_view 已裁归 session(D05-3),不进本篇键域。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致(改名点已标);JSON 化改造面在注释中注明。锚点类型他篇只许 `use`。

### 颜色类型(`slterm_settings`,本篇锚点)

```rust
// 终端/UI 调色板通用三元色 —— pebrel 同名类型照抄。
pub type Rgb8 = [u8; 3];
pub type Rgba8 = [u8; 4];

/// session schema 引用的可持久化颜色(05 篇锚点消费:`TabSession.color: Option<Rgb>`)。
/// serde 形态 = 三元数组 [r, g, b](05 篇 JSON 实例即夹具),禁字符串形态(防双记号)。
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub fn parse_hex_rgb(value: &str) -> Option<Rgb8>;    // #rgb/#rrggbb 宽容解析;非 hex/长度错/含中文一律 None
pub fn format_hex_rgb(rgb: Rgb8) -> String;           // 写盘统一六位小写;测试锁定往返
```

### 宽容解析层(`slterm_settings/src/lib.rs`,JSON 化改造)

pebrel `RawSettings` 的 txt 行协议整族不迁(spec 不采纳点 1);JSON 世界等价形态 = **JSON 对象树 + 逐键宽容读取**,行序保留/原地替换等文本补偿机制无对象(键唯一与类型由 JSON 自带):

```rust
/// settings.json 的宽容读取视图。键缺失/类型不符/值为 null 一律 None = 「键未设置」,
/// 由调用方回退默认值——「字段即合同」的宽容解析合同逐字段语义原样保留,只换载体。
/// 未知键不在本结构表达,它们留在原始 Value 树里由写通道浅合并保留(见数据流节)。
#[derive(Default)]
pub struct RawSettings {
    tree: serde_json::Value,          // 对象树;serde_json 为 settings crate 唯一新增依赖
}

impl RawSettings {
    pub fn load() -> Self;                                  // 损坏三态:Err→默认+.bak 回退归 app 适配层
    pub fn try_load() -> std::io::Result<Self>;             // 授权路径(安装器)不许把读取失败当默认
    pub fn from_json_bytes(bytes: &[u8]) -> Self;           // 宽容:解析失败 = 空树(不 panic)
    pub fn value(&self, key: &str) -> Option<&str>;         // 字符串值;非字符串/空串 → None
    pub fn f32(&self, key: &str) -> Option<f32>;            // 数值;NaN/∞ 由调用方钳(见 scrolling 归一)
    pub fn bool_on(&self, key: &str) -> Option<bool>;       // JSON 真布尔直取;**宽容八拼写随 txt 消亡**
    pub fn keys(&self) -> impl Iterator<Item = &str>;       // 已知键遍历——重置覆盖对账的数据源
}

/// 读-改-写语义三件套(pebrel `persist_keys`/`apply_updates` 语义,载体换 JSON):
/// 已有键替换、缺失键追加、未知键/注释段原样保留;全量读-改-写由保存锁串行化。
pub fn persist_keys(updates: &[(&str, serde_json::Value)]) -> std::io::Result<()>;
pub fn apply_updates(tree: &serde_json::Value, updates: &[(&str, serde_json::Value)])
    -> serde_json::Value;                                   // 纯函数,单测锁定
```

**宽容语义对照表**(txt → JSON,pebrel 机制 → 新世界机制):键大小写不敏感(txt 小写归一)→ JSON 键严格(注册表键名单源,无大小写双轨);空值=未设置 → `null`/缺键/空串均 None;布尔八拼写 → 真 JSON 布尔(设置页写盘 `Value::Bool`,手改 `"yes"` 不再是合法值——「宽容」降格为「类型不符不炸、回默认」,不兼容拼写宽容);行序保留 → 未知键在对象树原位保留(JSON 对象无序,语义等价)。读侧宽容、写侧白名单的净效果:**手改文件写错 = 该项回默认 + 键被保留,应用永不炸**。

### `RuntimeSettings`(本篇锚点,照抄+仅暗色/单窗裁剪)

```rust
/// 新 UI 消费的运行时设置,单一权威大结构。字段与出厂默认逐项对照 pebrel
/// `RuntimeSettings`;`Option` 字段 None = 键未设置,调用方自选回退。
/// 仅暗色裁剪:follow_system_theme/is_light 语义链不迁;单窗/定位裁剪见字段注释。
#[derive(Clone)]
pub struct RuntimeSettings {
    pub language: LanguagePref,                    // System/ZhCn/EnUs 三家
    pub theme: ThemeName,                          // 仅暗色内置目录成员
    pub font_family: Option<String>,
    pub font_family_cjk: Option<String>,
    pub ui_font_family: Option<String>,
    pub ui_font_size_px: Option<f32>,
    /// 逻辑像素(设置页 spinner 与 Ctrl+滚轮持久化时已除 scale factor)。
    pub font_size_px: Option<f32>,
    pub ctrl_wheel_font_zoom: bool,                // 默认开;关 = 手势整体消费
    pub ligatures: Ligatures,                      // On/Off/Theme 三态(默认开,偏安全侧)
    pub cursor_shape: Option<CursorShapeName>,
    pub cursor_blink: Option<bool>,
    pub cursor_motion: CursorMotion,               // 显式 opt-in(默认关)
    pub copy_on_select: bool,
    pub scrollback_lines: usize,                   // 七档白名单,档位外回默认
    pub scroll_speed: f32,                         // 0.25×–4× 步进 0.25,非有限回 1.0
    pub focus_follows_mouse: Option<bool>,
    pub dim_inactive_panes: bool,                  // 分屏 veil 开关联动 05 篇开放问题 1
    pub multiline_paste_confirm: bool,
    pub tab_close_visible: bool,
    pub terminal_proxy: bool,
    pub refresh_environment: bool,
    pub shell: Option<String>,                     // 原始 id 往返;解析归 02 篇 shell 检测层
    pub startup_directory: Option<String>,
    pub ghost: bool,                               // AI 内联补全 ghost text(eg 08 消费)
    pub accept: AcceptKeyName,
    pub completion_style: CompletionStyleName,     // Inline/Popup/Hybrid
    pub cjk_bold_regular: bool,
    // tabs_position 键不迁(不采纳 tabs 位置配置化)
    pub tab_reveal: TabRevealName,
    pub density: DensityName,
    pub new_tab_position: NewTabPositionName,
    // windowing_behavior 键不迁(B.10 已裁删键,单窗行为硬编码归 09 篇)
    pub cell_width_mode: CellWidthModeName,
    pub vcs_display: VcsDisplayName,               // auto/git 两档(SVN 砍)
    pub bell: BellModeName,                        // 关/闪烁/声音/两者
    pub ai_toasts: bool,
    pub notification_duration: NotificationDuration, // Default 档=不覆盖归 09 篇漏斗消费
    pub fetch: bool,
    pub auto_check_updates: bool,                  // 更新链归 09 篇;设置键本篇锚定
    pub auto_download_updates: bool,
    pub keep_session: bool,                        // restore_session 语义归 05 篇 session
    pub restore_session: bool,
    pub resume_ai: bool,                           //归 03 篇恢复注入消费
    pub tray: bool,                                //归 09 篇托盘消费
    pub silent_start: bool,
    pub launch_at_login: bool,                     // 开机启动;归 09 篇消费
    pub blur: BlurMode,                            // 五材质枚举(默认 None),D06-4/D09-1 已裁一次到位;显式权威模式保留
    pub opacity: f32,
    opacity_explicit: bool,                        // 「显式值权威」三元模式(不变更项 7)
    blur_explicit: bool,
    pub background: Option<Rgb8>,                  // 终端背景覆盖色(设置页取色器)
    pub theme_foreground: Option<Rgb8>,            // 前景覆盖;切主题可由调用方清除
    /// 序列化自定义主题标识/载荷。设置层只保存值,文件加载与格式解码归 theme_library。
    pub custom_theme: Option<String>,
    pub background_image: Option<String>,          // 壁纸路径(空=无);归 09 窗口特效渲染
    pub background_image_opacity: f32,
    pub background_image_fit: Option<String>,
    pub background_image_alignment: Option<String>,
    pub background_image_cover_chrome: bool,
    pub panel_resize: bool,
    // sidebar_width 键不迁(D05-3 已裁侧栏宽度归 session,05 篇锚定)
    // ssh_proxy_mode/url/no_proxy 三键不迁(SSH 砍)
    pub pane_card_radius: Option<f32>,             // None = 跟随主题自带几何(ThemeCardGeometry)
    pub pane_card_gutter: Option<f32>,             // 只作用于左/右/下三边(08-26 裁定照抄)
    pub pane_card_shadow: Option<bool>,
    pub pane_card_divider: Option<f32>,            // 卡缝归零形态的结构分界竖线
    // quick_terminal_* 四键不迁;app_icon/powerline 键不迁
}

impl RuntimeSettings {
    pub fn load() -> Self;                          // from_raw(&RawSettings::load())
    pub fn from_raw(raw: &RawSettings) -> Self;     // 逐键宽容解析:未知值回默认、范围钳制
    pub fn opacity_is_explicit(&self) -> bool;      // 用户显式写过(哪怕恰是 1.0)保持权威
    pub fn blur_is_explicit(&self) -> bool;
}
```

`ThemeCardGeometry`(`radius`/`gutter`/`shadow`/`divider` 四键一组)与 `DEFAULT_PANE_CARD_RADIUS` 常量族照抄,白边事故注释(旧壳/新壳各写一份字面量 = 白边来源)一并迁移;GPUI 世界 pane 卡几何消费随 workspace chrome,本篇只保留值模型槽与「跟主题走 + 显式覆盖」优先级。

### 键域枚举族四件套(`slterm_settings/src/lib.rs`,照抄锚点)

```rust
// 形态模板(CursorShapeName 例;AcceptKeyName/CompletionStyleName/TabRevealName/
// DensityName/NewTabPositionName/CellWidthModeName/VcsDisplayName/
// BellModeName 同构,个别无 ALL/VALUES 的按 GUI 需要补齐):
pub enum CursorShapeName { Bar, Block, Underline }
impl CursorShapeName {
    pub fn from_settings(value: &str) -> Option<Self>;  // 大小写/空白归一 + 历史别名兼容
    pub fn settings_value(self) -> &'static str;        // 写盘稳定值
    pub const ALL: [Self; 3] = [ /* GUI 枚举单源 */ ];
    pub const VALUES: [&'static str; 3] = [ /* 与 ALL 同序 */ ];
}
```

四件套——`from_settings`(宽容归一)、`settings_value`(稳定写盘值)、往返测试、`ALL`/`VALUES` 常量供 GUI 枚举——新设置键一律按此形态落地,杜绝散落字符串比较。`CompletionStyleName` 的 `active_style(popup_requested)`/`cycle()` 与 `BellModeName` 的 `visual()`/`audible()` 判定函数照抄(消费方归 02/08/09)。

### 语言注册表(`slterm_settings/src/language.rs`,照抄裁剪)

```rust
// languages! 宏形态不变,注册行从 11 行裁到 2 行(其余九语整删,spec 不采纳点 10):
languages! {
    ZhCn => ("zh-CN", "简体中文", "zh-CN"),
    EnUs => ("en-US", "English", "en"),
}
// 宏生成:LanguagePref 枚举(System=默认 + 变体)/ALL/VALUES/LANGUAGES/native_name/
// from_settings/settings_value —— 选择器、语言枚举、运行时查找表同源零分叉。
impl LanguagePref {
    pub fn from_locale(locale: &str) -> Option<Self>;
    // POSIX 后缀剥离(.UTF-8/@variant)、zh 的 Hans/Hant 与 TW/HK/MO 区域判定、
    // 地区变体回退主语言(en-GB→EnUs)——照抄;裁后 Hant/TW/HK/MO 分支无对象但宏形态不挡
}
```

### `AgentHook` 偏好段(`slterm_settings/src/agent_hooks.rs`,本篇锚点,03 篇消费)

```rust
pub enum AgentHook { Claude, Codex, OpenCode, Pi, Copilot, Grok, OhMyPi, Cursor, Kimi }
impl AgentHook {
    pub const DEFAULT: [Self; 2] = [Self::Claude, Self::Codex];  // 默认仅两家开启
    pub const ALL: [Self; 9] = [ /* 九家,与 03 篇 integrations::AGENTS 同序 */ ];
    pub const fn settings_key(self) -> &'static str;
    // pebrel: "ai_hooks_claude" 等 → slTerminal: "slterm_ai_hooks_claude" 命名空间(spec 采纳点 8);
    // 全局兜底键 "ai_hooks" → "slterm_ai_hooks"。键名挂在 agent_hooks 段内(段嵌套 D06-1 已裁)。
    pub fn enabled(self, raw: &RawSettings) -> bool;
    // 每 agent 独立键 → 历史全局键兜底 → 默认两家;「读-改-写保他人选择」语义照抄
    pub fn all_updates(enabled: bool) -> Vec<(&'static str, serde_json::Value)>;  // 显式安装/卸载写全部
    pub fn setup_updates() -> Vec<(&'static str, serde_json::Value)>;             // 恢复默认只写默认两家
}
```

与 03 篇缝合:安装权威(`ensure_claude_hooks` 族)读 `enabled` 做接入/卸载决策;`all_updates`/`setup_updates` 是 CLI setup-ai 与恢复默认路径的写盘入口,JSON 化后值形态 = 真布尔。

### 小域合同(`slterm_settings`,照抄锚点)

```rust
// scrolling.rs:七档白名单 + 归一钳制——「档位约束 + 归一钳制」模式适用一切未来数值键
pub const SCROLLBACK_VALUES: [usize; 7];
pub const DEFAULT_SCROLLBACK_LINES: usize;      // 终值归 02 篇 D02-3(默认随 pebrel 10000,M3 实测再评)
pub const DEFAULT_SCROLL_SPEED: f32; pub const MIN_SCROLL_SPEED: f32;
pub const MAX_SCROLL_SPEED: f32; pub const SCROLL_SPEED_STEP: f32;
pub fn normalize_scroll_speed(raw: f32) -> f32;  // 非有限/越界 → 默认 1.0
pub fn scrollback_lines(raw: Option<usize>) -> usize; // 档位外一律回默认

// notifications.rs:默认/5/10/30/90/常驻六档;Default 是不覆盖而非具体秒数
pub enum NotificationDuration { Default, S5, S10, S30, S90, Persistent }
impl NotificationDuration { pub fn timeout(self) -> Option<Duration>; }  // None=原本常驻归 09 消费

// ligatures.rs:On/Off/Theme 三态;Theme 档决定权交主题 typography 声明
pub enum Ligatures { On, Off, Theme }
impl Ligatures { pub fn enabled(self, theme: &ThemeTypography) -> bool; } // 默认开,无效值回默认

// cursor_motion.rs:显式 opt-in(默认关),无效值回默认
pub struct CursorMotion(pub bool);
```

### settings.json 实例(JSON 化后权威文件形态)

键形态 = 段嵌套(D06-1 已裁:appearance/scrolling/… 各成段,写通道按段浅合并);`slterm_*` 前缀按 spec 采纳点 8;未设置键落盘为 `null` 还是缺键归实现期(段内同语义):

```json
{
  "appearance": {
    "language": "system",
    "theme": "catppuccin-mocha",
    "font_family": "CaskaydiaCove Nerd Font",
    "font_size": 14.0,
    "opacity": 1.0,
    "custom_theme": null,
    "pane_card_radius": null
  },
  "scrolling": {
    "scrollback_lines": 10000,
    "scroll_speed": 1.0
  },
  "interaction": {
    "cursor_shape": "bar",
    "ligatures": "theme",
    "bell": "both",
    "notification_duration": "default"
  },
  "session": {
    "restore_session": true,
    "resume_ai": true
  },
  "system": {
    "ai_toasts": true,
    "tray": false
  },
  "agent_hooks": {
    "slterm_ai_hooks": true,
    "slterm_ai_hooks_claude": true,
    "slterm_ai_hooks_codex": true
  },
  "future_unknown_key": { "用户手改段": "原样保留" },
  "keybind": [ { "combo": "ctrl+shift+c", "action": "copy" } ],
  "conpty_input_modes": { "win10_legacy": 3, "win11": 7 }
}
```

`keybind` 段(eg 07 锚定结构)、`conpty_input_modes` 段(eg 02 锚定矩阵)进同一文件不同段——05 篇边界不变:布局恢复永远不进这个文件。

### 内置主题目录(`slterm_settings/src/themes.rs`,本篇锚点,照抄裁剪)

```rust
impl ThemeName {
    /// 可选清单常量化——目录即合同。仅暗色成员 = pebrel 暗色子集 + linear
    /// 作默认主题入目录(D06-3 已裁);pebrel 16 家含 8 家亮主题,亮成员整删。
    pub const BUILTIN: [Self; N] = [ /* 暗色子集 */ ];
    pub const BUILTIN_NAMES: [&'static str; Self::BUILTIN.len()];  // 持久化名=展示名

    /// 退休标识仍可解析但映射到继任主题:老用户配置不炸、选择器不出现。
    pub const fn available(self) -> Self;   // Nebula→CatppuccinMocha 等映射表照抄形态
    pub fn fresh_palette(self) -> Option<FreshPalette>;
}
pub struct FreshPalette { pub shell: Rgb8, pub surface: Rgb8, pub accent: Rgb8,
                          pub foreground: Rgb8, pub muted: Rgb8 }
// pebrel FreshPalette 带 is_light 字段——仅暗色裁定后字段不迁,形态退化为纯暗色槽组。
```

### 终端色表影响模型(`slterm_settings/src/lib.rs`,本篇锚点)

```rust
/// 一个主题对终端色表的全部影响。None 字段 = 主题未声明,调用方保留用户配置
/// 或宿主默认的显式缺省语义照抄;整表声明(Nord)与只声明背景两种形态并存。
pub struct TermTheme {
    pub background: Rgb8,
    pub exact: Option<ExactTermColors>,
    // is_light 字段不迁(仅暗色);powerline 八槽位不迁(spec 不采纳点 8)
}
pub struct ExactTermColors {
    pub foreground: Rgb8,
    pub ansi: [Rgb8; 16],
    pub cursor: Option<Rgb8>, pub cursor_text: Option<Rgb8>, pub cursor_stroke: Option<Rgb8>,
    pub selection_foreground: Option<Rgb8>, pub selection_background: Option<Rgb8>,
}
impl ThemeName { pub fn term_theme(self) -> TermTheme; }
```

### UI 语义槽(`slterm_settings/src/themes.rs`,本篇锚点)

```rust
/// shell/background/foreground/muted/accent + selected/line(带 alpha)+ 语义六色
/// + frame 固定槽位集。「终端色表」与「UI chrome 色」两套数据分离、各自单源;
/// 值静态,适配器不得去饱和 accent 或合成第二条选中色带。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewedPalette {
    pub shell: Rgb8, pub background: Rgb8, pub foreground: Rgb8,
    pub muted: Rgb8, pub accent: Rgb8,
    pub selected: Rgba8, pub line: Rgba8,
    pub red: Rgb8, pub green: Rgb8, pub yellow: Rgb8,
    pub blue: Rgb8, pub purple: Rgb8, pub cyan: Rgb8,
    pub frame: Rgb8,
}
impl ReviewedPalette {
    /// selected 前景色按 alpha 叠到 pane 底的 CSS 式合成——code_background 照抄。
    pub const fn code_background(self) -> Rgb8;
}
impl ThemeName { pub const fn reviewed_palette(self) -> ReviewedPalette; }
```

GPUI 主题 token 槽位集 = 本结构 × slTerminal 旧 `UiTokens` 区域组融合(改造节 4),槽位组织粒度实现期定,纪律一致:**语义槽单点、消费只引用槽**。

### 用户主题值模型(`slterm_settings/src/custom_theme.rs`,本篇锚点,照抄裁剪)

```rust
/// 完整用户主题。base 是起点快照的元数据,不建运行时继承链;克隆定义自含
/// 全部字符串与数组。模块头原则照抄:本文件零序列化零文件格式。
pub struct ThemeDefinition {
    pub name: String,
    pub base: ThemeName,
    // appearance: ThemeAppearance 不迁(仅暗色)——双权重混合简化为暗色单权重
    pub terminal: TerminalThemeColors,
    pub ui: ThemeUiColors,
    pub typography: ThemeTypography,
    pub layout: ThemeLayout,
    pub effects: ThemeEffects,
}
impl ThemeDefinition {
    pub fn from_builtin(base: ThemeName) -> Self;    // 内置解析为可编辑具体快照
    pub fn snapshot(base: ThemeName) -> Self;        // 适配器别名
    pub fn validate(&self) -> Result<(), ThemeValidationError>;  // 全标量范围校验
    pub fn derived_ui(&self) -> ThemeUiColors;       // 唯一共享 derive 规则
    pub fn resolved_ui(&self) -> ThemeUiColors;      // ui.derive ? 派生 : 显式
    pub fn with_terminal_foreground(&self, fg: Rgb8) -> Self;    // with_* 不可变编辑
    pub fn with_cursor(&self, color: Option<Rgb8>, text: Option<Rgb8>, stroke: Option<Rgb8>) -> Self;
    pub fn with_selection(&self, fg: Option<Rgb8>, bg: Option<Rgb8>) -> Self;
    pub fn with_ansi(&self, index: usize, color: Rgb8) -> Option<Self>;
    pub fn with_indexed(&self, index: usize, color: Rgb8) -> Option<Self>;
    pub fn parse_color(value: &str) -> Option<Rgb8>;   // 复用 parse_hex_rgb
    pub fn format_color(color: Rgb8) -> String;
}

pub struct TerminalThemeColors {
    pub background: Rgb8, pub foreground: Rgb8,
    pub cursor: Option<Rgb8>, pub cursor_text: Option<Rgb8>, pub cursor_stroke: Option<Rgb8>,
    pub selection_foreground: Option<Rgb8>, pub selection_background: Option<Rgb8>,
    pub palette: IndexedPalette,
}
/// 完整 xterm 256 色表:前 16 = ANSI 镜像;16..231 色立方 / 232..255 灰阶按
/// xterm 常量生成,测试锁定。set/get/ansi_colors/indexed_colors 照抄。
pub struct IndexedPalette { pub colors: [Rgb8; 256] }

/// UI chrome 语义色。derive 标志镜像文档 ui.derive;槽位刻意保持小集合,
/// 适配器可从稳定锚点派生 disabled/hover。终端前景背景逐字不动——唯一共享
/// 规则(from_terminal)不许多适配器各自实现背景混合。
pub struct ThemeUiColors {
    pub derive: bool,
    pub shell: Rgb8, pub background: Rgb8, pub foreground: Rgb8,
    pub muted: Rgb8, pub accent: Rgb8,
    pub selection: Rgba8, pub line: Rgba8, pub frame: Rgb8,
    pub error: Rgb8, pub success: Rgb8, pub warning: Rgb8, pub info: Rgb8,
}
impl ThemeUiColors {
    pub fn from_palette(palette: ReviewedPalette) -> Self;      // 内置 → UI 锚点
    pub fn from_terminal(terminal: &TerminalThemeColors) -> Self; // 暗色单权重(0.18);
    // pebrel 亮暗双权重(0.13/0.18)随 appearance 字段一并砍
    pub fn with_derive(self, derive: bool) -> Self;
    pub const fn selection_rgb(self) -> Rgb8;   // alpha 由后端自施加
    pub const fn line_rgb(self) -> Rgb8;
}

/// Option 字段 = 未覆盖(None 保留用户全局设置)——「显式值权威」同构复用。
pub struct ThemeTypography {
    pub font_family: Option<String>, pub font_size: Option<f32>,
    pub line_height: Option<f32>, pub letter_spacing: Option<f32>,
    pub font_weight: Option<u16>, pub ligatures: bool,
    pub ui_font_family: Option<String>, pub ui_font_size: Option<f32>,
}
pub struct ThemeLayout { /* card: ThemeCardGeometry + padding_x/y: Option<f32> */ }
pub struct ThemeEffects { /* opacity/blur Option 槽 + 校验 */ }
pub enum ThemeValidationError { /* 名称/字体/字号/行高/字距/字重/布局/透明度专属变体 */ }
```

### WCAG 对比度校验(`slterm_settings/src/custom_theme.rs`,本篇锚点,照抄)

```rust
pub fn wcag_contrast_ratio(foreground: Rgb8, background: Rgb8) -> f64;
// 相对亮度按 WCAG 2 公式:sRGB 线性化 + 0.2126/0.7152/0.0722 加权
pub fn meets_wcag_aa(foreground: Rgb8, background: Rgb8) -> bool;   // AA 普通文本 4.5:1
pub fn foreground_recommendations(background: Rgb8, original: Rgb8) -> [Rgb8; 3];
// 原色 + 冷/暖两个可读候选;原色永远排第一(用户颜色绝不静默改写,候选只供建议)
pub fn readable_tint(color: Rgb8) -> Rgb8;
fn relative_luminance(rgb: Rgb8) -> f64;   // 私有,公式注释照抄
fn mix_rgb(a: Rgb8, b: Rgb8, weight: f32) -> Rgb8;
fn blend_rgb(...);                          // 候选不达标时向白/黑较优端点逐步混合直至达标
```

消费点:主题编辑器保存路径(不达标拒绝/建议)与取色对话框推荐——主题编辑器照抄(下条)。

### 主题库应用侧(`slterm_app/src/theme_library/`,照抄)

```rust
// document.rs —— 版本化 JSON 信封;schema_version 字段 + 未知字段留在值树
// 供未来演进 + 256KB 上限;「设置层管值、应用层管格式」的分层边界件。
pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
pub struct ThemeDocument { /* name/appearance/id/revision/vendor 值树 + 定义载荷 */ }
impl ThemeDocument {
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, DocumentError>;  // 有界:超限即拒
    pub fn from_value(value: Value) -> Result<Self, DocumentError>;
    pub fn to_value(&self) -> Value;
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, DocumentError>;
    pub fn name(&self) -> &str; pub fn id(&self) -> Option<&str>;
    pub fn revision(&self) -> u64; pub fn is_builtin(&self) -> bool;
    pub fn definition(&self) -> Result<ThemeDefinition, DocumentError>;  // 值树 → 值模型
    pub fn resolve(&self) -> Result<ThemeDefinition, DocumentError>;     // derive 展开
    pub fn with_definition(&self, definition: &ThemeDefinition) -> Result<Self, DocumentError>;
    pub fn with_id_and_revision(&self, id: impl Into<String>, revision: u64) -> Self;
    pub fn with_name(&self, name: impl Into<String>) -> Result<Self, DocumentError>;
    pub fn fork(&self, ...) -> Result<Self, DocumentError>;   // 首次保存恒 fork 的载体
    pub fn color(&self, section: &str, key: &str) -> Option<Rgb8>;
}
pub fn builtin_document(theme: ThemeName) -> Result<ThemeDocument, DocumentError>;  // 种子形态
pub fn seed_document(...) -> ThemeDocument;

// store.rs —— 持久化 store;乐观并发:外部编辑过就不覆盖。
pub enum RevisionPrecondition { Absent, Exact(u64), Any, Unchanged(ThemeDocument) }
pub struct ThemeLibraryStore { /* root: PathBuf(数据目录 themes/ 子目录归 01 篇 D3) */ }
impl ThemeLibraryStore {
    pub fn list(&self) -> Result<ThemeLibrarySnapshot, StoreError>;
    pub fn load(&self, id: &str) -> Result<ThemeDocument, StoreError>;
    pub fn save(&self, doc: &ThemeDocument, precondition: RevisionPrecondition)
        -> Result<ThemeDocument, StoreError>;   // 修订号不匹配拒绝
    pub fn fork_builtin(&self, theme: ThemeName, name: &str) -> Result<ThemeDocument, StoreError>;
    pub fn import(&self, doc: ThemeDocument) -> Result<ThemeDocument, StoreError>;
    pub fn delete(&self, id: &str, expected_revision: u64) -> Result<(), StoreError>;
}
// preferences.rs —— 主题应用键(slterm_theme/custom_theme)与 theme_library 的
// 读写桥;SettingsPane::try_persist_checked 经它携带 revision 写入,防外部
// 编辑被静默覆盖(下条 GUI 签名)。

// package/ —— 主题包 ZIP:Manifest(版本化)+ Author + CheckedPackage(整包有界
// 检查,不解可信以外字段)+ export_package;分发渠道不做。
```

### GUI 设置页(`slterm_app/src/gpui_shell/settings_pane.rs` + `settings_pane/`,照抄裁剪)

```rust
pub struct SettingsPane {
    runtime: RuntimeSettings,
    selects: Vec<(Key, Entity<SelectState>, Vec<&'static str>)>,
    // 控件实体跨语言重建会丢焦点/编辑值/undo/订阅——语言切换只刷显示项与
    // placeholder,实体保留(pebrel refresh_localized_controls 纪律照抄)。
}
pub enum SettingsPaneEvent { Changed, Close, TerminalProfilesChanged /* …归 05/07 消费 */ }

impl SettingsPane {
    /// 统一持久化入口:设置页只负责摆控件,改动一律经此落盘 → 重载单一事实源
    /// 与全局 Settings → 通知宿主热应用。JSON 化后落点 = 新写通道 persist_keys;
    /// 写盘失败 = stderr 记录 + 调用方 toast,控件回滚已存值。
    fn try_persist(&mut self, updates: &[(&str, serde_json::Value)], cx: &mut Context<Self>)
        -> std::io::Result<()>;
    /// 成组偏好对照「编辑动作前观察到的字节」写入(携带 theme_library revision),
    /// 外部写入者不可被静默覆盖——主题应用路径专用。
    pub(super) fn try_persist_checked(&mut self, updates: &[(&str, serde_json::Value)],
                                      cx: &mut Context<Self>) -> std::io::Result<()>;
    fn apply_persisted_runtime(&mut self, updates: &[(&str, Value)], cx: &mut Context<Self>);
    // RuntimeSettings::load → resolve_theme_name(仅暗色) → Settings::load_with_runtime
    // → gpui_component::set_locale → cx.set_global → emit Changed
}

// navigation.rs —— 搜索是路由查找不是页面过滤:查询命中即跳到拥有该控件的节。
pub(super) const SECTION_IDS: [&str; N];            // 稳定路由,只追加不重用
pub(super) const SECTION_SEARCH_TERMS: [&str; N];   // 每节中英混合别名串(两语后收缩)
pub(super) const NAV_GROUPS: [(&str, &[usize]); M]; // 视觉分组与路由编号分离
pub(super) const HIDDEN_NAV_SECTIONS: &[usize];     // 可隐藏不删位,防路由漂移
// 节集合重排归改造节 5;常量族(SETTINGS_NAV_WIDTH/ROW_HEIGHT/SELECT_WIDTH 等)照抄。

// localization.rs —— 值/显示分离单点:稳定持久化值 → 当前语言显示文案。
pub(super) fn localized_select_labels(key: &str, values: &[&str], language: UiLanguage)
    -> Vec<SharedString>;                            // 语言切换与控件创建共用同一入口
pub(super) fn localized_input_placeholder(key: &str, language: UiLanguage) -> SharedString;
pub(super) fn provider_input_placeholders(language: UiLanguage) -> Vec<SharedString>;
// debug_assert 标签数与值数对齐;scrollback 档位格式化派生显示也收在这里。

// theme_editor.rs —— 承重语义:草稿/基线双快照 + 首次保存恒 fork。
pub struct ThemeEditor {
    draft: ThemeDefinition,        // 编辑中的值
    baseline: ThemeDefinition,     // 已持久化快照(回滚/脏判定基准)
    fork_source: Option<ThemeDocument>,
    source_is_import: bool,        // 导入文档保留 vendor 字段,保存仍是新库快照
}
// 模板切换替换 draft 不动源主题与已持久化运行时;保存与应用是两个动作;
// 取色对话框(foreground_recommendations 消费)+ 高级字段编辑(typography/
// layout/effects)照抄 theme_editor/controls.rs + color_dialog.rs + theme_advanced.rs。

// theme_transfer.rs + theme_package.rs —— 导入/导出异步事务相位机:
pub enum Phase { Idle, Picking, Inspecting, Committing, Done }
// 会话序号失效(关闭后迟到回调不复活旧事务)+ busy 期间禁关,有界异步交互形态照抄。

// scrolling.rs 设置页 —— 「预览不脏盘、提交可回滚」的控件-持久化交互范本:
// 滑条拖动 = 内存预览(cx.global_mut 即时生效不触盘);松手 = 归一化提交;
// 提交失败 = 读回已存值回滚控件 + 警告 toast;可访问性增减键走 observe 补提交。
// reset.rs 设置页 —— 恢复默认返回备份路径,UI 可告知备份在哪。
// status.rs —— 操作结果按语义成功/失败渲染,不是裸错误码直出。
// localization.rs 设置页 —— 语言下拉走 LanguagePref::ALL + native name 自显示。
```

### i18n(`slterm_app`,照抄)

```rust
// build/i18n.rs(生成器,独立合同 workspace 同编)+ build/i18n/catalog.rs:
pub fn generate(directory: &Path, output: &Path) -> Result<(), String>;
// 目录由 LanguagePref 注册表驱动遍历;合同全族照抄:
// - en-US 与 zh-CN 键集完全相同且非空,不等即 panic(构建期硬合同)
// - 每语言键集不得超出英文;命名占位符集合跨语言一致;message id 合法性 +
//   变体名碰撞检测;重复 id/空串/非字符串/嵌套与点号冲突全拒
// - source_id 反查只在所有语言同义时生成(歧义短语不借翻译)
// - 部分目录构建期回退英文(缺失键直接填英文值,运行时不做回退逻辑)——
//   机制保留;当前两语均走完整键集硬合同
// 生成物(translations.rs,include! 进 OUT_DIR):
//   enum Message { … }                     // typed ID:新代码唯一入口,缺 ID 编译失败
//   static MESSAGES: [[&str; COUNT]; LANG]; // 全部构建 profile 进程级静态分配
//   MESSAGE_COUNT / TRANSLATED_BYTES 常量(预算测试消费)
//   fn message_id(&str) / fn source_id(&str) 回查表(tr/pick 兼容层用)

// src/i18n/mod.rs(查询 API):
impl UiLanguage {
    pub const fn text(self, message: Message) -> &'static str;   // 表索引直取,零分配
    pub fn pick<'text>(self, zh_cn: &'text str, en_us: &'text str) -> &'text str;
    // 旧双语硬编码迁移桥:中英即时分支 + 歧义英文短语不反查(source_id 无命中直用原文)
    pub fn tr(self, key: &'static str) -> &'static str;          // 字符串键兼容层
    pub fn format(self, message: Message, args: &[(&str, &str)]) -> String;
    pub fn tr_args(self, key: &'static str, args: &[(&str, &str)]) -> String;
}
impl LanguagePreference {
    pub fn parse(value: &str) -> Option<Self>;        // 消费 settings LanguagePref
    pub fn resolved(self) -> UiLanguage;              // 显式缺失 → system_locale 解析 → 回英文
}
pub fn for_locale(locale: Option<&str>) -> UiLanguage;

// src/i18n/format.rs:
pub fn substitute(template: &str, args: &[(&str, &str)]) -> String;
// 参数值字面插入绝不二次解析为占位符(防注入式串扰);缺失参数原样可见;
// {{/}} 转义;未成对花括号原样保留;预分配容量。

// src/platform/locale.rs(照抄裁剪):
pub fn system_locale() -> Option<String>;   // Windows GetUserDefaultLocaleName 一手
// + POSIX(LC_ALL/LC_MESSAGES/LANG)回退;探测与渲染解耦——结果进应用状态,
// 渲染只认解析后的 UiLanguage;macOS CoreFoundation 分支砍。

// src/i18n/locale.rs(system_locale 的 i18n 侧 re-export 形态照抄)。
```

`text` 是新代码唯一入口;`tr`/`pick` 是渐进迁移入口而非继续堆双语硬编码的理由(docs/internationalization.md 纪律照抄,语言集改为 en-US + zh-CN,「Add a language」退化为「改键」流程)。机制不因语言数收缩而简化:`pick` 立即分支与 `tr`/`text` 三层全保留。

### 设置注册表(新建,`slterm_settings/src/keys.rs`)

白名单从「字面量数组」进化为「注册表驱动」的落点;pebrel `RESET_KEYS` 反射对账思想以 JSON 等价形态守住:

```rust
/// 一个设置键的注册条目 = 键名/默认/域归属/白名单与重置清单同源。
/// 新键落地 = 在本表加一行:白名单、重置覆盖、GUI 枚举、搜索别名自动入列,
/// 防「加了新设置忘了加进重置清单」的结构性漂移(等价 reset 反射对账)。
pub struct SettingsKeyDef {
    pub key: &'static str,                 // 段内 snake_case 键名(段嵌套 D06-1 已裁;段 = domain)
    pub default: serde_json::Value,
    pub domain: SettingsDomain,            // Appearance/Scrolling/Interaction/…(页归属)
    pub resettable: bool,                  // false = 用户数据/授权类显式选择,重置保留
    pub search_terms: &'static str,        // 中英别名串(设置页路由索引导出)
}
pub struct SettingsRegistry;               // 模块级单例;register/getAll/_reset 契约
pub fn registry() -> &'static SettingsRegistry;
pub fn whitelist() -> &'static [&'static str];       // 写通道校验消费(SEC-11 等价)
pub fn reset_keys() -> Vec<&'static str>;            // restore_default_settings 消费
pub fn search_index() -> Vec<(&'static str, SettingsDomain)>;  // SECTION_SEARCH_TERMS 导出
// 域模块常量引用纪律保留:bg 域归 09/02 的键(bg_tasks/pty 先例)仍经
// 域模块常量登记,禁在注册表表体二次字面量。
```

## 数据流与状态机

### 设置写通道(读-改-写全链,JSON 化后形态)

```
GUI 控件事件(滑条松手/下拉确认/开关翻转/取色器关闭)
  → SettingsPane::try_persist(updates)            // 控件不自行写盘,一律经统一入口
    → persist_keys(updates)                        // slterm_settings 写通道契约
      ├─ SETTINGS_SAVE_LOCK 持锁                  // 保存锁串行化读-合并-写全程
      │    (移植自旧 settings.rs SETTINGS_SAVE_LOCK;防并发 persist 的
      │     Windows 句柄占用 PermissionDenied,杀软扫描窗口为残余偶发源)
      ├─ read_existing_settings(path)
      │    ├─ NotFound → Null(首次启动合法,空数据合并)
      │    ├─ IO/权限错 → Err 传播(不落盘,防静默覆盖)
      │    └─ 解析失败 → Err 传播(损坏文件禁覆盖;用户确认语义归 load 侧)
      ├─ validate_settings_input(incoming)         // 顶层键 ∈ 注册表白名单(动态导出)
      ├─ merge_settings(existing, incoming)        // 段级浅合并:incoming 顶层键覆盖
      │    // 未知键在 existing 树原位保留——「未知键/用户手改不被设置页清掉」
      │    // 的语义由浅合并机械保证(txt 的行序保留在 JSON 世界的等价物)
      ├─ 序列化 pretty;> MAX_PERSIST_BYTES(1MB) → 拒绝
      ├─ NamedTempFile::new_in(app_dir) 写全 → .persist() rename(原子写)
      └─ 旧文件 copy 为 .bak(备份兜底;损坏三态的恢复源)
    → apply_persisted_runtime(updates)
      ├─ RuntimeSettings::load()(from_raw 宽容解析:未知值回默认/范围钳制)
      ├─ resolve_theme_name(runtime.theme)(仅暗色:无 follow_system 分支)
      ├─ Settings::load_with_runtime(theme, runtime) → cx.set_global(全局唯一事实源)
      ├─ gpui_component::set_locale(语言热应用;组件 locale 与自有文案分层照抄)
      └─ emit SettingsPaneEvent::Changed(宿主订阅热应用归各篇)
  └─ 失败路径:Err → stderr 记录 + toast 告警 + 控件读回已存值复位(失败可感知)
```

**损坏三态**(移植自旧 `app_dir.rs` 的 `LoadResult`,load 侧):无文件 → 默认 + 不标记;`.bak` 命中 → 备份数据 + `corrupted=true`(用户可感知警示条);原文件损坏且 .bak 可用 → 回退 + 标记。与 session(05 篇 quarantine)同纪律:读侧回退、写侧拒绝,两边不对称是刻意的。

**宽容解析流**:`RawSettings::from_json_bytes` 永不 panic——解析失败 = 空树,所有键 None → `from_raw` 全默认。逐键级:类型不符 → None → 默认;枚举值未知 → `from_settings` None → 默认;数值越界 → clamp/回默认。手改文件的人写错任何一项,最坏结果 = 该项回默认,应用与其余键不受影响。

### 重置状态机(`slterm_settings::reset`,JSON 化改造)

```
restore_default_settings()
  ├─ 读现文件;无效(损坏)文件 → 直接报错,不覆盖
  ├─ 时间戳备份:before-reset-<ts>-<pid>-<seq>.bak(同 .bak 纪律)
  ├─ 删除注册表 resettable=true 的全部已知键 → 回到默认
  │    // 未知键与用户数据(含 hook 授权类显式选择:AgentHook 非默认家)
  │    // 原样保留
  └─ tmp + rename 原子写
覆盖对账(等价 pebrel reset_covers_every_runtime_settings_key 反射测试):
  from_raw 消费的键集合(keys() 遍历 + 注册表) vs registry().reset_keys()
  断言全等——键集合从 JSON 注册表单源提取,防结构性漂移
```

### 主题选择与应用流(仅暗色)

```
启动/主题键变更
  → RuntimeSettings::load → resolve_theme_name(theme)
    // 仅暗色:无 follow_system_theme 分支,无亮暗家族切换
  → ThemeName::available(退休 id 映射到继任主题,老配置不炸)
  ├─ term_theme() → 终端调色板归 02/03 渲染消费(ExactTermColors None 槽 =
  │    调用方保留用户配置/宿主默认)
  └─ reviewed_palette() → UI 语义槽 → Settings::load_with_runtime
       → GPUI token 注入(改造节 4;消费只引用槽,禁硬编码颜色)
自定义主题激活:
  custom_theme 键(theme_library id)→ store.load(id) → document.resolve()
  → ThemeDefinition → TermTheme/ThemeUiColors 双通道;IndexedPalette 256 色
  整表(前 16 = ANSI 镜像)
```

### 主题编辑与库事务流(照抄)

```
ThemeEditor(草稿/基线双快照)
  ├─ 模板切换 → 替换 draft;源主题与已持久化运行时不动
  ├─ 取色 → foreground_recommendations(原色永远第一)→ 不达标拒绝或建议候选
  ├─ 保存(首次恒 fork;Save 不可能改到既有主题或激活快照)
  │    → ThemeDefinition::validate → document.with_definition
  │    → store.save(precondition=携带基线 revision)
  │         └─ revision 不匹配 → 拒(外部编辑过就不覆盖)
  │    → preferences::save(主题应用键)——SettingsPane::try_persist_checked 路径
  └─ 应用与保存是两个动作;导入文档保留 vendor 字段但保存永远是新库快照
导入/导出(theme_transfer 相位机):
  Idle →(begin_task)→ Picking → Inspecting(CheckedPackage 有界检查)
  → Committing → Done;会话序号失效:关闭后迟到回调不复活旧事务;busy 禁关
主题包 ZIP:Manifest(版本化)+ Author + export_package;导入整包有界检查,
不解可信以外字段;分发渠道不做。
```

### i18n 构建与运行时流

```
构建期(build.rs):
  build/i18n::generate(slterm_app/i18n/, OUT_DIR/translations.rs)
    → catalog::parse 逐 JSON:键集/占位符/id 合法性/碰撞/重复/空串/嵌套点号冲突全校验
    → en-US 键集 vs zh-CN 键集 全等且非空,不等即 panic(硬合同)
    → 部分目录回退英文(缺失键填英文值)
    → 生成 Message 枚举 + MESSAGES 静态表 + message_id/source_id 回查 +
      MESSAGE_COUNT/TRANSLATED_BYTES 预算常量
    → include!(OUT_DIR/translations.rs) —— 运行时零解析、零文件读、零锁

运行时:
  LanguagePreference(settings language 键)→ resolved()
    → 显式 Some(lang) → 该语言
    → System → system_locale():GetUserDefaultLocaleName(Windows 一手)
      → POSIX 环境变量回退 → LanguagePref::from_locale 协商
      → 解析不出 → EnUs
  查询:UiLanguage::text(Message) = MESSAGES[lang][id](零分配)
    格式化:text + substitute(唯一允许 String 的翻译路径)
  UI 语言切换:SettingsPane::apply_persisted_runtime → set_locale
    → refresh_localized_controls(实体保留,只刷显示项)
```

### hooks 配置双轨流(与 03 篇缝合,spec 采纳点 37)

```
轨一(接入偏好):slterm_settings::AgentHook 段
  → 03 篇安装权威 ensure_claude_hooks 族读 enabled() 做接入/卸载决策
  → CLI setup-ai / 恢复默认经 all_updates/setup_updates 写盘(真布尔)

轨二(配置编辑):设置中心 Agent 组 hooks 页
  数据源:03 篇 integrations::inspect() → Vec<AgentIntegration>
    → HookInspection 八字段(installed/needs_repair/helper_missing/config_path…)
    → 替换旧注入三态(AgentInjectionStatus 消亡,eg 03 篇改造节 1)
  编辑域:限用户/第三方 handler;slterm marker 认领的归安装器所有,编辑器不触碰
    (03 篇 marker 语义;写入语义校验 SEC-05/SEC-17 事件名白名单 + command 型必填
    在编辑器提交路径保留)
  三层读写(user/project/local,eg 03 篇边界):slTerminal 旧 claude config.rs
    三层形态归 03 篇;本篇只锚定设置页数据源与编辑域边界
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(薄写;每项一行:pebrel 源 · 符号 + 缝合点 + 因果链一句)

| pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- |
| `nebula_settings/src/lib.rs` `RuntimeSettings`/`from_raw` | `slterm_settings` 全量改名+JSON 化 | 单一权威大结构 + 逐键宽容解析是设置模型心智核心,JSON 化只换载体不改语义 |
| `nebula_settings/src/lib.rs` 键域枚举族(`CursorShapeName` 等)+ `ALL`/`VALUES` | 同上全量 | 四件套形态杜绝散落字符串比较,GUI 枚举与写盘值单源 |
| `nebula_settings/src/lib.rs` `opacity_is_explicit`/`blur_is_explicit` + `pane_card_*` 四键 Option 模式 | 同上全量 | 「显式值权威」是设置/主题优先级体系地基,切主题不得悄悄改写用户意志 |
| `nebula_settings/src/lib.rs` `persist_keys`/`apply_updates` 读-改-写语义 | 语义对齐,机制换 slTerminal 写通道 | 未知键保留是「用户手改不被设置页清掉」的合同;行序保留随 txt 消亡,浅合并是 JSON 等价物 |
| `nebula_settings/src/lib.rs` `parse_hex_rgb`/`format_hex_rgb`/`Rgb8`/`Rgba8` | 全量 | 设置与主题共享的颜色文本记号,往返测试锁定 |
| `nebula_settings/src/lib.rs` `ThemeName`/`TermTheme`/`ExactTermColors`/`ThemeCardGeometry` + 常量族 | 全量(亮字段砍) | 主题对终端色表的全部影响聚合为显式缺省语义;白边事故的「单一真源常量」教训随迁 |
| `nebula_settings/src/language.rs` `languages!`/`LanguagePref`/`from_locale` | 全量,注册行 11→2 | 单行注册一种语言,选择器/枚举/查找表同源零分叉;locale 协商(zh Hans/Hant、地区回退)照抄 |
| `nebula_settings/src/scrolling.rs` 档位族/`normalize_scroll_speed`/`scrollback_lines` | 全量(默认值归 02 篇 D02-3) | 七档白名单 + 归一钳制,防手改出 1 行/一千万行;模式适用一切未来数值键 |
| `nebula_settings/src/reset.rs` `restore_default_settings`/`RESET_KEYS` | JSON 化 + 注册表对账 | 重置=删已知键回默认、未知键/授权保留;反射覆盖测试以注册表等价形态守住 |
| `nebula_settings/src/agent_hooks.rs` `AgentHook`/`enabled`/`all_updates`/`setup_updates` | 键名 `slterm_*` 命名,eg 03 | 每 CLI 独立键 + 全局兜底 + 默认两家 + 「显式装全写/恢复默认保选择」安装器合同归 03 消费 |
| `nebula_settings/src/ligatures.rs`/`cursor_motion.rs`/`notifications.rs` `Ligatures`/`CursorMotion`/`NotificationDuration` | 全量 | 三态主题优先/显式 opt-in/Default=不覆盖的解耦语义,默认值偏安全侧 |
| `nebula_settings/src/themes.rs` `BUILTIN`/`available`/`ReviewedPalette`/`FreshPalette`/`code_background` | 全量(亮成员/ is_light 砍) | 目录即合同:退休 id 可解析不出现;终端色表与 UI chrome 两套数据分离单源 |
| `nebula_settings/src/custom_theme.rs` `ThemeDefinition` 全家/`IndexedPalette`/`ThemeUiColors::from_terminal`/`with_*` 编辑族/`validate` | 全量(appearance 砍) | 值模型零序列化零文件格式,唯一共享 derive 规则不许多适配器各自混合背景 |
| `nebula_settings/src/custom_theme.rs` `wcag_contrast_ratio`/`meets_wcag_aa`/`foreground_recommendations`/`readable_tint` | 全量 | WCAG 2 公式 + AA 4.5:1 + 原色永远第一的推荐纪律,用户颜色绝不静默改写 |
| `nebula_app/src/theme_library/` `ThemeDocument`/`SCHEMA_VERSION`/`MAX_DOCUMENT_BYTES`/`ThemeLibraryStore`/`RevisionPrecondition`/`builtin_document`/`seed_document` | `slterm_app::theme_library` 全量 | 版本化信封 + 有界导入导出 + 乐观并发,「设置层管值、应用层管格式」单向依赖 |
| `nebula_app/src/theme_library/package/` `Manifest`/`Author`/`CheckedPackage`/`export_package` | 全量 | 主题分享载体,整包有界检查不解可信以外字段 |
| `nebula_app/src/gpui_shell/settings_pane.rs` `SettingsPane`/`try_persist`/`try_persist_checked`/`apply_persisted_runtime`/`refresh_localized_controls` | 全量,JSON 落点替换 | 设置页三纪律(预览不脏盘/提交可回滚/失败可感知)+ 控件实体跨语言保留 |
| `nebula_app/src/gpui_shell/settings_pane/navigation.rs` `SECTION_IDS`/`SECTION_SEARCH_TERMS`/`NAV_GROUPS`/`HIDDEN_NAV_SECTIONS` + 常量族 | 节集合重排归改造节 5;常量值经注册表导出 | 搜索=路由查找不是过滤;路由 id 只追加,视觉分组与编号分离 |
| `nebula_app/src/gpui_shell/settings_pane/localization.rs` `localized_select_labels`/`localized_input_placeholder`/`provider_input_placeholders` | 全量 | 值/显示分离单点:语言切换与控件创建共用同一入口,不留构造时旧语言 |
| `nebula_app/src/gpui_shell/settings_pane/theme_editor.rs` + `theme_editor/` `ThemeEditor`/`draft`/`baseline`/`fork_source`/`source_is_import` | 全量 | 草稿/基线双快照 + 首次保存恒 fork;保存与应用两个动作 |
| `nebula_app/src/gpui_shell/settings_pane/theme_transfer.rs`/`theme_package.rs` `Mode`/`Phase`/`begin_task`/`is_current`/`PackageTransfer` | 全量 | 相位机 + 会话序号失效 + busy 禁关,有界异步交互形态 |
| `nebula_app/src/gpui_shell/settings_pane/scrolling.rs` `commit_scrollback_lines`/`create_scroll_speed_slider`/`preview_scroll_speed`/`commit_scroll_speed` | 全量 | 控件-持久化交互范本:拖动=内存预览、松手=归一化提交、失败=读回回滚 + toast |
| `nebula_app/src/gpui_shell/settings_pane/reset.rs`/`status.rs`/`localization.rs` | 全量(节集合重排) | 重置备份路径可告知;结果按语义渲染;语言下拉 native name 自显示 |
| `nebula_app/build/i18n.rs` + `build/i18n/catalog.rs` `generate`/`message_variant`/`catalog::parse`/`catalog::placeholders` | `slterm_app/build/` 全量 | 构建期硬合同(键集全等/占位符一致/id 合法)panic 强制,运行时零解析零文件读零锁 |
| `nebula_app/src/i18n/mod.rs` `UiLanguage`/`LanguagePreference`/`text`/`tr`/`pick`/`format`/`tr_args`/`for_locale` | 全量 | typed ID 唯一入口缺 ID 编译失败;`pick` 桥即时分支与三层回退机制不随语言数收缩 |
| `nebula_app/src/i18n/format.rs` `substitute` | 全量 | 注入防护(参数值绝不二次解析)+ 缺失参数可见 + 转义,测试锁定 |
| `nebula_app/tests/i18n_contract.rs` `first_and_repeated_translation_lookups_allocate_nothing`/`translation_lookup_fits_a_small_stack`/`embedded_translations_stay_within_the_initial_payload_budget` | 随迁改名(预算值按两语重定,eg 开放问题 1) | 零分配合同有机器验证;64KB 小栈子进程回归隔离;payload 预算钉测试 |
| `tools/i18n-contract/` 独立 workspace(`pebrel-i18n-contract`,build.rs `#[path]` 复编) | `tools/slterm-i18n-contract`(包名 `slterm-i18n-contract`)归 12 篇打包清单登记 | 合同可脱离应用壳运行,零渲染依赖 |
| `nebula_app/src/platform/locale.rs` `system_locale`/`native_locale` | 全量(macOS 分支砍) | Windows 一手 locale + POSIX 回退;探测与渲染解耦 |
| `docs/internationalization.md` 文案规则集(Runtime contract/Checks/母语审校) | 改写归仓内文档归 12 篇;纪律本篇执行 | typed ID 纪律/桥不扩大/预算纪律/母语审校,语言集改两语后流程不变 |
| `nebula_app/i18n/en-US.json`/`zh-CN.json` | 随迁改名(`slterm_app/i18n/`) | 键集即合同本体;九语目录不迁 |

### slTerminal 侧移植资产(git 历史提取,`git show HEAD:<path>`)

| 旧栈位置 · 符号 | 落位 | 处置 |
| --- | --- | --- |
| `src-tauri/src/settings.rs` `SETTINGS_SAVE_LOCK`/`validate_settings_input`/`merge_settings`/`read_existing_settings`/`save_settings_blocking` | `slterm_app` 写通道适配层(eg 01 篇提取清单归 06) | **移植**:比 pebrel 裸 `fs::write` 更严的原子写(tempfile persist + rename)/.bak/保存锁/损坏禁覆盖/1MB 上限全保留;白名单从八键字面量进化为注册表驱动 |
| `src-tauri/src/app_dir.rs` `app_data_dir`/`LoadResult`(`data`/`corrupted`)/`MAX_PERSIST_BYTES` | 同上(路径单点归 01 篇锚) | **移植**:损坏三态载体;数据目录 = `%APPDATA%\slterm` 标准布局(D2 已裁,便携 exe 同级语义消亡,零迁移 D06-2) |
| `src/features/settingsCenter/SettingsPageRegistry.ts` `register`/`getAll(group?)`/`_reset`/惰性单例 | `gpui_shell::settings_pane::registry`(Rust 重建归 M6) | **契约保留**:注册表家族形态(硬约束 #13)跨语言重建;`SettingsPage.group` 全局/Agent 二分(ADR-0023)保留,吸收 pebrel 导航元数据(改造节 5) |
| `src/theme/schemes/types.ts` `UiTokens` 区域组(gitFile/gitGutter/explorer/sidebar/agentStatusUsage + 27 标量) | GPUI 主题 token 槽位集融合归改造节 4 | **纪律保留**:语义槽单点、消费只引用 token、禁硬编码颜色(硬约束 #6 跨语言重建) |
| `src/theme/schemes/linear.ts` + `SchemeRegistry`(`setActive` 未知回退/onDidChange 订阅/`_reset`) | 内置主题目录机制(采纳点 18)+ `resolve` 链 | **映射**:多配色方案 → 内置主题目录;注册表契约(`onDidChange` 等价物归改造节 4) |
| `src/theme/startupColors.ts` 生成物 + `sync-startup-colors.mjs` | 编译期主题常量等价物归改造节 4 | **等价物重建**:GPUI 无 CSS 变量层,fail-safe 静态色 = 编译期暗色常量 + 主题就绪前不渲染 |
| `src/types/hooksConfigGui.ts` `HooksConfigGui`/`HookEventGroup`/`HookMatcherGroup` 五型 handler 矩阵 | 设置中心 Agent 组 hooks 页编辑归 03 篇缝合(spec 采纳点 37) | **数据源换代**:自研注入三态 → `HookInspection` 八字段;编辑域限用户/第三方 handler;SEC-05/SEC-17 校验在提交路径保留 |

### 改名映射引用

本篇一切改名以 01 篇「改造 / 移植 / 新建设计」节的**改名映射单点表**为唯一权威,本篇不另立映射;直接消费点:`nebula-settings` → `slterm-settings`(A 节);`pebrel_settings.txt`/`nebula_settings.txt` → `settings.json`(D 节,JSON 化已定);`pebrel-theme.json`(ThemeFormat::Pebrel)→ `slterm-theme.json`(D 节,pebrel 格式导入兼容本篇裁定不保留,见改造节 8);`SLTERM_CONFIG_DIR`(C 节);品牌串 `Pebrel`/`Nebula` → `slTerminal`(P-5,i18n 文案与关于页消费);线程/管道/环境变量归 03/04 篇。本篇域内新增映射项:无(设置键名 `slterm_ai_hooks_*` 是键域值不是品牌映射,挂 agent_hooks 段,段嵌套 D06-1 已裁)。

### 缝合点

1. **01 篇(基线)**:`settings_dir`/`settings_path` 路径唯一权威归 01 篇锚定,本篇只消费;数据目录叶子名(D2 = `slterm`)与文件命名去品牌化(D3)直接决定本篇文件落点与 theme_library 落盘文件名(开放问题 2);禁名门禁对 `docs/pebrel-design/` 临时稿的豁免 01 篇已登记,本篇引 pebrel 符号属「来源」语境。
2. **02 篇(终端)**:`conpty_input_modes` 键域归本篇锚定(02 篇登记),键名经 02 域模块常量引用防双源漂移;终端调色板消费 `TermTheme`/`ExactTermColors`(02 篇「壳调色板解析 eg 06」登记);`scrollback_lines` 默认值终值归 02 篇 D02-3;CJK 粗体/字宽模式键归本篇枚举族,消费归 02。
3. **03 篇(AI CLI)**:`AgentHook` 偏好段与 `slterm_ai_hooks` 全局键本篇锚定、03 篇安装权威消费;设置页 hooks 组数据源 = `integrations::inspect()` 八字段(03 篇给出);`hooksConfigGui` 编辑域与 marker 认领组边界归 03 篇 marker 语义;SEC-12 审查门 UI(展示 Suspended 原文归 06 设置页,审查逻辑归 03)。
4. **04 篇(Runtime API)**:settings 只读快照归 runtime 方法族消费归 04;`windowing_behavior` 键已裁删除(B.10,单窗行为硬编码归 09 篇);设置写操作经 UI owner 线程执行的热应用链与 runtime 写路径的互斥归 04 篇定。
5. **05 篇(布局)**:`WorkspaceTab::Settings` 单例 tab 的渲染分支与关闭语义归本篇(05 篇只锚变体);侧栏 collapsed/width/active_view 已裁归 session(D05-3),不进本篇键域;非聚焦 pane veil 的主题开关归本篇键域(05 篇开放问题 1 默认);`TabMeta.color: Option<Rgb>` 消费本篇 `Rgb` 锚点。
6. **07 篇(文件/编辑)**:`keybind` JSON 段归 07 篇锚定结构(本篇只登记格式);编辑器主题槽(editorThemeSlot 的 Rust 对应物)消费主题 token 集归 07 篇装配;设置页中编辑器相关键位行归 07。
7. **09 篇(系统)**:托盘/静默启动/自更新/开机启动键本篇锚定、09 消费;`notification_duration` Default=不覆盖归 09 漏斗消费;`opacity` 标量与 `blur` 五材质枚举(默认 None)本篇已锚定(D06-4/D09-1),窗口特效渲染消费归 09;壁纸 `background_image_*` 五字段归 09 渲染消费。
8. **10 篇(安全)**:设置页供应商/providers 组归 10 篇凭据域,不进本篇页集合;设置文件与主题文档的凭据红线(SEC-18)本篇执行——schema 无 token 字段。
9. **11 篇(测试)**:本篇全部测试点的虚拟窗口/夹具/门禁登记归 11;i18n 合同测试的独立 workspace 形态归本篇(上表已落);豁免登记归 11。
10. **12 篇(打包)**:`tools/slterm-i18n-contract` 包名与 Cargo 元数据归 12 篇清单;release notes 双语形态归 12 消费 i18n 键。

## 改造 / 移植 / 新建设计

### 1. 设置持久化 JSON 化改造(pebrel txt/Lua → JSON 的序列化设计)

裁定:「语义照抄、载体替换」(spec 分片已定)。落地四步:

- **值层不动**:`RuntimeSettings`/`from_raw` 逐字段语义(未知回默认/钳制范围/None=未设置)原样保留——JSON 化改的是 `RawSettings` 的读取载体与写通道的合并机制,不改权威大结构一个字段语义。
- **宽容解析平移**:pebrel txt 宽容(大小写归一/布尔八拼写/行序保留)是文本格式的补偿机制,JSON 世界按其服务对象逐项重定(对照表见「关键类型与签名」`RawSettings` 条):真正要保留的服务对象是「手改文件的人写错不炸」,由「解析失败不 panic + 逐键类型宽容 + 未知键保留」覆盖;可牺牲的补偿(八拼写/行序)随载体消亡。
- **写盘值形态布尔化**:pebrel GUI 写盘 `"1"`/`"0"` 字符串,JSON 世界写 `Value::Bool`——设置页落 JSON 真布尔(spec 采纳点 23);`AgentHook::all_updates` 等授权路径同步改布尔。
- **键位绑定段归 07**:pebrel `keybind_pairs` 多行 txt 协议不迁,JSON 世界 = `keybind` 顶层键下数组(归 07 篇锚定结构);本篇只登记 `persist_keybinds` 整表替换语义(删旧数组、按序追加)由 07 写通道消费。

### 2. 写通道底座并入(slTerminal settings.rs 移植)+ 零迁移口径(D06-2 已裁)

**并入点**:pebrel `persist_keys` 的裸 `fs::write` 全部替换为移植资产 `save_settings_blocking` 全链(校验 → 浅合并 → 原子写 → .bak),落 `slterm_app` 适配层(`slterm_settings` 保持零生产依赖:写通道硬机制在 app 侧,persist_keys 以函数注入或 app 实现 settings trait 的形态接线——接线形态归 M6.1 实现期,契约不变)。

**白名单进化**:`SETTINGS_ALLOWED_KEYS` 八键字面量数组 → `keys.rs` 注册表 `whitelist()` 动态导出;SEC-11 语义保留(未知顶层键拒绝),「键名经域模块常量引用防双源漂移」(`background_tasks::SETTINGS_KEY`/`pty::spawn::SETTINGS_KEY` 先例)继续有效——注册表条目引用域模块常量,表体禁二次字面量。

**零迁移(D06-2 已裁)**:新版只读写 `%APPDATA%\slterm` 下的 `settings.json`;旧版 exe 同级便携文件不看不迁,不做探测、不做哨兵、不做映射——旧文件存在与否行为不变。唯一例外 = `projects.json` 按 B.20 提取(归 05 篇消费)。pebrel `paths/migration.rs` 防御性形态无服务对象,不迁。

### 3. 仅暗色裁剪清单(亮主题全套消亡登记)

| 消亡项(pebrel 位置) | 消亡语境 | 新世界对应物 |
| --- | --- | --- |
| `custom_theme.rs` `ThemeAppearance` 枚举 + `from_terminal` 亮暗双权重(0.13/0.18) | 仅暗色定位 | 暗色单权重 0.18 常量;selection 无显式声明时 `mix_rgb(background, foreground, 0.18)` |
| `lib.rs` `LIGHT_FOREGROUND`/`LIGHT_ANSI` 浅色替换表与「浅色主题替换前景/ANSI-16」消费语义 | 没有亮主题就没有替换对象 | 无;`ExactTermColors` 整表声明的暗色主题不经过任何替换 |
| `lib.rs` `RuntimeSettings.follow_system_theme` + `resolve_theme_name` 亮暗家族切换分支 | 无可跟随外观 | 单参数 `resolve_theme_name(theme)` |
| `TermTheme.is_light` 字段 + `themes.rs` 亮成员 `ReviewedPalette` | 模型只带暗色语义 | 字段不迁;亮主题成员不进 `BUILTIN` |
| `settings_pane` 亮主题预览/切换 UI 分支 | 同上 | 主题选择器只列暗成员 |
| 机制若未来重启(产品定位修订) | — | 从 git 历史(baseline + pebrel 演进)取回,不回填兼容层 |

### 4. 主题 token 机制跨语言重建(配色单点纪律的 Rust 形态)

**槽位集融合**:GPUI 主题 token = pebrel `ReviewedPalette` 语义槽(骨:shell/background/foreground/muted/accent/selected/line/语义六色/frame)× slTerminal 旧 `UiTokens` 区域组(肉:gitFile/gitGutter/explorer/sidebar/agentStatusUsage 区域组 + 27 标量)融合。融合纪律:

- 语义六色(error/success/warning/info 归 `ThemeUiColors`;red/green/yellow/blue/purple/cyan 语义槽)与区域组(git gutter/explorer 等 07 篇文件域消费)分层:**槽位值单点定义于主题,区域组是消费侧命名视图,不复制值**。
- 旧 `UiTokens` 中纯表现派生色(hover/disabled/border 变体)从稳定锚点派生——「适配器可派生、不合成第二色带」原则照抄 `ThemeUiColors` 模块注释。
- xterm/CM6/CSS 变量三层适配器(Dockview/Allotment overrides)整面消亡:GPUI 消费直取语义槽,无中间记号层;终端调色板 eg 02 渲染消费 `TermTheme`。

**内置主题目录 = 多配色方案注册表**:`ThemeName::BUILTIN` 即 slTerminal 旧 `SchemeRegistry` 多方案的对应物——「目录即合同,注册即入列」(采纳点 18);`setActive`/`onDidChange` 的 Rust 等价物 = `Settings::load_with_runtime` 热应用链 + `SettingsPaneEvent::Changed` 订阅(消费方归各篇)。旧 `SchemeRegistry` 的「未知 id 回退 linear」语义由 `ThemeName::available` 退休映射 + `from_settings` 未知回默认双保险承接。

**fail-safe 静态色等价物**:旧栈 `startupColors.ts` 生成物(启动链超时错误页三槽位,防 facade 求值前裸奔)→ GPUI 世界 = 编译期暗色常量(`pub const STARTUP_*: Rgb8`,值锚定默认主题)置于 settings crate(零依赖,启动链最先可引);「主题就绪前不渲染」由 GPUI 壳启动序列保证(应用态持主题后才首帧)。

**纪律机械保障**:「组件只引用 token、禁硬编码颜色」从 TS 纪律升级为跨语言架构约定——消费测试守卫(渲染代码 grep 禁裸 `hsla(`/`rgb(` 字面量,eg 11 篇门禁登记形态)+ 主题快照逐字节测试;常量定义只许出现在 settings crate 与内置主题目录。

### 5. GUI 设置页融合(设置中心注册表 × pebrel 导航形态)

**注册表吸收导航元数据**:`SettingsPage` 条目(Rust 重建归 M6)从 `{id, title, group, order}` 扩为吸收 pebrel 导航三要素——稳定路由 id(只追加不重用)+ 双语搜索别名串 + 视觉分组;`search_index()` 从注册表导出,`SECTION_SEARCH_TERMS` 不再是独立字面量数组(与 `RESET_KEYS` 同法进化)。旧 `dirtyRegistry` 的 dirty 真值源语义由 ThemeEditor draft/baseline 脏判定 + 各页 try_persist 回滚语义承接(GPUI 单进程无跨 store 面,归壳内组件态)。

**页集合重排**(spec 优化方向):全局组照抄落地 application/appearance/profiles/interaction/advanced/localization/reset 节;ssh/network 节随 SSH 砍;providers 节归 10 篇凭据域;agents 节(eg 03 hooks 页数据源);keymap 归 07;mobile/backup 裁;about/sponsor 节归 12 篇品牌位。节集合终表归 M6.2 编码期随 i18n 键集一次定(键集即合同)。

**全局/Agent 二分保留**(ADR-0023):导航组序全局在上 Agent 在下;Agent 组内按 `AgentKind`(03 篇锚点)分节,分节标题 = logo + display_name——旧 `syncAgentPagesFromProfiles` 从 cliProfileRegistry 同步页的形态,替换为 27 家 `AgentKind::ALL` 单源驱动(身份事实单源归 03)。

### 6. i18n 两语落地(旧中文硬编码迁移路径)

- **构建管线**:`slterm_app/i18n/{en-US,zh-CN}.json` → build.rs `generate`(目录由 `LanguagePref` 注册表驱动)→ `translations.rs` 生成物;独立合同 workspace `tools/slterm-i18n-contract` 经 `#[path]` 复编 `build/i18n.rs` + `catalog.rs` + 查询核心归 `slterm-settings`(LanguagePref)+ 本地 crate,零 GPUI 依赖可独立 `cargo test`。
- **旧栈文案迁移三阶段**(internationalization.md 纪律照抄,桥不扩大):存量全中文硬编码 → `pick(中文, 英文)` 桥(渐进,歧义短语不反查)→ `tr("stable.id")` 键化 → 新代码直写 `Message::Variant` typed ID。三阶段共存期允许,新代码只许 typed ID。
- **System 档文案**:`LanguagePreference::resolved` 链(系统 locale → from_locale → 回英文);语言设置页 System 选项文案经 `tr`(native name 自显示)。
- **payload 预算**:`TRANSLATED_BYTES` 预算上限 + `MESSAGE_COUNT` 下限钉测试随迁;预算值按两语实际重定(eg 开放问题 1)。

### 7. hooks 配置双轨归位(与 03 篇缝合,spec 采纳点 37 落地)

- **轨一偏好段**:`AgentHook` + `slterm_ai_hooks` 全局键进设置注册表(resettable=false——授权类显式选择重置保留,与 pebrel RESET_KEYS 语义一致);03 篇安装权威/CLI 消费。
- **轨二编辑页**:设置中心 Agent 组 hooks 页数据源 = `Vec<AgentIntegration>`(03 篇 `integrations::inspect()`);编辑域限用户/第三方 handler,slterm marker 认领组归安装器专属(编辑器不触碰——双轨不交叉的判据 = marker 归属);写入校验(SEC-05 事件名白名单/SEC-17 user 层写入审计归 10)在编辑器提交路径保留。
- **旧 `hooksConfigGui` 五型 handler 矩阵**保留为编辑页 GUI 模型(值模型归前端遗产,GPUI 重建归 M6.5 与 03 联调归位);旧注入三态展示消亡(数据源换代)。

### 8. `ThemeFormat::Pebrel` 导入兼容裁定(回应 01 篇 D 节挂账)

**裁定:不保留 pebrel 主题格式导入**。slTerminal 是全新 fork,无 pebrel 历史用户的主题库可迁;主题分享载体走自有 `slterm-theme.json`(`SCHEMA_VERSION` 1 信封归 theme_library)。01 篇禁名门禁无需为此登记 `PROTOCOL_COMPATIBILITY_PATTERNS` 形态豁免;若未来出现「从 pebrel 导入」真实诉求(产品定位修订),按增量评审从 pebrel git 历史取回导入适配器归 `theme_library/formats.rs`。

## 测试点清单

> 引测试一律例名;测试基础设施形态(虚拟窗口/夹具/门禁登记)归 11 篇。pebrel 侧承重用例全量随迁改名(用例名照抄),替换 slTerminal 旧栈散点测试;bugfix 防复发纪律按本仓测试金字塔归位。

| 测试 | 层级 | 机制 |
| --- | --- | --- |
| 设置读写往返:每键写盘值 → 重载 → 字段级相等(枚举族 `from_settings`↔`settings_value` 全对) | L1 slterm_settings | 迁移 pebrel 枚举往返用例 + JSON 载体改写 |
| 宽容解析族:未知值回默认/范围钳制(字号)/None=未设置/解析失败不 panic 全默认/类型不符单项回默认 | L1 | 迁移语义新建 |
| 写通道:读-改-写浅合并保留未知键与用户手改段/损坏文件禁覆盖(Err 不落盘)/1MB 上限拒绝/白名单拒未知顶层键 | L1(临时目录隔离) | 移植旧 `save_settings_blocking` 语义用例 |
| 原子写:崩在半写的 settings.json 落盘不可见(tempfile + rename)+ .bak 兜底 + 保存锁串行化并发 persist | L1(临时目录) | 移植旧 SPE-06/CP-005 用例语义 |
| 损坏三态:无文件(默认不标记)/原损坏 .bak 可用(回退 + corrupted)/双损(默认 + corrupted) | L1 | 移植旧 LoadResult 用例语义 |
| 注册表对账:whitelist() ⊇ from_raw 消费键集;reset_keys() == 注册表 resettable 集合;新增键忘登记 = 对账红 | L1 keys | 等价 pebrel `reset_covers_every_runtime_settings_key` |
| 「显式值权威」:opacity 显式 1.0 切主题后仍 1.0/未显式时主题默认可填充/pane_card 四键 None 跟主题走 | L1 | 迁移语义新建 |
| scrolling 档位:七档白名单边界(档外回默认)/滚速步进归一/非有限值回 1.0 | L1 | 迁移 pebrel 用例 |
| `AgentHook` 合同:每键独立/全局兜底/默认两家/all_updates 写全部/setup_updates 保可选家选择 | L1 | 迁移 pebrel `legacy_global_setting_and_individual_overrides_round_trip` 等三例 |
| `LanguagePref` 往返 + `from_locale` 协商表(POSIX 后缀/zh 区域/en-GB 回退)裁三家后全例 | L1 | 迁移 pebrel `system_locale_negotiates_regions_scripts_and_posix_suffixes` 改写 |
| 颜色文本往返:`parse_hex_rgb`/`format_hex_rgb`(#rgb/大写/中文/非法长度全族) | L1 | 迁移 |
| `IndexedPalette` 256 色立方/灰阶逐字节 + ANSI 镜像一致性 | L1 custom_theme | 迁移 |
| 主题快照:`ThemeDefinition::from_builtin` 逐字节/`with_*` 编辑族不可变返回副本/validate 全错误变体 | L1 | 迁移 |
| WCAG:`wcag_contrast_ratio` 公式锚点值(黑/白 21:1)/`meets_wcag_aa` 4.5:1 边界/`foreground_recommendations` 原色永远第一 + 多背景下候选全达标 | L1 | 迁移 |
| `ThemeUiColors::from_terminal` 暗色单权重派生(终端前景背景逐字不动/锚点派生表) | L1 | 迁移改暗色单权重 |
| 内置主题目录:退休 id 映射继任(老配置可解析)/`BUILTIN` 只含暗成员(pebrel 暗色子集 + linear 默认,D06-3 已裁)/BUILTIN_NAMES 与持久化名一致 | L1 themes | 迁移 + 新建 |
| theme_library:信封版本不符拒收/256KB 上限/未知字段留值树往返/`RevisionPrecondition` 四态(外部编辑拒写)/fork 首次保存恒 fork | L1 theme_library | 迁移 |
| 主题包 ZIP:Manifest 版本化/整包有界检查/不解可信以外字段 | L1 | 迁移 |
| 零迁移断言（D06-2)：旧 exe 同级 settings.json 存在/不存在/损坏三态下，新版读写行为不变（只认 `%APPDATA%\slterm`) | L1（临时目录） | 新建（防复发：旧便携文件被误读的回归闸） |
| 设置页三纪律:滑条拖动不触盘(预览纯内存)/松手归一化提交/写失败读回已存值复位 + toast | L3 虚拟窗口(归 11 基础设施) | 迁移 pebrel scrolling 页用例语义 |
| 搜索路由:中英别名命中跳节(归注册表 search_index 导出)/路由 id 只追加不重用/HIDDEN 节不删位 | L1 navigation | 迁移 + 注册表接线 |
| `localized_select_labels`:值/显示分离/语言切换刷新/标签数与值数对齐 debug_assert | L1 | 迁移 |
| ThemeEditor:草稿/基线双快照/模板切换不动已持久化/首次保存恒 fork/导入保留 vendor 但存新快照 | L1 | 迁移 |
| theme_transfer 相位机:迟到回调不复活/ busy 禁关/五相位流转 | L1 | 迁移 |
| i18n 硬合同:en/zh 键集全等(生成期 panic 反向用例)/占位符跨语言一致/id 碰撞/重复/空串/嵌套点号冲突/部分目录回退英文 | L1 生成器单测 + 构建期 panic | 迁移 pebrel `build/i18n.rs` 内嵌五例 |
| i18n 查询:`text` 零分配(全局分配器计数)/`tr` 三层回退/`pick` 即时分支 + 歧义短语不反查/`substitute` 注入防护(参数值不二次解析)/缺失参数可见/转义 | L1 + 合同 workspace | 迁移 |
| 合同三钉:零分配首查 + 重复查/64KB 小栈子进程查词/payload 预算 + 消息条数下限 | L1 合同 workspace | 迁移 pebrel `i18n_contract.rs` 三例(预算值重定归开放问题 1) |
| `system_locale`:Windows 一手探测(mock 注入)+ POSIX 回退序(LC_ALL → LC_MESSAGES → LANG) | L1 platform | 迁移 |
| hooks 双轨:`AgentHook` 段写盘 → 03 安装权威读决策联调(预置偏好下接入/卸载路径)/编辑域 marker 组不触碰 | L1(与 03 篇联调用例) | 新建归 M6.5 |
| 配色单点守卫:渲染代码禁裸颜色字面量(grep 归 11 门禁登记)/主题快照逐字节 | L1 + 门禁归 01/11 | 新建 |
| UI 关键路径:设置页打开 → 改键 → 热应用 → 重开面板值持久(L3 虚拟窗口,eg 11) | L3 | 新建 |

## 阶段归属与出口标准

本片跨 **M1**(crate 首迁)与 **M6**(点亮),细分步与可机验出口(对齐 00-roadmap):

**M1 段 · `slterm_settings` 迁入**(00-roadmap 依赖序首 crate,eg 01 篇):值模型/枚举族/宽容解析/语言注册表/主题值模型/WCAG 纯函数/reset JSON 化形态随 crate 一次落地(裁剪 + 改名 + JSON 契约一次做净,不迁 txt 读写)。出口 = `cargo test` 全绿(值模型/枚举往返/宽容解析/档位归一/WCAG/256 色表/语言协商用例);零生产依赖契约校验过;禁名门禁过。**写通道硬机制(.bak/保存锁/三态)与 GUI 此期不点亮,M1 期 persist 用最小原子写过渡属库态可接受**。

**M6.1 写通道底座并入 + 段嵌套落地**：前置项 = 段嵌套（D06-1）的键→段映射表与白名单/RESET_KEYS/键域枚举三清单的段形态重构先行产出（含 07 keybind、02 conpty_input_modes 两段归属回登），再落写通道。移植 settings.rs 全链 + 注册表白名单；零迁移断言（D06-2)：旧 exe 同级 settings.json 存在与否，读写行为不变。出口 = 写通道/三态/原子写/注册表对账/零迁移断言用例全绿；`cargo test` 全绿。

**M6.2 GUI 设置页点亮**(依赖 M3 壳):SettingsPane + 注册表吸收导航 + 页集合重排落地(全局组先亮,Agent 组依赖 03 篇 inspect 联调归 M6.5)。出口 = 三纪律/搜索路由/值显示分离用例全绿;L3 设置页关键路径过;`cargo test` 全绿。

**M6.3 主题系统点亮**:内置暗色目录 + theme_library store + ThemeEditor + WCAG 保存路径 + 主题包 ZIP。出口 = 主题快照/WCAG 推荐达标/store 并发/包检查用例全绿;L3 主题编辑关键路径过。

**M6.4 i18n 静态生成点亮**:build.rs 管线 + 两语硬合同 + 查询 API + locale 探测 + 语言设置页。出口 = 生成器合同族/查询零分配/合同三钉用例全绿(独立 workspace 可独立跑);`cargo test` 全绿。

**M6.5 双轨缝合 + 收口**(依赖 04 壳 + 03 篇 inspect):hooks 页数据源接线 + 全链回归。出口 = 双轨联调用例绿;`cargo check`/`cargo test`/门禁三件套全绿——对齐 00-roadmap M6 出口(设置读写往返、WCAG 对比度校验、i18n 键集硬合同测试过)。

M6 全程按 00-roadmap 绿灯语义:功能缺失不拦出口,只认可机验项。

## 待沉淀决策

> [待沉淀] **D06-1 · settings.json 键形态:平铺 snake_case vs 段嵌套**。真实权衡:段嵌套 = 语义分组清晰(appearance/scrolling/agent_hooks 各成段)、schema 演进按段隔离、读-改-写按段浅合并;平铺 = pebrel 键名映射零翻译、迁移表最薄、宽容解析逐键独立、与 `RESET_KEYS`/白名单清单形态天然一致(三清单都是平铺键集合)。意外因素:01 篇 D2/D3 数据目录与文件命名未定,键形态与其共同决定迁移映射表形态;`keybind`/`conpty_input_modes` 两域外段归 07/02 锚定,段嵌套会引入跨篇段归属协议。难逆点:键形态即磁盘格式,发布后改 = 数据搬迁义务(pebrel 自身迁移链即前车之鉴)。截止:M6.1 写通道落地前定(本篇示例与签名为行文方便按平铺展示,可被本决策推翻)。

> [待沉淀] **D06-2 · 便携化旧 settings.json(exe 同级)与新数据目录的并存/迁移策略**。真实权衡:slTerminal 0.2.0 历史用户的便携语义(单目录带走)与 01 篇默认 `%APPDATA%\slTerminal` 标准布局冲突;双位置探测并存 = 永久双源,单位置强迁 = 便携用户感知破坏。意外因素:01 篇 D2 叶子名未定;旧栈 exe 同级写在杀软实时扫描窗口下 .bak/persist 有偶发摩擦史;迁移哨兵与 .migrated 备份的留存策略同一裁定。难逆点:并存探测一旦发布即成兼容义务。截止:M6.1 迁移落地前定(依赖 01 篇 D2 先定;默认:新启动只写新位置,首启一次性迁移旧文件,旧文件改 .migrated 留存归 D06-2 终裁)。

> [待沉淀] **D06-3 · 内置主题集合终选**。真实权衡:照抄 pebrel 暗色子集(Catppuccin Mocha/Frappe/Macchiato、Nord、GlassDark、BreezeDark、MintDark、SteelDark→继任…) = 现成 16 套去亮即得、主题包生态兼容;对齐 slTerminal linear 单方案 = 产品面一致、目录小、移植线性方案的 UiTokens 区域组映射工作量大头。意外因素:仅暗色裁定砍掉 8 家亮主题后目录规模减半;「linear」是否进目录或以「继任映射」身份出现直接决定迁移映射表;退休映射(available)对旧 id 的承接集合随之定。难逆点:主题 id 发布后用户主题文档/主题包引用之,删主题 = 迁移义务。截止:M6.3 内置目录编码前定(默认:pebrel 暗色子集为底 + linear 作为默认主题入目录,终表实现期评审)。

> [待沉淀] **D06-4 · `blur`/`opacity` 键值域与 09 篇窗口特效的形态对齐**。真实权衡:pebrel `BlurModeName` 五档材质枚举随不采纳点 6 不迁,但 `RuntimeSettings.blur`/`opacity` 字段与「显式值权威」模式必须在本篇冻结类型;值域取 bool/透明度标量/新材质枚举是 09 篇窗口特效的产品裁定,错配 = M6 字段类型返工或 M9 数据搬迁。意外因素:09 篇 Mica/Acrylic 材质链(归 09 照抄)与「按 DWM 每帧成本排序、默认档性能安全」注释纪律预示值域非平凡;壁纸四键同属 09 渲染域但值模型已在篇内锚定无此问题。难逆点:字段类型进 `RuntimeSettings` 锚点签名与测试字面量,改类型 = 全键用例重写。截止:M6.1 字段类型冻结前与 09 篇对齐(默认:bool 占位 + 显式权威位,材质枚举归 09 落地时重域)。

## 开放问题

1. **i18n payload 预算值与 `MESSAGE_COUNT` 下限**:pebrel 值按 11 语标定,两语实际 payload 约为其零头;预算钉测试随迁但数值实现期按两语实测重定(eg M6.4 生成首版后回填,测试注释锚定「负载裕度,非断言放宽」纪律不适用——此为契约数值本身)。
2. **theme_library 落盘文件名/目录形态**:store root 用 `themes/` 子目录还是单文件库,文件名是否带品牌归 01 篇 D3 波及面;本篇锚 store 语义不锚文件名,eg M6.3 随 D3 终裁落地。
3. **搜索别名无机械强制**:新设置键的搜索别名串靠注册表条目自觉登记,无 grep 门禁可强制「每个键至少一条别名」;是否加消费测试(whitelist × search_index 交集非空归 11 篇门禁登记)归 M6.2 实现期。
4. **hooksConfigGui 五型 handler 矩阵的 GPUI 重建粒度**:旧前端 GUI 模型的 GPUI 控件重建归 M6.5 与 03 篇联调;「编辑域限用户/第三方 handler」的判定函数归属(installation 侧归 03 还是编辑器侧归 06)归 M6.5 按改动面最小定,两处测试锚等价)。
5. **自定义主题与内置主题同名/id 冲突**:用户主题 name 与内置主题重名时选择器展示与 `custom_theme` 键解析的消歧规则,pebrel 以「自定义主题激活键独立(custom_theme)+ 库 id 命名空间」隐含处理;显式消歧规则归 M6.3 实现期,本篇锚「保存恒 fork 不产生覆盖」底线。
