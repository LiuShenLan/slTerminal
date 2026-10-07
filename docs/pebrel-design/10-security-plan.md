# 10 安全与套餐详细设计

> pebrel-design 分片 10/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/10-security-plan.md`(spec 分片,含采纳点编号);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **凭据类型**(凭据引用字段、密文句柄、Zeroizing 包装、凭据域边界类型)的锚点归属篇(01 篇类型锚点表已登记:`凭据类型(provider 凭据、引用字段、尾四掩码)→ 10 security,token 不出凭据域的类型层边界`)——`CredentialRef`/`api_key_hint` 掩码语义/`SecretBytes` 出域句柄/凭据域门面签名级定义唯一定义在本篇;他篇(04/06/08 等)只许 `use` 或经 facade 传参,禁重定义。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号;引本仓源码用符号名。

## 目标形态

安全域在单进程 GPUI 模块化单体中的终态,三个能力面全部收敛:

- **凭据基建两层**——`slterm_app::platform::credentials`(Windows 凭据管理器唯一封装:load/store/delete 三原语 + Win32 原语,target 命名空间归调用方,全门面禁旁路)+ 域凭据面(provider 凭据库在其上叠独立文件 + 引用字段 + 尾四掩码 + 写即清稿 + Zeroizing 纪律)。plan_balance 的 token 只读通道独立不并库,共享面仅原语封装与 Zeroizing 纪律,不共享凭据本体(spec 分片 10 裁定 28)。
- **本地加密备份**——AES-256-GCM + Argon2id 管线、魔数 AAD、白名单即安全边界、先验后写、恢复点撤回全链路照抄 pebrel;slterm 魔数与类别清单重设计;凭据库/token 来源文件永不进备份清单为设计红线;远端推送不建。
- **安全审计与套餐**——hooks 域审计三件套(SEC-12 statusline 审查 / SEC-13 Outdated 防篡改 / SEC-17 user 层写入审计)作为 Rust 域逻辑原样存活,审计通道 = tracing target 保留,用户确认交互变 GPUI 原生模态;plan_balance 套餐余额移植,查询语义原样,「token 不出后端」升格为「token 不出凭据域」。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_app/src/platform/credentials.rs` | pebrel `nebula_app/src/platform/credentials.rs` 照抄改造(砍 legacy 双身份与多平台分支) | 凭据管理器通用封装:`CredentialIdentity` 单态 + `load`/`store`/`store_with_username`/`delete`,Windows 分支转 `credentials/windows.rs` |
| `slterm_app/src/platform/credentials/windows.rs` | pebrel `nebula_app/src/ssh_credentials.rs::windows_store` 的 `load_secret`/`save_secret`/`delete_secret` 解绑改名 | Win32 原语:CredReadW/CredWriteW/CredDeleteW + `CRED_TYPE_GENERIC` + `CRED_PERSIST_LOCAL_MACHINE` + ERROR_NOT_FOUND 归一 + 缓冲区全 `fill(0)` |
| `slterm_app/src/secrets.rs`(门面,本篇新建) | pebrel `ssh_credentials.rs::{store_generic_secret,load_generic_secret,delete_generic_secret}` 三原语门面搬迁 | 非 SSH 功能消费凭据管理器的唯一入口;「target 命名空间归调用方」doc comment 随迁 |
| `slterm_app/src/ai_providers.rs` + `provider_test.rs` | pebrel 照抄(eg 08 篇元数据面;凭据安全面归本篇锚点) | 十三家 provider 元数据/凭据引用体系;`credential_target`(`Slterm/AI/<id>`)/`save_api_key`/`store_provider_api_key`/`delete_api_key`/`remove_provider`/`load_api_key`/`test_provider` 凭据纪律归本篇 |
| `slterm_app/src/encrypted_backup.rs` + `encrypted_backup/recovery.rs` | pebrel 照抄改造(SSH 类别砍、清单重定、魔数重设计) | AES-256-GCM 口令保护备份:collect/seal/open/restore 全链 + 选择性恢复与加密恢复点撤回 |
| `slterm_app/src/plan_balance/`(mod/source/query/deepseek/kimi) | slTerminal 自有(`src-tauri/src/plan_balance/`,git 历史提取)移植 | 套餐余额:双静态切片注册表、快照合并、emit 口径、实证红线原样;token 全链路 Zeroizing 化(补强) |
| `slterm_app/src/ai_hook/local/` 审计件 | slTerminal 自有(hooks/claude/inject.rs + config.rs 提取)归 03 篇安装器家族归位归 03;审计语义归本篇登记 | SEC-12 审查并入 03 篇 `review_statusline_command`;SEC-13 哈希对账归 03 注入状态三态;SEC-17 user 层写入审计归本篇 tracing 通道裁定 |

