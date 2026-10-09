# REVIEW A:跨篇一致性 + 裁决校准

> 角度 A review 产出。真值源:`DECISIONS.md`(裁决单源,A/B/C/D 四表);各篇「待沉淀决策」节为过程稿,制度上豁免,本报告只列**正文**(目标形态/边界条/关键类型/数据流/缝合点/改造节/测试点/阶段归属/开放问题节)与裁决冲突或跨篇断链的确认项。
> 核查面:01-12 全篇通读 + 跨篇类型/改名 grep 抽查 + pebrel 源码(`D:/data/learn/code/AIGC-tools/pebrel`,baseline 工作树)一手核对。
> 分级:🔴 阻塞(整段机制/结构按被推翻选项展开,实现期照做必大返工)/ 🟡 应修(正文与裁决冲突或断链,默认解 = 按 DECISIONS/锚点改写)/ 🟢 建议。

## 🔴 阻塞(3 条)

### A-1 | 01 篇 workspace 终态仍按 slterm_config 双 crate(B.1 已裁并入 settings)

- 定位:`01-arch-baseline.md` 目标 workspace 表(crate 行含 `slterm_config`/`slterm_config_derive`)、根 Cargo.toml members 草稿、dependencies.toml 草稿(`[crates.slterm_config]`/`[crates.slterm_config_derive]` + slterm_app deps)、file-budgets roots、M1「落位裁定」子步出口(「决策落定(保留双 crate 或并入 settings 单 crate)」仍当未决)。
- 问题:B.1 已裁 slterm_config 并入 settings,01 篇全套骨架文件(members/依赖方向/行数预算)仍按双 crate 写;连带 `12-docs-packaging.md` 两处「8 个成员 crate」(文档面清单/新鲜度链源码根)同源滞后(并入后 = 6 crate)。
- 影响:M1 落骨架即建出两个该裁掉的 crate + 全门禁配置为其开户,返工面 = 整个 workspace 骨架与门禁数据文件。
- 建议:01 篇删双 crate 行、M1 出口改「B.1 已裁,并入 settings 单 crate」;12 篇 crate 计数同步。

### A-2 | 06 篇「旧 settings.json 八键一次性迁移」整段(D06-2 已裁零迁移)

- 定位:`06-settings-theme-i18n.md` 改造节 2(锁文件 + 哨兵 + 映射表全机制)、M6.1 子步「八键迁移」、测试点「设置迁移:旧八键→新段映射全键」。
- 问题:D06-2 已裁零迁移(新版只读写 `%APPDATA%\slterm`,旧 exe 同级文件不看不迁,仅 projects.json 按 B.20 提取),06 篇正文按被推翻选项展开了一整套迁移机制设计。
- 影响:M6.1 照做 = 实现并测试一套裁决已禁的迁移子系统,纯反向工作量。
- 建议:整段删除,M6.1 子步与测试点改写为零迁移断言(旧文件存在与否行为不变)。

### A-3 | 07 篇 git 执行体按 git2 双轨移植设计(D07-1 已裁子进程统一)

- 定位:`07-files-editor.md` 模块终态表(`slterm_app/src/git/` git2 移植行)、改造节 2(「两执行体并存是照抄的既成事实」+ GIT_OPTIONAL_LOCKS=0 等价纪律)、边界条 7、M8.1、测试点(git2 夹具)。
- 问题:D07-1 已裁子进程统一(与 pebrel 同构;git2 语义资产转为测试保留),07 篇正文按「双轨并存 + facade 收敛」的旧默认展开移植设计。
- 影响:M8.1 照做 = 移植并维护一个裁决已砍的执行体,返工面 = 整个 git 域。
- 建议:改造节 2 重写为子进程统一(git2 不移植,其语义资产转测试夹具),终态表/边界条/测试点联动。

## 🟡 应修(28 条)

### 02 篇

**A-4 | DA1 应答身份表格值未随 D02-1 改**
- 定位:DA1/DSR 接管状态机表(sideloaded host 启动握手应答字节 ` \x1b[?61c`);合流细节节已注明「priming 预写字节与统一身份同步改」但表格值漏改。
- 问题:D02-1 已裁 `?64;22c` 三处同身份(priming/Term 自答/reader 代答),表格残留 `?61c`。
- 影响:M2 照抄 = 三处身份不一致,slTerminal 现役 CLI 兼容契约破。
- 建议:表格值改 `?64;22c`。

