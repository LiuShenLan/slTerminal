# 主题与国际化

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 拥有语义 token 目录、内置暗色主题集合与 Linear 默认、密度/透明度/壁纸/窗口材质，以及界面语言 System/English/Chinese 与 typed i18n 合同；组件只消费 token 与 `Message`。界面语言属于迁移能力，本文件服务于迁移实现，featureSpec 只定义应用页入口与保存纪律。

### 决策与红线

- **仅暗色、内置目录**：主题目录只含暗色成员，`Linear` 为默认；未知或退休标识回退 `Linear`。原因：产品定位仅暗色，用户自定义主题与浅色主题会扩大校验面。边界：不提供运行期新增、导入或编辑主题。
- **组件零硬编码**：颜色、间距、字号、圆角、动效与终端 ANSI 色只从 `ThemeTokens` 读取；组件不得写颜色字面量或自行混合派生色。原因：硬编码会绕过主题切换与主题 token 单点。边界：绘制指令内部的数学常量（抗锯齿、贝塞尔控制点）不属于 UI 样式。
- **主题只保证暗色可读**：内置主题与透明度、壁纸、密度组合以文字可读为优先；不做对比度校验、不提示、不因对比度拒绝主题或自动回退，不静默改写设置。边界：用户显式选择即接受结果。
- **界面语言是设置而非构建变体**：同一二进制支持 `LanguagePreference::{System, English, Chinese}`，存储值 `system`/`en-US`/`zh-CN`，用户标签 `System`/`English`/`简体中文`；语言切换立即重绘，不重启。原因：单一发布产物避免双包漂移。边界：安装器语言由 Inno Setup 独立管理。
- **翻译键集是编译期合同**：语言目录键集必须完全一致；缺键、多余键、未知占位符在构建期失败。原因：运行期缺键会回退英文或显示键名，属于可见缺陷。边界：新增语言需同时提交目录与生成器枚举。
- **格式化只做一次替换**：`{name}` 占位符按同名参数替换，参数值中的占位符形态按字面文本输出，绝不二次解析。原因：防止用户输入或错误消息被再次展开。边界：缺失参数保留占位符可见，便于发现未传参。

### 语义 token 目录

```rust
pub struct ThemeTokens {
    pub color: ColorTokens,
    pub space: SpaceTokens,
    pub typography: TypographyTokens,
    pub radius: RadiusTokens,
    pub motion: MotionTokens,
    pub terminal: TerminalPalette,
}
```

颜色 token（语义槽，不是色值）：

| 组 | token | 用途 |
|---|---|---|
| 表面 | `canvas`、`surface`、`raised`、`overlay` | 窗口底、内容面、抬升面、浮层底 |
| 壳 | `shell`、`titlebar`、`activitybar`、`sidebar` | 窗口壳、自绘标题栏、活动栏、侧栏 |
| 面板 | `panel`、`editor`、`terminal` | 面板容器、编辑器底、终端底 |
| 交互 | `hover`、`selected`、`active`、`focus_ring` | 悬停、选中、激活、焦点环 |
| 文本 | `text_primary`、`text_secondary`、`text_muted`、`text_disabled`、`text_inverse`、`link` | 正文、次要、弱化、禁用、反色、链接 |
| 边框 | `border_subtle`、`border_default`、`border_strong` | 发丝线、常规边框、强调边框 |
| 强调 | `accent`、`accent_hover`、`accent_muted` | 主强调、悬停、弱强调 |
| 状态 | `success`、`warning`、`error`、`info` 及各自 `_surface` 变体 | 状态文字、图标、横幅底 |
| 终端 | `ansi[16]`、`cursor`、`cursor_text`、`selection_fg`、`selection_bg` | 终端 ANSI 与光标/选区 |

尺寸与排版 token：