裁剪不迁:SSH 凭据全家(askpass 面/prompt_password/私钥口令,`ssh_credentials.rs` 除三原语与 Win32 原语外全砍);多平台凭据后端(macOS security_framework/Linux secret-tool/`can_store` 平台分叉);Nebula→Pebrel  legacy 凭据迁移链(`CredentialIdentity::legacy`/`load_with`/`delete_with`);`prompt_generic_secret` OS 凭据提示框(CredUIPromptForCredentialsW);远端备份推送全家(`backup_remote.rs`:UNC/WebDAV/S3/SFTP + `KEEP_ARCHIVES` + `pebrel_backup.txt`);备份清单的 pebrel 文件名具象与 `BackupManifest.device` 字段。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri 命令 / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。
2. **token 不出凭据域**(00-roadmap 不变量 2 的本篇落地)三重防线:类型层(`PlanBalanceInfo`/`FetchOutcome` 等快照与 DTO 类型无 token 字段,新增即编译期拒;serde 键集精确匹配测试升格保留为类型层断言)、内存擦除(出域句柄 `SecretBytes` 即 `Zeroizing` 包装,即用即焚)、日志不插值(tracing/Err 消息禁止插值 token 与 Authorization 头——ureq 错误 Display 不含请求头,构造错误消息时禁止自行拼接,plan_balance 模块头注释红线随迁)。
3. **SEC-18 红线延续并扩展**:真实凭据值(API token/key、Authorization 头实际值、备份口令)禁止写入任何 git 追踪文件——代码、测试夹具、文档、脚本一律不行;测试与文档仅允许假值占位符(`sk-test` 形态);真实凭据全应用仅两处落点:user 层 `~/.claude/settings.json`(plan_balance 只读,仓库外)、Windows 凭据管理器(ai_providers 库写通道)。扩展适用面:本片新增凭据面(provider 凭据库、加密备份口令、恢复点)凡 git 追踪面恒为假值。
4. **凭据域唯一入口纪律**:任何域消费凭据管理器只经 `secrets.rs` 三原语门面(`store_generic_secret`/`load_generic_secret`/`delete_generic_secret`),禁止旁路直调 Win32 或 `platform::credentials` 之外的第二封装;target 命名空间按域划分(`Slterm/AI/<id>` / 未来 `Slterm/Backup/<n>`),凭据库可多域复用。
5. **文件边界即凭据边界**:provider 凭据独立文件 `providers.json`(01 篇 D 节归位归 08 篇消费)绝不与普通设置同文件序列化——普通设置是用户可编辑运行态文件,凭据需要稳定身份(pebrel `ai_providers.rs` 模块头注释纪律逐字随迁);明文永不入 store,store 文件可整份展示/备份/diff 而无泄露。
6. **白名单即备份安全边界**:加密备份不遍历用户 home 目录,`collect_from` 只认 data_dir 固定文件清单;AI 会话库、环境变量、凭据库、token 来源文件(`~/.claude/settings.json`)永不读写进出备份(pebrel 模块头注释「The allowlist below is the security boundary for both export and import」逐字随迁语义);导入导出同一道边界,清单外文件不存在「顺手带上」的通道。
7. **远端推送不建**:安全三件套 = 本地加密备份;SFTP 随 SSH 砍,WebDAV/S3/OAuth 回环不在范围;加密归档格式与远端推送解耦照抄,远端通道不建,未来若需另行立项(spec 不采纳点 6)。
8. **审计可观测面不降级**:SEC-12/13/17 三件的「审计通道」等价物 = tracing target `"audit"` 保留(TQ-COV-05 tracing-test 断言兜底可观测的等价形态归 11 篇);用户确认交互(SEC-12 Suspended 确认)变 GPUI 原生模态,确认语义不变。
9. **plan_balance 查询语义原样**:来源判定、双注册表、快照合并(merge_slot 四分支)、emit 口径(含 updated_at 整体 PartialEq)、冻结与已用百分比口径、kimi 实证红线(2026-08 实测修正规格、remaining 恒在/used 可缺、全有或全无、数值字段按字符串解析)全部原样随迁,任何「顺手改进」须经实证重测。
10. **类型锚点纪律**:本篇唯一定义凭据类型(引用字段语义、掩码、出域句柄、门面签名);跨领域类型只消费:`PaneId`/`TerminalPane`(05)、Runtime API 信封(04)、`AiProvider`/`ProviderStore`/`ProviderTestOutcome`(08)、`RuntimeSettings`/设置页键域(06)、`AppError`(01)。`PlanBalanceInfo` 快照类型归本篇锚定(plan_balance 域内消费,壳/HUD 经只读投影消费,eg 09 篇通知漏斗不归本篇)。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致,改名/收敛点已标;契约数值(Argon2id 参数组/魔数长度/口令下限/盐 nonce 尺寸)首发取 pebrel 实测值照抄,与 04 篇 D04-2 同纪律。

### 凭据域类型(本篇锚点)

```rust
// slterm_app/src/secrets.rs —— 凭据域门面(本篇锚点;pebrel ssh_credentials.rs
// 三原语门面搬迁,doc comment「Keeping the target namespace at the caller makes
// the secret store reusable without duplicating Windows API code or ever exposing
// the value to UI state」逐字随迁)
/// 凭据域唯一入口:非 SSH 功能消费凭据管理器只经此门面,禁止旁路直调 Win32。
pub fn store_generic_secret(target: &str, secret: &[u8]) -> std::io::Result<()>;
pub fn load_generic_secret(target: &str) -> std::io::Result<Option<Vec<u8>>>;
pub fn delete_generic_secret(target: &str) -> std::io::Result<()>;
// 委托 platform::credentials::{store,load,delete};返回值出域即由调用方
// Zeroizing 包装(pebrel 形态:裸 Vec<u8> 出域,纪律在调用点——本篇补强为
// 「出域句柄」形态见下,不依赖调用点自觉)。

// slterm_app/src/platform/credentials.rs —— 通用封装(照抄改造)
// CredentialIdentity 收敛为单一 Slterm service 单态(legacy 双身份/迁移链不迁,
// spec 不采纳点 3):
struct CredentialIdentity<'a> { service: &'static str /* "Slterm" */, target: Cow<'a, str> }
impl<'a> CredentialIdentity<'a> { fn current(target: &'a str) -> Self; }   // 无 legacy 回退
pub fn load(target: &str) -> io::Result<Option<Vec<u8>>>;
pub fn store(target: &str, secret: &[u8]) -> io::Result<()>;
pub fn store_with_username(target: &str, username: &str, secret: &[u8]) -> io::Result<()>;
pub fn delete(target: &str) -> io::Result<()>;
// Windows 分支(#[cfg(windows)] 收敛于本模块 + windows.rs,归 00-roadmap 不变量 4):
fn load_at/store_at/delete_at -> credentials::windows::{load_secret,save_secret,delete_secret}

// slterm_app/src/platform/credentials/windows.rs —— Win32 原语
// (pebrel ssh_credentials.rs::windows_store 解绑;askpass/prompt 全家砍)
pub(crate) fn load_secret(target_name: &str) -> io::Result<Option<Vec<u8>>>;
// CredReadW + CRED_TYPE_GENERIC;ERROR_NOT_FOUND 归一为 Ok(None);CredFree 释放
pub(crate) fn save_secret(target_name: &str, username: &str, secret: &[u8]) -> io::Result<()>;
// CredWriteW + CRED_PERSIST_LOCAL_MACHINE;写后 blob/target/user 缓冲区全 fill(0)
pub(crate) fn delete_secret(target_name: &str) -> io::Result<()>;
// CredDeleteW;ERROR_NOT_FOUND 归一为 Ok(())
```