### 04 篇

**A-5 | D04-1 已裁 console 子系统,正文仍外推 12 篇裁定**
- 定位:改造节 3(「或反之 console 子系统接受启动闪窗归 12 篇裁定」)、M7.4(「子系统细节归 12 篇归 D04-1」)。
- 问题:D04-1 已裁 console 子系统(GUI 启动闪控制台窗接受),04 篇正文仍当未决外推;且 12 篇全文无此承接口径。
- 影响:M7.4 出口挂空裁定方。
- 建议:改写为「D04-1 已裁 console 子系统」陈述句。

**A-6 | 开放问题 3(legacy ATTACH 首发开关位归 12)→ 12 篇无承接**
- 定位:`04-runtime-api.md` 开放问题 3;`12-docs-packaging.md` 全文无 ATTACH/旧客户端接入开关条目。
- 问题:04 把首发开关裁定权交 12,12 未承接;DECISIONS 四表亦无此项登记。
- 影响:缝合断链,实现期无人认领。
- 建议:12 篇打包开关节补登记,或 04 篇收回自裁。

**A-7 | 缝合点 5 回登缺口(C 表声称已回登)**
- 定位:`04-runtime-api.md` 缝合点 5(无「聚焦唯一窗 + tab.new」回登文字);`09-system-integration.md` 改造节 1 已作答并自称「回登 04 篇缝合点 5」。
- 问题:DECISIONS C 表「04开放4 → 09 已答并回登 04 缝合点 5」,实际 04 侧未回登。
- 影响:裁决闭环记录与文档现状不符。
- 建议:04 缝合点 5 补一行回登。

**A-8 | M7 出口段「归开放问题 4」编号错指**
- 定位:改造节 6/测试点附近「协议文件转正式归置归开放问题 4」;开放问题节实际 schema 归置 = 开放问题 1(开放问题 4 = 移交窗口还原)。
- 问题:编号错指;且 schema 归置已经 D12-3 裁(assets/)。
- 影响:读者顺错编号找到无关条目。
- 建议:改「开放问题 1」并标注已闭环(D12-3)。

### 05 篇

**A-9 | D05-1 已裁回放,边界条 5 与 schema 注释仍「只写不回放」**
- 定位:边界条 5(「WindowState 只写不回放…归待沉淀 D05-1」)、session schema WindowState 注释(「只写不回放…启动按配置列行数定形」)。
- 问题:D05-1 已裁回放(记住上次窗口大小/位置,schema 字段语义 = 行为输入)。
- 影响:M5.5 快照闭环照注释做 = 裁决心仪功能缺席。
- 建议:两处改写为回放口径。

**A-10 | D05-2 已裁双槽,SidebarState 关键类型仍单槽草稿**
- 定位:关键类型节 SidebarState(active_view 单槽注释;篇内后文自标挂账)。
- 问题:D05-2 已裁双槽(ActivityBar + 上下槽,R1-R9 随迁),类型草稿未改。
- 影响:M5 侧栏骨架照类型做 = 结构返工;session schema 持久化形态难逆。
- 建议:SidebarState 按双槽重定字段。

**A-11 | D05-3 已裁归 session,边界条 6 与字段注释仍「内存态/归 06 设置键」**
- 定位:边界条 6(侧栏开合/sideViews 选中「全部内存态」)、SidebarState 字段注释(collapsed「内存态;设置键归 06」、width「持久化归 06 设置键」)。
- 问题:D05-3 已裁 collapsed/width/active_view 归 session 跟现场恢复。
- 影响:状态归属错层(06/内存 vs session),恢复行为缺失。
- 建议:两处改归 session schema。

**A-12 | 标题栏归属两处「归 09」未按 C 表改归 05(已知待修项核验:确认未修)**
- 定位:关键类型节 titleBar 段(「壳重建归 09 篇」)、缝合点 6(「titleBar 归 09」)。
- 问题:DECISIONS C 表「09开放1 → 按 spec 归 05,05 篇文字待修」,05 篇两处文字未修;09 篇开放问题 1 亦旁证待 05 自洽。
- 影响:两篇各执一词,M5/M10 分工争议。
- 建议:05 两处改「归 05」,09 开放问题 1 销项。

### 06 篇