```rust
pub struct SpaceTokens { pub xxs: Px; pub xs: Px; pub sm: Px; pub md: Px; pub lg: Px; pub xl: Px; }
pub struct TypographyTokens {
    pub ui_family: FontFamily,
    pub mono_family: FontFamily,
    pub cjk_family: Option<FontFamily>,
    pub ui_size: Px,
    pub title_size: Px,
    pub caption_size: Px,
    pub line_height: f32,
}
pub struct RadiusTokens { pub control: Px; pub panel: Px; pub overlay: Px; }
pub struct MotionTokens { pub fast: Duration; pub normal: Duration; }
```

- 契约尺寸进入 token 或布局常量：自绘标题栏 34px、窗口控制钮 38×26px、面板组最小 120×80px、面板页签最小 80px、通知最大宽度 320px、设置中心左导航 180px。组件引用 token/常量，不重复写字面量。
- 终端 ANSI 调色板由主题单点拥有；不提供独立终端配色。终端光标固定竖线，仅闪烁开关可配。
- 编辑器语法高亮、Git 行标、Diff 增删色、终端状态圆点都映射到语义槽；不得为某组件新增孤立色值。

### 内置暗色主题与 Linear 默认

- 目录成员：`Linear`、`BreezeDark`、`MintDark`、`Nord`、`CatppuccinMocha`、`CatppuccinFrappe`、`CatppuccinMacchiato`、`GlassDark`。目录顺序即选择器顺序，`Linear` 居首并作为默认。
- 每个成员提供完整语义槽映射：`shell`、`background`、`foreground`、`muted`、`accent`、`selected`、`line`、`red`、`green`、`yellow`、`blue`、`purple`、`cyan`、`frame`，以及终端 ANSI 16 色、光标、选区。
- 主题切换即时生效并重绘；终端按新 ANSI 色重绘现有网格，不改写回滚内容。
- 退休/未知主题名回退 `Linear`，不报错、不删除设置值；选择器不显示退休成员。
- 主题选择属于应用设置，按 2 秒去抖保存；保存失败保留运行期生效值并显示“仅在本次运行中生效”。

### 主题可读性边界

- 主题色值保持暗色可读；不做对比度校验、不提示、不因对比度拒绝主题或自动回退，不静默改写设置。
- 主题选择不触发任何可读性判断；用户选择即保存并立即生效。
- 状态不能只靠颜色：错误同时有图标/文本，选中同时有底色与指示条，焦点有焦点环；禁用态必须可识别为禁用（文本 + 控件形态），不得仅靠变淡。

### 密度、透明度、壁纸与窗口材质

- 界面密度 `Compact`/`Standard`/`Comfortable`，默认 `Standard`。密度缩放间距、行高、控件高度、列表项与页签内边距；不改变终端字号/行列、不改变编辑器字号、不改变窗口最小尺寸。即时生效。
- 窗口透明度范围 80%–100%，默认 100%。只作用于壳表面（背景、侧栏、标题栏及其分隔线），不改变文字前景透明度；每次从基线 alpha 重新计算，避免滑块与异步回调层层相乘。
- 壁纸仅作为窗口背景。终端与编辑器内容区保持主题背景；承载文字的壳表面使用不透明遮罩。壁纸读取失败保留原壁纸并通知 `壁纸加载失败`；清除后恢复主题背景。
- 壁纸 fit/alignment 取值受校验；路径经路径安全边界读取，不允许设备路径与根外自动读取。
- 窗口材质 `BlurMode`：`None`（默认，性能安全）、`Mica`、`MicaAlt`、`Aero`、`Acrylic`。材质调用只在平台封装模块；系统不支持或调用失败时回退 `None` 并记录一次警告，不阻塞启动。
- 材质与透明度叠加时，先应用材质，再按透明度重算壳表面；不做对比度校验、不提示、不因合成结果拒绝材质或改写透明度。
- 减少动效设置不影响材质，但主题切换、面板切换与覆盖层动画在减少动效下取消或缩短。

### 界面语言与 typed i18n 合同