**出域句柄与引用字段(本篇锚点,token 不出凭据域的类型层落地)**:

```rust
// 出域密文句柄:load 出域的裸 Vec<u8> 由门面侧包 Zeroizing——调用方拿到的
// 不是 String/Vec 而是此句柄,「即用即焚」从纪律降为类型(补强,非行为变更;
// pebrel 形态 = 调用点自觉 Zeroizing,ai_providers.rs::test_provider 即此形态)。
pub struct SecretBytes(Zeroizing<Vec<u8>>);
impl SecretBytes {
    pub fn as_bytes(&self) -> &[u8];              // 只借出,不转移所有权
    pub fn into_string(self) -> Option<Zeroizing<String>>;  // UTF-8 校验合并入句柄
}
// 使用点:ai_providers.rs::load_api_key 返回 io::Result<Option<SecretBytes>>
// (08 篇消费签名按本篇锚点改;plan_balance 的 fetch(token) 入参同改 SecretBytes 借用)。

// 凭据引用字段(AiProvider 内,eg 08 篇结构体归位归 08;字段语义锚定归本篇):
//   api_key_set: bool      —— 「A credential reference only. The actual key is
//                             held by the OS store.」字段注释逐字随迁
//   api_key_hint: String   —— 尾四掩码,语义 = api_key_hint() 产出(下)
// 明文永不入 store;store 文件可整份展示/备份/diff 而无泄露——引用与元数据
// 分离是凭据面根纪律。
```

### provider 凭据面签名(归 08 篇结构体归本篇凭据纪律归 08 消费归本篇锚点)

```rust
// slterm_app/src/ai_providers.rs —— 凭据安全面(spec 采纳点 1-7 签名级)
pub fn credential_target(id: &str) -> String;        // "Slterm/AI/{id}"(归 01 表归位归本篇锚定)
pub fn api_key_hint(key: &str) -> String;            // trim 后空 → "";否则 "••••" + 末四位(chars 级)
pub fn save_api_key(id: &str, key: &str) -> io::Result<String>;   // 入 OS 库即返回 hint,不落中间态
/// Store a replacement key and update only its non-secret metadata.
/// The plaintext never enters `ProviderStore` and callers must clear their
/// write-only draft immediately after this returns.(doc comment 逐字随迁)
pub fn store_provider_api_key(provider: &mut AiProvider, key: &str) -> io::Result<()>;
pub fn delete_api_key(id: &str) -> io::Result<()>;
/// Remove a provider and its credential through one presentation-neutral
/// transition. Metadata is mutated only after credential deletion succeeds,
/// avoiding an unreachable orphaned secret.(doc comment 逐字随迁)
pub fn remove_provider(store: &mut ProviderStore, id: &str) -> io::Result<()>;
pub fn load_api_key(id: &str) -> io::Result<Option<SecretBytes>>;  // 出域即句柄
/// Blocking provider connectivity test shared by every UI runtime.
/// Callers must run this off their UI thread. Credential lookup stays inside
/// this function; no UI receives plaintext from the OS credential store.
pub fn test_provider(provider: &AiProvider) -> ProviderTestResult;
// key 与 "Bearer {key}" 拼接串均 Zeroizing::new 包裹(出域句柄形态下 = 句柄内联
// 构造);12s timeout_global;凭据查找内聚在函数体内,任何 UI 层零接触。
```

### 加密备份(照抄改造)