**A-13 | D06-1 已裁段嵌套,settings.json 实例仍按平铺展示**
- 定位:settings.json 实例(平铺 snake_case,注「段嵌套候选见 D06-1」)、AgentHook settings_key 注释、SettingsKeyDef.key 注释(「扁平 snake_case」)。
- 问题:D06-1 已裁段嵌套(写通道按段浅合并),实例与注释未改。
- 影响:照示例建 schema = 首版键形态错;设置键进持久化即背负迁移义务,难逆。
- 建议:实例改段嵌套形态,注释同步。

**A-14 | D06-4 已裁「06 字段一次到位锚定」,正文三处仍「值域重定归 09」**
- 定位:裁剪清单(BlurModeName 五档材质枚举「值域重定归 09 篇」)、blur 字段注释(「值域重定归 09 篇(D06-4)」)、缝合点 7(「opacity/blur 值域与窗口特效归 09」)。
- 问题:D06-4/D09-1 已裁五材质枚举(默认 None)+ opacity 标量、06 字段一次到位锚定、无 bool 占位;09 篇改造节 3 已按此落地(方向相反:09 等 06 锚,06 推 09 定)。
- 影响:M6.1 字段类型冻结无锚,双篇互等。
- 建议:06 按裁决锚定字段类型(五材质枚举 + 标量),删「归 09」挂账。

**A-15 | D04-3 已裁删键,windowing_behavior 字段与枚举仍在**
- 定位:RuntimeSettings.windowing_behavior 字段、WindowingBehaviorName 枚举(裁剪清单与键域表两处)。
- 问题:B.10 已裁删键(单窗行为硬编码归 09),06 篇类型面未删。
- 影响:死键进 RuntimeSettings 锚点与测试字面量。
- 建议:删字段与枚举。

**A-16 | D05-3 连锁:RuntimeSettings.sidebar_width 成无源字段**
- 定位:RuntimeSettings.sidebar_width 字段;边界条 10 已自承「D05-3 若归设置域经本篇键域落地」——裁决归 session,该前提不成立。
- 问题:字段失去归属依据,悬在 RuntimeSettings。
- 影响:与 05 篇 session 侧 width 重复定义风险。
- 建议:删字段(宽度归 05 session)。

**A-17 | D2=slterm 残留:`%APPDATA%\slTerminal` 五处**
- 定位:`01-arch-baseline.md` settings_dir 注释、测试名 `windows_path_uses_slterminal_layout`(改造节 + 测试点表各一处)、改名映射 D 表行(映射值 `%APPDATA%\slTerminal`);`06-settings-theme-i18n.md` paths.rs 段(「默认 %APPDATA%\slTerminal,叶子名承 01 篇 D2」)。
- 问题:D2 已裁叶子名 `slterm`;01 D 表行备注「叶子名取值见 D2」半挂账但具体值错。
- 影响:M1 路径落位照抄 = 数据目录名错,测试名/断言连锁错。
- 建议:五处一律改 `slterm`(测试名同步 `*_slterm_layout`)。

### 08 篇

**A-18 | 测试基建错指 12 篇(应 11);「组件实现/键位归 11」11 篇无承接**
- 定位:缝合点表(「测试基建|L1 测试迁移与豁免登记|12 篇」)、改造节(「具体豁免与落位归 12 篇」「层级归 12 篇分配」);另三处(键位细节/组件与 Ctrl+. 接线/三者的组件实现、焦点/键位裁决)指 11 篇。
- 问题:测试基建锚点篇 = 11(11 篇篇首自承),指 12 为错指;而 11 篇是测试体系,全文无补全组件实现/Ctrl+. 键位承接,指 11 的三处为断链。
- 影响:M9 落地时测试形态与组件键位双方找不到承接方。
- 建议:12 改 11;组件实现/键位归属重新指派(候选:08 本篇或 05/06 壳层)。

**A-19 | ProviderTestOutcome 计数矛盾(十态 vs 十七态)**
- 定位:模块终态表(「连通性测试十态」)、关键类型节(「十态族」)vs 改造节 7(「十七态」)。
- 问题:篇内前后矛盾;一手核 pebrel `nebula_app/src/provider_test.rs` 枚举 = **17 个变体**,「十七态」为真;且计数入文档违反快照数字禁令。
- 影响:测试矩阵照「十态」写 = 漏 7 变体断言。
- 建议:删计数表述(引符号名),或统一为十七态。