```rust
pub enum LanguagePreference { System, English, Chinese }   // 设置值 system / en-US / zh-CN

pub enum UiLanguage { EnUs, ZhCn }

pub struct MessageCatalog {
    pub en_us: &'static [(&'static str, &'static str)],
    pub zh_cn: &'static [(&'static str, &'static str)],
}

pub enum Message { /* 构建期生成，一对一映射目录键 */ }

impl UiLanguage {
    pub const fn text(self, message: Message) -> &'static str;
    pub fn format(self, message: Message, args: &[(&str, &str)]) -> String;
    pub fn plural(self, one: Message, other: Message, count: u64) -> &'static str;
}
```

- 目录文件：`i18n/en-US.json`、`i18n/zh-CN.json`，扁平键值；键名按域分段（如 `settings.app.language`、`common.cancel`）。构建脚本验证：
  - 两种语言的键集完全一致；
  - 值非空且不含未转义控制字符；
  - 占位符集合一致；同名占位符在两种语言中拼写一致；
  - 生成 `Message` 枚举与二维静态表，运行时零解析、零文件读、零锁。
- 语言协商：`System` 读取 `GetUserDefaultLocaleName`；失败回退 `LC_ALL` → `LC_MESSAGES` → `LANG`。POSIX 后缀与地区变体剥离，主语言匹配：`zh` 家族一律 `zh-CN`，`en` 家族一律 `en-US`，未知回退英文。
- 新代码只使用 `text(Message)`/`format(Message, args)`；禁止字符串键查表与双语 `if` 分支。旧字符串键兼容层在重建中删除。
- 格式化语义：`{name}` 替换同名参数；缺失参数保留 `{name}`；`{{`/`}}` 转义为字面花括号；未闭合 `{` 原样保留；参数值不再次解析。
- 复数：英文 `count == 1` 用单数消息，其余用复数；中文统一用复数消息。计数与单位由调用方格式化后作为参数传入，避免在模板中做数字格式化。
- 用户可见文案不得拼接句子片段；需要组合时使用完整模板与命名参数。路径、文件名、项目名作为参数值插入，不翻译。
- 语言切换：更新全局 `UiLanguage`，立即重绘全部界面；窗口标题、托盘提示、通知与错误文案在下次渲染/发送时使用新语言。已打开面板不重建，仅重绘。
- 应用页入口：应用页的“界面语言”项，选项 `System`、`English`、`简体中文`；`System` 下显示解析结果（如“当前：简体中文”）。切换即时生效，按 2 秒去抖保存；无活跃项目时仍可修改。
- 系统通知正文使用发送时语言；项目名、CLI 名不翻译。安装器语言独立于应用语言。

### CJK 与字体

- 字体族从系统已安装字体选择；终端与编辑器分别配置字体族与字号（8–32 整数）。当前字体不可用时回退默认等宽字体并提示。
- CJK 字体族作为回退链的一环；缺字时按字体回退链查找，不显示豆腐块作为唯一结果。字体加载失败保留上一次有效字体并提示。
- 文本测量、省略、换行、搜索高亮按字素簇处理；标题省略不得截断代理对；全角字符宽度参与终端网格计算。
- 中文输入法组合、候选窗定位与焦点切换按 `design/ux-framework.md` 的 IME 门控执行。

### 验收与测试

- 纯函数层：主题语义槽完整、密度缩放计算、主题名往返与未知回退；负向断言：不做对比度校验、不提示、不因对比度拒绝主题或自动回退，不静默改写用户设置。
- 生成层：构建脚本对缺键、多余键、占位符不一致、空值给出失败；生成枚举与目录键一一对应。
- 语言层：System 协商（`zh_CN.UTF-8`、`zh-Hant-HK`、`en-GB`、未知、无 locale）、语言切换即时重绘、格式化字面替换、缺失参数保留、复数规则。
- 虚拟窗口层：主题切换不重建面板；密度切换不改变终端行列；透明度只影响壳表面；壁纸失败保留原图；应用页语言入口即时生效并持久化。
- 人工/环境面：真实字体渲染、CJK 字体回退、壁纸观感、Mica/Acrylic 视觉与 DWM 占用；登记豁免与人工核验。
- 静态门禁：脚本扫描业务组件中的颜色字面量、`if language == ...` 双语分支与未使用 `Message` 的字符串键。