```rust
// slterm_app/src/encrypted_backup.rs
// 魔数重设计:D 节决策归 M10 落地时定(默认 "SLTRMBK1" 形态,见待沉淀 D10-1);
// 长度契约 8 字节随迁。ARCHIVE_VERSION/SALT_LEN/NONCE_LEN/KEY_LEN 数值照抄。
const MAGIC: &[u8; 8] = b"SLTRMBK1";       // ← D10-1 决策点,默认填
const ARCHIVE_VERSION: u32 = 1;            // 版本号校验:不符即「版本不支持」一票拒
const SALT_LEN: usize = 16; const NONCE_LEN: usize = 12; const KEY_LEN: usize = 32;

#[derive(Serialize, Deserialize)]
pub(crate) enum BackupCategory { /* slterm 自有清单归实现归 M10(归 D10-2);
    pebrel 九类中 Ssh 砍;「默认仅外观一项」的保守默认思想保留 */ }
pub struct BackupSelection { /* bool 字段族照抄;default = 仅外观 true 其余 false */
    pub(crate) fn is_empty(self) -> bool; pub(crate) fn set(&mut self, c: BackupCategory, bool);
    pub(crate) fn from_categories(impl IntoIterator<Item = BackupCategory>) -> Self;
    pub(crate) fn categories(self) -> impl Iterator<Item = BackupCategory>; }
pub(crate) struct BackupEntry { pub category: BackupCategory, pub name: String, pub bytes: Vec<u8> }
pub(crate) struct BackupArchive { pub manifest: BackupManifest, pub entries: Vec<BackupEntry> }
pub(crate) struct BackupManifest { pub version: u32, pub categories: Vec<BackupCategory> }
// BackupManifest.device 字段不迁(spec 不采纳点 7:设备名标识随远端推送砍、无本地消费方)

fn validate_passphrase(passphrase: &str) -> Result<(), String>;  // chars().count() >= 8
fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String>;
// Argon2id v0x13,m=19456 KiB/t=2/p=1,Some(KEY_LEN)——参数组合作契约数值随迁
fn encrypt_bytes(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String>;
// getrandom 取盐与 nonce;Payload { msg, aad: MAGIC }——格式身份与密文认证绑死;
// 派生密钥 Zeroizing;输出 = MAGIC|salt|nonce|ciphertext
fn decrypt_bytes(packet: &[u8], passphrase: &str) -> Result<Zeroizing<Vec<u8>>, String>;
// 先验魔数再派生——魔数不符即「非本应用备份或版本不支持」,一票拒解
pub(crate) fn seal(archive: &BackupArchive, passphrase: &str) -> Result<Vec<u8>, String>;
pub(crate) fn open(packet: &[u8], passphrase: &str) -> Result<BackupArchive, String>;
pub(crate) fn collect(selection: BackupSelection) -> Result<BackupArchive, String>;
fn collect_from(root: &Path, selection: BackupSelection) -> Result<BackupArchive, String>;
// 白名单即边界:只认 data_dir 固定文件清单,不遍历 home;清单归 D10-2
fn filter_settings(bytes: Vec<u8>) -> Vec<u8>;      // 导出侧敏感键剥除(pebrel 剥 ssh_proxy_*/
//   pinned_hosts/saved_hosts/hidden_hosts——字面量随 SSH 砍,双闸抽象保留归 D10-2 套
//   slterm 自有敏感文件)
fn safe_path(root: &Path, name: &str) -> Result<PathBuf, String>;
// 空名/绝对路径/非 Normal 分量(含 ..)→ 拒
fn restore_path(root: &Path, name: &str) -> Result<PathBuf, String>;
// 逐分量核查父链:命中符号链接或非目录即拒
fn validate_archive(archive: &BackupArchive) -> Result<(), String>;
// 版本号/类别集合无重复/逐条 safe_path/按类别精确文件名白名单/JSON 可解析项/
// 重复条目名拒——任何一条不过整单拒写,先于任何落盘动作
pub(crate) fn restore(packet: &[u8], passphrase: &str) -> Result<(), String>;
// 先 open(完整鉴权)再 restore_to;restore_to 内 atomic_file::write 原子落盘归 05/06 共有件

// slterm_app/src/encrypted_backup/recovery.rs(选择性恢复 + 加密恢复点撤回,全链照抄)
pub(crate) struct RestorePoint { pub path: PathBuf, names: Vec<String> }
// UI 只持句柄,不碰写入规则;names 私有 = 目标集合不可被外部篡改
pub(crate) fn restore_selected(archive: &BackupArchive, selection: BackupSelection,
    passphrase: &str) -> Result<RestorePoint, String>;
// 先采原字节与「不存在」状态 → 本机加密落盘恢复点(encrypt_bytes,口令派生) →
// restore_to 失败自动回滚(write_originals);恢复点只在本机加密落盘,不经会过滤
// 凭据字段的可分享收集器(注释承载的因果随迁)
pub(crate) fn undo(point: &RestorePoint, passphrase: &str) -> Result<(), String>;
// undo_at:解密恢复点 → 目标名集合必须与该次已验证恢复一致(point.names 私有
// 只读)——外部导入文件不能借撤回扩展白名单(注释承载的因果随迁)
// 恢复文件命名:backup-recovery/<ts>.slterm-recovery(pebrel .pebrel-recovery 改名归 D 节)
```

### plan_balance 移植签名(自有保留,查询语义原样归 09 消费归本篇锚定)