**A-20 | load_api_key 签名 Vec<u8> 出域 vs 10 篇 SecretBytes 锚点**
- 定位:`08-ai-assistants.md` 关键类型节(`pub fn load_api_key(id) -> io::Result<Option<Vec<u8>>>` 注「出域即 Zeroizing」);`10-security-plan.md` 锚点(SecretBytes 句柄,并明写「08 篇消费签名按本篇锚点改」)。
- 问题:跨篇类型形态冲突:10 把「即用即焚」从纪律降为类型,08 仍调用点自觉形态。
- 影响:凭据域边界静态审查测试(10 篇 M10.9 出口)照 08 签名做必红。
- 建议:08 签名改 `Option<SecretBytes>`。

### 09 篇

**A-21 | 静默启动「三条件 vs 两条件」篇内矛盾**
- 定位:目标形态表、边界条 8、startup.rs 签名注释、缝合点表行 36、M10.6 出口(「三条件真值表测试绿」)均写三条件;改造节 1 写两条件(`hide_window_on_close` 因子消去,`silent_start && tray`)。
- 问题:改造节 1 口径正确(关窗即退定位下 hide_window_on_close 键不存在),其余五处(含 M10.6 测试出口)未同步。
- 影响:M10.6 照出口写三条件真值表测试 = 断言对象不存在。
- 建议:五处统一为两条件口径。

**A-22 | 06↔09 设置键缝合面漂移:壁纸键命名/计数 + launch_at_login 缺席**
- 定位:`09-system-integration.md` 缝合点 5(设置键含 `visual_*`/`launch_at_login`)、照抄点 39(「06 `wallpaper_*` 五键」);`06-settings-theme-i18n.md` 字段 = `background_image_*` 五字段、缝合点 7 自称「壁纸四键」、全篇无 launch_at_login。
- 问题:三重漂移——键名(wallpaper_*/visual_* vs background_image_*)、计数(06 五字段 vs 06 自称四键)、键缺席(09 消费的 launch_at_login 06 未锚定)。
- 影响:M6/M10 键域对不上,照 09 名词做 = 编译期/测试字面量全错。
- 建议:以 06 字段为准统一名词与计数;launch_at_login 键补入 06 或 09 改自锚。

**A-23 | SLTERM_RELEASES_* 真值已裁仍挂「未定/待回填」(09+12)**
- 定位:`09-system-integration.md` 开放问题 3(「12 篇落定前以占位符表达,12 定稿后回填」);`12-docs-packaging.md` 常量注释(「真值待定,开放问题 1」)、AppUpdatesURL 注释(「坐标定稿后回填」)、开放问题 1(「均未定…留占位回填」)。
- 问题:DECISIONS A 表末行已落定真值(LiuShenLan/slTerminal,「09/12 篇占位回填」);12 常量体已填值但注释与开放问题未收敛,09 仍在等 12。
- 影响:开放问题空转;更新链验收前提(坐标可覆盖)表述含糊。
- 建议:12 删「待定」注释与开放问题 1(改登记已裁真值),09 开放问题 3 销项。

### 10 篇

**A-24 | D10-1 已裁 TRMBK001,关键类型节常量仍 `b"SLTRMBK1"`(难逆)**
- 定位:加密备份签名段(魔数注释「默认 SLTRMBK1 形态」+ `const MAGIC = b"SLTRMBK1"`)、照抄清单行 9(「魔数归 D10-1(默认 SLTRMBK1)」)。
- 问题:D10-1 已裁 `TRMBK001`(去品牌,与 D3 同理由),正文常量草稿未改。
- 影响:魔数随首份备份产物固化(D10-1 自陈难逆点),M10.10 照抄 = 错魔数进用户备份,改 = 旧备份不可读。
- 建议:常量改 `TRMBK001`,清单行同步。

**A-25 | D10-3/D10-4 已裁,正文多处仍挂「归 D10-x」待决;开放问题 3 未回登闭环**
- 定位:plan_balance 签名区(apply_snapshot「签名归 D10-3」)、改造节 1(「订阅模型归 D10-3」)、状态机(「推送形态归 D10-3」)、缝合点表(「加密备份 UI 入口落位归 D10-4」);开放问题 3(plan 快照迁移「待 D2/D3 裁决后回登」)。
- 问题:D10-3 已裁模块级 watch channel、D10-4 已裁 06 设置页安全分区(恢复点清单同面);开放问题 3 已经 C 表闭环(D06-2 零迁移,不迁)。
- 影响:M10.10/M10.11 开工前挂账点其实已有裁决,照挂账等 = 停滞。
- 建议:三处 D10-3 改 watch channel 形态、缝合点表 D10-4 改 06 设置页、开放问题 3 删。

**A-26 | 凭据 target 与 `.slterm-recovery` 称归 01 改名表,01 表缺席**
- 定位:`10-security-plan.md` 照抄清单行 4(`credential_target`「归 01 篇改名映射表归位」)、恢复点段(「.pebrel-recovery 改名归 D 节」);`01-arch-baseline.md` 改名映射表无 `Slterm/AI/` 与 `.slterm-recovery` 条目(grep 确认)。
- 问题:10 把两个改名项的登记处指到 01 单点表,01 未登记,单点表真值缺两行。
- 影响:禁名门禁/改名核对时两值无权威出处。
- 建议:01 表补 `Pebrel/AI/<id>` → `Slterm/AI/<id>` 与 `.pebrel-recovery` → `.slterm-recovery` 两行。

### 11 篇

**A-27 | D11-1/D11-2 已裁,改造节 1 仍按「M1 实测后定」待决写**
- 定位:改造节 1 防线 2(「显式 test target 形态归留归待沉淀 D1…按 M1 落位时实测定,不照搬」)。
- 问题:D11-1 已裁每 crate 显式 test target、D11-2 已裁 cargo xtask 封装(cargo xtest),改造节正文仍按待决推进。
- 影响:M1 首 crate 落位时入口形态无定论,防线空窗。
- 建议:改写为已裁形态(显式 target + xtask),预防性知识段落保留。

**A-28 | `cargo check -p slterm_app` vs 01 篇 package 名 slterm**
- 定位:`11-testing.md` 改造节 7 步骤 5(feature off 形态 `cargo check -p slterm_app`);`01-arch-baseline.md` 改名映射(「目录 nebula_app,package nebula → slterm_app(package slterm)」)与 workspace 表(`slterm_app | lib+bin slterm`)。
- 问题:package 名 = `slterm`(目录名才是 slterm_app),`-p slterm_app` 无此包;12 篇 `-p slterm --bin slterm` 为正确形态。
- 影响:双形态编译门禁命令照抄即报错,门禁静默失效风险。
- 建议:改 `cargo check -p slterm`。

**A-29 | 阶段表 M8/M9 出口错位一行**
- 定位:阶段归属表 M8 行(内容「编辑器内核测试组织归 07」出口却写「归 08 出口」)、M9 行(内容「补全引擎/should_suggest 测试归 08」出口却写「归 09 出口」)。
- 问题:出口列与内容列错配(off-by-one):M8 文件与编辑应对 07,M9 AI 辅助应对 08。
- 影响:伴生出口认领方错。
- 建议:两格改「归 07 出口」「归 08 出口」。

**A-30 | windows_console_startup 对照目标理据与 D04-1 冲突**
- 定位:关键类型节 windows_console_startup 段(「GPUI 应用本体是 GUI 子系统,此对照目标是唯一可观测继承语义的载体」)。
- 问题:D04-1 已裁本体 = console 子系统,「本体 GUI 子系统」前提失效;对照目标的存废与因果链需按新前提重估(console 本体下 Ctrl+C 继承可直接观测)。
- 影响:M7 照抄建一个理据已消失的测试目标(或漏掉仍必要的差异面)。
- 建议:按 console 子系统重估该目标存废并改写因果链。

### 12 篇

**A-31 | DefaultDirName 注释「叶子名与 D2 同名原则」失实**
- 定位:installer.iss 骨架(DefaultDirName={autopf}\slTerminal,注释「叶子名与 01 篇 D2 数据目录同名原则」)。
- 问题:D2 值 = `slterm`(数据目录),安装目录叶子名取 `slTerminal`,两者并不同名;注释把「同品牌」误写为「同名」。
- 影响:后续维护者照注释「纠正」其中一处 = 误改。
- 建议:注释改为「安装目录取品牌全名,数据目录取 D2=slterm,二者不同名系有意」。

## 🟢 建议(8 条)