```rust
// slterm_app/src/plan_balance/mod.rs —— 套餐余量模块(F10 语义原样)
// 模块头红线注释逐字随迁:「token 不出凭据域——DTO 无 token 字段;本模块
// tracing/错误消息禁止插值 token 与 Authorization 头」
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanBalanceInfo {   // 快照/DTO 类型:六键精确集,无 token 字段(类型层防线)
    pub source_id: String,     // v1 恒 "claude"
    pub plan_id: String,       // "deepseek" | "kimi"
    pub frozen: bool,          // kimi 月限额触顶
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
    pub updated_at: u64,       // 最近成功查询 unix 秒;0 = 尚无成功值(渲染 --)
}
pub struct AmountInfo { pub value: String, pub currency: String }     // 原样透传 total_balance
pub struct WindowsInfo { pub five_hour: WindowInfo, pub seven_day: WindowInfo }
pub struct WindowInfo { pub used_percent: u8, pub resets_at: Option<String> }
// used_percent 口径:used 优先(used/limit×100)、remaining 换算回退((limit−remaining)/limit×100),
// .round() + clamp 0–100;used 缺失经 remaining 换算回退可行恰因「remaining 恒在」实证
pub struct FetchOutcome { pub frozen: bool, pub amount: Option<AmountInfo>,
                          pub windows: Option<WindowsInfo> }   // 查询产出,无 source_id/plan_id/updated_at

pub(crate) fn merge_slot(old: Option<&PlanBalanceInfo>, source_id: &str, plan_id: &str,
    result: Result<FetchOutcome, AppError>, now: u64) -> PlanBalanceInfo;
// 快照槽合并四分支:成功采用新值(updated_at=now);失败保留同 planId 旧值(含旧
// updated_at 不刷新);planId 变化丢弃旧值;失败且无旧值 → 占位(updated_at=0)
pub(crate) fn poll_once_with(old: &[PlanBalanceInfo],
    resolve: impl Fn(&'static dyn PlanSource) -> Option<(String, SecretBytes)>,  // ← 补强:token 句柄出 resolve
    fetch: impl Fn(&'static dyn PlanQuery, &SecretBytes) -> Result<FetchOutcome, AppError>,
    now: u64) -> Vec<PlanBalanceInfo>;
// 一轮拉取编排(D6 最小可测性):来源消失 → 本轮从数组移除;未命中套餐 → 静默降级;
// emit 判定在 apply_snapshot——快照整体 PartialEq(含 updated_at)有变化才 emit
fn apply_snapshot(/* 单进程形态归改造节:订阅模型替换 tauri emit,签名归 D10-3 */);
pub fn poll_once_executor(/* GPUI 后台执行器形态归改造节 */);

// slterm_app/src/plan_balance/source.rs —— 余量来源(双注册表之一)
pub trait PlanSource: Send + Sync + std::fmt::Debug {
    fn source_id(&self) -> &'static str;          // DTO source_id,按注册序 emit
    fn resolve(&self) -> Option<(String, SecretBytes)>;  // ← 补强:出 (baseUrl, token 句柄)
}
// 静态切片注册表(照 hooks provider.rs 先例,偏离 #13 可变单例——Rust 无 side-effect
// import,测试经参数化注入;「一行注册」目标不变):
pub(crate) static SOURCES: &[&dyn PlanSource] = &[&CLAUDE_USER_SOURCE];
// ClaudeUserSettingsSource:读 user 层 ~/.claude/settings.json env 段
// (ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN);文件缺失/env 缺失/token 空 → None 静默降级
fn resolve_env(content: &str) -> Option<(String, SecretBytes)>;  // 纯函数,罐装 JSON 全测

// slterm_app/src/plan_balance/query.rs —— 套餐查询注册表(双注册表之二)
pub trait PlanQuery: Send + Sync + std::fmt::Debug {
    fn plan_id(&self) -> &'static str;
    fn base_urls(&self) -> &'static [&'static str];  // 元素须为已归一化形态:小写、无尾斜杠
    fn fetch(&self, token: &SecretBytes) -> Result<FetchOutcome, AppError>;
    // 阻塞 HTTP 查询(调用方负责后台执行);错误消息禁止含 token/Authorization(红线)
}
pub(crate) static QUERIES: &[&dyn PlanQuery] = &[&DEEPSEEK, &KIMI];
pub(crate) fn normalize_base_url(url: &str) -> String;  // 规格字面:仅小写化 + 去尾斜杠,不加 trim
pub(crate) fn find_query_by_url<'a>(base_url: &str, queries: &'a [&'a dyn PlanQuery])
    -> Option<&'a dyn PlanQuery>;                        // 参数化查找供 L1 注入
pub(crate) fn http_agent(timeout: Duration) -> ureq::Agent;  // timeout_global 覆盖全程
pub(crate) fn query_err(plan_id: &str, e: ureq::Error) -> AppError;
// 消息只含 planId + 错误类别(HTTP {code}/超时/网络错误),禁止拼 token

// slterm_app/src/plan_balance/deepseek.rs —— DeepSeekQuery(TIMEOUT 5s)
// base_urls = ["https://api.deepseek.com/anthropic"];parse_deepseek_balance(body) 纯函数
// slterm_app/src/plan_balance/kimi.rs —— KimiQuery(TIMEOUT 8s)
// base_urls = ["https://api.kimi.com/coding"];GET /coding/v1/usages + Authorization Bearer
// (非 X-Kimi-Authorization——2026-08 实证修正);parse_kimi_usages(body) 纯函数:
//   冻结判定:totalQuota.remaining 字符串 parse ≤ 0 → frozen(不要求窗口解析成功);
//   totalQuota 无 used 字段(实测可为空对象 {});5h 窗数值承载于 limits[i].detail
//   内层(外层无),7d 窗为顶层 usage 对象;remaining 恒在、used 可缺;数值字段按
//   字符串解析(若 API 返回数字形态须先实测再放宽);全有或全无:非冻结时任一窗口
//   解析失败 → 整体 Err;真实响应快照锚点 parse_real_response_snapshot 随迁(防
//   API 漂移回归)
```

### 安全审计签名（SEC-12/SEC-13 归 03 篇安装器；本篇只锚审计通道与凭据纪律）

```rust
// slterm_app/src/ai_hook/local/(归 03 篇):SEC-12 审查并入其安装器,
// 本篇只锚定审计通道形态与凭据纪律。
pub(crate) fn review_statusline_command(raw: &str) -> InstallGate;   // 定义归 03 篇
// 命中词表(curl/wget/Invoke-Expression/iex 等,词表演化归 03 篇 D03-2)→
// InstallGate::Suspended { suspicious_command } + 审计 tracing::warn!(target: "audit");
// 确认路径归 03 篇 confirm_install,审计事件走本篇裁定的通道、由 03 落盘。
// SEC-13 哈希对账归 03 篇注入状态三态(磁盘脚本 vs 内嵌模板 SHA-256 比对,
// sha256_digest/template_sha256 形态归 03);SEC-17 user 层写入审计归本篇通道:
tracing::warn!(target: "audit", "hooks user 层配置写入: {}", path.display());
// ——写入路径归 03 篇、设置写通道归 06 篇;审计通道 = 本篇裁定的唯一形态,03 消费。
```

## 数据流与状态机

### provider 凭据生命周期(无凭据 → 已存凭据 → 已删除)

```
存(首次配 key):key 入参 → store_provider_api_key → save_api_key
  → credential_target("Slterm/AI/<id>") → secrets.rs 门面 → OS 凭据库
  → 回写 api_key_set=true + api_key_hint(尾四) → 调用方清写稿
  任一步 IO 失败 → 引用字段不动,无中间态残留
取(出境/连通测试):load_api_key → SecretBytes 句柄出域
  → 消费点内联拼 "Bearer {key}"(句柄内构造)→ drop 即焚
删:remove_provider → delete_api_key 成功 → 才动 store 元数据
  失败停在删凭据前,不产生孤儿密钥
不变量:明文永不过 providers.json;hint 不可还原明文;任何 UI 层零接触明文
(凭据查找内聚在 test_provider / 出境请求体内部)
```

### 备份与恢复(seal / open+validate / restore / undo 全链)

```
备份:BackupSelection → collect(白名单收集,导出侧敏感键剥除)
  → seal(manifest + entries → MAGIC|salt|nonce|ciphertext;派生密钥 Zeroizing)
  口令与派生密钥不过磁盘
整单恢复:packet → decrypt_bytes 先验 MAGIC 再派生(不符即一票拒)
  → open 完整鉴权 → validate_archive 全量校验(版本/类别无重/逐条
  safe_path/按类别文件名白名单/JSON 可解析/重名拒)
  → 任何一条不过整单拒写,先于任何落盘 → restore_to 原子写
选择性恢复:restore_selected = 先采原字节与「不存在」状态 → 本机加密
  落盘恢复点(encrypt_bytes 口令派生)→ restore_to → 失败 write_originals
  自动回滚
撤回:undo → 解密恢复点 → point.names 私有只读比对(外部导入文件
  不能借撤回扩白名单)→ write_originals
不变量:半份不可信报文零副作用;恢复点只本机加密、不经可分享收集器;
导入导出同一道白名单,清单外不存在「顺手带上」通道
```

### plan_balance 轮询(快照槽四态 + emit 判定)

```
快照槽四态:占位(updated_at=0,渲染 --)/ 有效(updated_at=now)/
  保留旧值(查询失败,updated_at 不刷新)/ 移除(来源消失,本轮出数组)
一轮 poll_once_with:resolve(source) → None → 本轮移除;
  find_query_by_url → None → 静默降级;命中 → fetch(token 句柄借用)
  → merge_slot(old, source_id, plan_id, result, now) 四分支
apply_snapshot:新快照 vs SNAPSHOT 整体 PartialEq(含 updated_at)→
  变化才推送;失败保留旧值不动不推;单进程推送形态归 D10-3
不变量:token 生命周期闭在「resolve 产出 → fetch 消费 → drop」内;
快照/DTO/推送载荷无 token 字段(类型层防线,编译期拒)
```

### 审计三通道(SEC-12 / SEC-13 / SEC-17,审计通道 = tracing target "audit")