1. **05 PaneId/TabId 缺 serde derive**(ids.rs 仅 Debug/Clone/Copy/PartialEq/Eq/Hash/PartialOrd/Ord):04 篇 RuntimeTab.tab_id 要求「serde 透明序列化为 u64」,05 侧未 derive Serialize/Deserialize——建议补 derive(透明 newtype)。
2. **05 WindowState{width,height,maximized} 无位置字段**:D05-1 回放含「位置」,回放落地需 `#[serde(default)]` 扩 x/y(或注明首版只回放尺寸/最大化,位置留扩)。
3. **07 D07-3 默认尾巴自相矛盾**:默认后半句「旧 editor 值迁移时一次性并入并登记映射」与裁决主句「不做旧场景配置迁移」及 D06-2 零迁移背景冲突——建议删尾巴。
4. **08 D08-2/D08-3 行文滞后**:待沉淀节仍写「M9.3/M9.4 拍板」、改造节 6 保留「若 fork 不随迁则…」备选——裁决已定(默认关/最小集随迁),建议正文收敛。
5. **09 TrayAgent.label 示例残留禁名词**:「claude · nebula」示例文案(docs 豁免期内合法,但防照抄进代码触禁名门禁)——建议换示例词。
6. **新增跨域类型未回登 01 锚点表**:09 篇 BlurMode 值域、10 篇 PlanBalanceInfo 快照类型均自称本篇锚点,但未回 01 表登记(01 表规则:「新增跨领域类型时先在本表登记归属」)——建议补登记。
7. **10 篇病句堆叠(多人续写拼接痕)**:节标题与缝合点表多处「归 08 结构体归本篇凭据纪律归 08 消费归本篇锚点」「查询语义原样归 09 消费归本篇锚定」类堆叠——语义可读但需通稿一遍;前后矛盾项未发现(双注册表/审计通道/契约数值自洽)。
8. **11 开放问题 2/3 行文滞后**:默认口径已与 B.23(pr_size_hint 不挂 check_all)、B.24(M11 前全量过 + 阶段出口前可选)一致,仍挂「开放」——建议收敛为既定形态。

## 待核(2 条)

1. **04 篇「笔误已修」复验无法执行**:任务所列「04 篇笔误已修(复验)」未给出原笔误定位/内容,全篇通读未发现可对应的残留错字——需任务发起方提供笔误出处后方可复验。
2. **09 篇开放问题 3 与 12 篇开放问题 1 的「公网坐标占位」语义**:若首发不开 GitHub Release(12 开放问题 1 提及 `update-test-source` 本地通道验收),A 表真值是否仍作为首发常量值,属产品决策面,本角度不替裁——仅提示两开放问题与 A 表真值的关系需在 M11.4 前一句话说清。

## 已核通过项(择要)

- M 子步编号:09 篇 M10.1–M10.8 与 10 篇 M10.9–M10.12 接续无重叠;12 篇 M11.1–M11.4 无跨篇重号;00-roadmap 只定义到阶段级,无细分冲突;各篇出口标准抽查均为机验形态(测试绿/命令过),未见「功能可用」类模糊出口。
- 09 篇 BlurMode 五材质枚举(默认 None)+ opacity 标量与 D06-4/D09-1 一致;09 改造节 1 已实质回答 04 开放问题 4(答案本体存在,仅 04 侧回登缺,见 A-7)。
- 03 篇 D03-1/2/3/4 默认与裁决一致;PaneId 以 u64 形参承接 05 newtype 已注明。
- 08 篇命名偏差(history.jsonl/providers.json)已从 01 表权威,偏差登记开放问题 4 归 M11 回改 spec——已知待修项核验通过。
- 12 篇 D12-1(GUID 新立)/D12-2(实测+15% 裕度)/D12-3(assets/ 已落地正文)/D12-4(installer.iss 默认 0.3.0)与裁决一致;04 include_str! 的 docs/ 路径已经 12 缝合点 4 登记连锁(M11.3 改指 assets/),无断链。
- 协议名 `slterm.runtime`(04 锚、11 消费)一致;09 background_tasks 注册表 planBalance 条目承接 10 篇消费;pebrel 现役 env 名 PEBREL_CONFIG_DIR(NEBULA_CONFIG_DIR 为兼容层)经一手核对,11 篇映射源名成立。
- 10 篇审计通道(tracing target "audit")锚定与 03 篇消费方向一致;10 篇 M10.9–M10.12 出口均机验。