```
SEC-12:写盘前审查 review_statusline_command → 命中 → Suspended 零写盘
  → GPUI 模态展示原文 → 用户确认经 confirm 路径(跳过审查)完成注入
  → 全程 warn!(target:"audit")(定义归 03,通道归本篇)
SEC-13:启动对账 → 磁盘脚本 vs 内嵌模板 SHA-256 比对 → 只补缺失不覆盖;
  磁盘被替换 → Outdated 提示 + 审计(注入状态归 03 HookInspection)
SEC-17:hooks user 层 settings.json 写入 → warn!(target:"audit", path)
  (写入路径归 03,写通道归 06,通道归本篇)
不变量:审计消息只含路径/命令类别,禁含凭据值;确认交互唯一面 = GPUI 原生模态
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

> 薄写:每条对应 spec 分片 10 一个采纳点,一行。采纳点编号沿用 spec 分片 10。

### 凭据三件套(spec 分片 10 采纳点 1-7、16-18)

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 1 | `nebula_app/src/ai_providers.rs` 模块头「文件边界即凭据边界」注释 + `STORE_FILE`/`store_path`/`load`/`save` | `slterm_app/src/ai_providers.rs`(eg 08 元数据归位归 08) | 凭据需稳定身份,绝不与普通设置同文件序列化 |
| 2 | `ai_providers.rs::AiProvider` 的 `api_key_set`/`api_key_hint` 引用字段 | 归 08 结构体归本篇字段语义锚点归 08 消费归本篇锚定 | 明文永不入 store,store 可整份展示/备份/diff |
| 3 | `ai_providers.rs::api_key_hint`(掩码 + `key_hint_never_contains_the_full_secret` 测试) | 归 08 消费归本篇锚定 | 可辨识指纹与全量绝不见面是同一份代码两面 |
| 4 | `ai_providers.rs::credential_target`(`Pebrel/AI/<id>` → `Slterm/AI/<id>`) | 归 01 篇改名映射表归位归本篇锚定归 08 消费 | target 命名空间归调用方,凭据库可多域复用 |
| 5 | `ai_providers.rs::store_provider_api_key`/`save_api_key` 写即清稿纪律 + doc comment | 归 08 结构体归本篇凭据纪律归 08 消费归本篇锚点 | 凭据明文单行生命周期:过手即入库即弃 |
| 6 | `ai_providers.rs::remove_provider` 先删凭据再删元数据删除序 | 归 08 消费归本篇锚定 | 失败停在删凭据前,不产生孤儿密钥 |
| 7 | `ai_providers.rs::test_provider` 凭据内聚查找 + Zeroizing 全程 | 归 08 枚举归本篇凭据纪律归 08 消费归本篇锚点 | 明文生命周期闭在后台执行体内部,UI 零接触 |
| 16 | `nebula_app/src/platform/credentials.rs` `load`/`store`/`store_with_username`/`delete` + `CredentialIdentity` 单态 | `slterm_app/src/platform/credentials.rs` + `windows.rs` | 封装层与 Win32 分离,调用方只给 target 与明文 |
| 17 | `nebula_app/src/ssh_credentials.rs::windows_store::{load_secret,save_secret,delete_secret}` | 解绑改名 `credentials/windows.rs`,`pub(crate)` | SSH 砍但凭据原语保留;缓冲区全 `fill(0)` 纪律随迁 |
| 18 | `ssh_credentials.rs::{store_generic_secret,load_generic_secret,delete_generic_secret}` 三原语门面 + doc comment | 搬迁 `slterm_app/src/secrets.rs`(本篇新建) | 非 SSH 消费凭据管理器唯一入口,禁旁路直调 Win32 |

### 加密备份(spec 分片 10 采纳点 8-15)

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 8 | `nebula_app/src/encrypted_backup.rs::derive_key`/`encrypt_bytes`/`decrypt_bytes` | `slterm_app/src/encrypted_backup.rs` 全量 | AES-256-GCM + Argon2id 算法栈完整,参数组作契约数值随迁 |
| 9 | `encrypted_backup.rs::MAGIC`(PEBRBAK1 作 `Payload::aad`) | 魔数归 D10-1(默认 `SLTRMBK1`) | 格式身份与密文认证绑死,不符一票拒解 |
| 10 | `encrypted_backup.rs` 模块头白名单注释 + `collect_from` | 白名单清单归 D10-2;不遍历 home 照抄 | 白名单即安全边界,枚举即授权 |
| 11 | `filter_settings`/`add_sanitized_ssh`/`ssh_is_sanitized` 双闸 | SSH 字面量砍;双闸抽象归 D10-2 套 slterm 自有敏感文件 | 导出清洗 + 导入复验,防敏感面进出 |
| 12 | `encrypted_backup.rs::validate_archive` | 全量 | 先验后写:任何不过整单拒写,先于任何落盘 |
| 13 | `encrypted_backup.rs::restore`/`restore_to`/`restore_path` + `atomic_file::write` | 全量;原子写归 05/06 共有件归 01 提取 | 先鉴权后落盘,符号链接父链核查 |
| 14 | `encrypted_backup/recovery.rs` `RestorePoint`/`restore_selected`/`undo` | `slterm_app/src/encrypted_backup/recovery.rs` 全量 | 恢复可逆:加密恢复点 + 失败回滚 + 口令核对撤回 |
| 15 | `BackupSelection`/`BackupCategory` 机制族 | 机制照抄;类别枚举归 M10 归 D10-2 | 分类选择机制与按类别校验照抄,清单重定 |

### 改名映射引用

本篇一切改名以 01 篇「改造 / 移植 / 新建设计」节**改名映射单点表**为唯一权威,本篇不另立映射。

## 改造 / 移植 / 新建设计

### 1. plan_balance 移植落位与执行面改造

模块落位照「模块终态」表:`slterm_app/src/plan_balance/`(mod/source/query/deepseek/kimi)自 `src-tauri/src/plan_balance/` git 历史提取,eg 01 篇提取清单;查询语义零改动。执行面改造三件:

| 件 | 旧形态 | 新形态 |
| --- | --- | --- |
| 轮询载体 | tauri poller + 前端 interval 双端调度 | GPUI background executor;executor 体归本篇 |
| 节拍与生命周期 | 旧后端 `background_tasks` interval 调度器 | 09 篇 `background_tasks` 注册表 `planBalance` 条目归 09 锚点归本篇消费 |
| 推送 | `apply_snapshot` tauri emit | 订阅模型归 D10-3;模块级 `SNAPSHOT` 静态存储形态不变 |

`poll_once_executor` 体(本篇锚点)= 旧 `poll_once_production(now)` 实参化:`poll_once_with(&SNAPSHOT, |s| s.resolve(), |q, t| q.fetch(t), now)` → `apply_snapshot(new)`。签名从 `fn(tauri::AppHandle)` 收敛为 `fn()`(eg 09 篇 `TaskExecutor` 消费归 09);阻塞 HTTP 全程在后台执行器,不阻塞 UI 线程。

`apply_snapshot` 实现期形态归 D10-3,emit 判定语义不变:快照整体 PartialEq(含 updated_at)有变化才推,失败保留旧值不推。`refresh_plan_balance` 恒返回 Ok 语义原样——轮询失败保留旧值,错误不冒泡到 UI 线程。

### 2. SEC 清单二分归档表

旧 SEC 清单按「OS 面保留 / IPC-webview 面消亡」二分归档,消亡项随 Tauri 栈 M0 全删自然灭失、不设替代:

| 面 | 项 | 归宿 |
| --- | --- | --- |
| OS 面保留 | SEC-12 statusline 审查门 | 03 篇安装器(ensure_claude_hooks 前置);审计通道本篇锚定 |
| OS 面保留 | SEC-13 Outdated 防篡改(注入哈希对账) | 03 篇注入状态三态;审计通道本篇锚定 |
| OS 面保留 | SEC-17 user 层配置写入审计 | 本篇通道(tracing target "audit");写入路径归 03/06 |
| OS 面保留 | SEC-18 git 追踪文件凭据红线 | 本篇纪律,扩展至新凭据面(providers/备份/runTime.port token) |
| IPC-webview 面消亡 | 原 IPC 攻击面族(命令注入/参数穿透/webview XSS 桥) | 随单进程消亡——无 IPC 即无该面,不重建不登记豁免 |

### 3. SEC-13 / SEC-17 新形态落位

- **SEC-13(Outdated 防篡改)**:磁盘 hook 脚本 vs 安装器内嵌模板 SHA-256 比对,三态(一致/被改/缺失)判定与 `sha256_digest`/`template_sha256` 字段形态全部归 03 篇安装器;本篇只锚审计事件格式——对账结果落 `tracing::warn!(target: "audit", ...)`,字段含路径/期望/实测哈希,不插值任何凭据。
- **SEC-17(user 层写入审计)**:任何写 user 层 `~/.claude/settings.json` 的路径(03 篇安装器、06 篇设置写通道)必须经本篇审计包装函数;审计记录写路径+来源模块,不记内容值(防凭据进日志)。
- 审计通道唯一形态:`tracing` target `"audit"`,订阅器装配归 09 篇日志初始化;运行期落盘策略(文件轮转/仅 stderr)归实现期,测试期以 `tracing::subscriber` 捕获断言。

### 4. 远端推送不建(边界声明)

不建任何远端凭据/备份推送面(WebDAV/S3/SFTP 等 spec 不采纳项);加密备份产物只落本地用户目录,移动与异地容灾由用户自理。凭据域出网面唯一例外 = plan_balance 查询请求本身(HTTPS 至 provider 端点,token 仅存在于请求头组装瞬时栈帧,响应解析后立即 Zeroizing)。

### 5. 缝合契约

| 对象篇 | 契约 |
| --- | --- |
| 08(ai_providers 消费) | 凭据读写只经本篇 `secrets.rs` 门面;`fallback_api_key` 归 08 配置文件、本篇不管;providers.json 形态归 08 |
| 06(设置) | 设置 GUI 只展示凭据引用字段+尾四掩码,永不取明文;写通道原子写底座归 06,本篇凭据文件独立不走该通道 |
| 04(runtime API) | 控制面不暴露任何凭据读/写/列方法;runtime.port 的 128bit token 本身按本篇凭据纪律存取(Zeroizing、日志不插值) |
| 09(系统集成) | `background_tasks` 注册表挂 `planBalance` 节拍(体归本篇);日志/审计 subscriber 装配归 09;加密备份 UI 入口落位归 D10-4 |
| 03(hooks 安装器) | SEC-12/13 判定逻辑归 03,审计事件格式与通道归本篇 |

## 测试点清单

测试基础设施形态归 11 篇;以下为本领域用例点:

- **凭据域边界(类型层)**:`SecretBytes` 出域面静态审查测试——凭据域外模块的公开签名扫描,断言无 `String/&str/Vec<u8>` 裸 token 载体;域内 `as_bytes` 借出仅限白名单调用点(请求头组装)。
- **凭据域边界(日志)**:全测试路径挂 tracing 捕获层,断言任何凭据假值(`sk-test` 形态)不出现在任何 target 日志;audit target 记录只含路径/哈希,不含值。
- **凭据三件套**:Windows 凭据管理器存/取/删往返(假值);尾四掩码格式;写即清稿(写后内存缓冲已 zeroize);独立文件权限位。
- **加密备份**:seal→open 往返逐类别;MAGIC 错/AAD 错/口令错/密文改一字节 → 拒绝且不写盘;先验后写(校验失败时目标文件不变);恢复点撤回(undo 集合与 point.names 私有只读比对);口令 <8 拒绝。
- **SEC-13 审计**:磁盘脚本被改 → 三态判定 + audit 事件字段断言(归 03 判定、本篇断事件格式)。
- **SEC-17 审计**:user 层 settings.json 写入触发 audit 事件;绕过审计包装的直接写 → 无(包装为唯一写路径,编译期守)。
- **plan_balance 防复发(旧资产用例全迁)**:kimi 实证红线各例(冻结判定/totalQuota 无 used/5h 窗 detail 内层/remaining 恒在/字符串解析/全有或全无);merge_slot 四分支;emit 判定(含 updated_at 整体相等不推、变化才推、失败保留旧值不推);`refresh_plan_balance` 恒 Ok;`parse_real_response_snapshot` 锚点例。
- **双通道裁定**:plan_balance 只读 user 层 settings.json 的源测试(不读 Windows 凭据管理器);ai_providers 不读 user 层文件。

## 阶段归属与出口标准

对照 00-roadmap M10 安全半边(系统集成半边 M10.1-M10.8 归 09 篇),本篇子步骤:

| 子步 | 内容 | 出口标准(可机验) |
| --- | --- | --- |
| M10.9 | 凭据域底座(SecretBytes + platform/credentials 原语 + secrets 门面) | 凭据域边界两测试(类型层+日志)绿;三件套往返测试绿 |
| M10.10 | 加密备份(seal/open/restore/undo) | 备份往返与篡改拒绝测试全绿 |
| M10.11 | plan_balance 移植(查询语义原样 + GPUI executor 执行面) | plan_balance 防复发用例全绿;executor 体在后台线程断言绿 |
| M10.12 | SEC-13/17 审计通道 + 双通道裁定守卫 | 审计事件格式断言绿;双通道源测试绿;`cargo test` 全绿 |

## 待沉淀决策

> [待沉淀] **D10-1 备份魔数最终形态**。默认 `"SLTRMBK1"`(8 字节,作 AAD 绑文件来源)。真实权衡:带品牌魔数可辨识来源 vs 去品牌未来改名零成本(承接 01 篇 D3)。难逆点:魔数随首份备份产物固化,改 = 旧备份不可读需迁移工具。截止:M10.10 开工前定。

> [待沉淀] **D10-2 备份类别清单与敏感键剥除字面量重定**。pebrel 白名单类别照抄,但 slTerminal 键域(JSON 化后)与 pebrel 不同,类别清单与「敏感键剥除」字面量表须按 06 篇键域终态重定。难逆点:清单即备份边界,漏类别 = 恢复后功能缺损,错纳敏感键 = 凭据出域事故。截止:M10.10 开工前(依赖 06 篇键域定稿)。

> [待沉淀] **D10-3 plan_balance 单进程订阅/推送形态**。旧 tauri emit + 前端订阅消亡;候选:GPUI `Context::notify` 刷新绑定组件 / 模块级 watch channel 供多消费者。真实权衡:notify 简单但耦合组件树,channel 通用但多一层间接。难逆点:消费面铺开后换机制 = 跨组件改动。截止:M10.11 开工前定。

> [待沉淀] **D10-4 备份 UI 入口落位**。pebrel 备份入口在其设置页;slTerminal 侧候选:06 篇 GUI 设置页安全分区 / 09 篇托盘菜单。真实权衡:设置页符合心智 vs 托盘更可达。难逆点:低,但涉及两篇缝合面。截止:M10.10 开工前定。

## 开放问题

1. **审计落盘策略**:audit target 运行期是否落独立轮转文件(便于事后取证)还是仅随主日志——归实现期,影响 SEC-17 取证能力。
2. **恢复点清单 UI 形态**:undo 需要向用户展示恢复点列表(point.names 私有只读),展示面归 06 设置页还是独立对话框,随 D10-4 联动定。
3. **plan_balance 旧便携数据去向**:旧版 settings.json(exe 同级)中的 plan 快照是否随 01 篇 D2/D3 数据目录裁定一并迁移,待 D2/D3 裁决后回登。
