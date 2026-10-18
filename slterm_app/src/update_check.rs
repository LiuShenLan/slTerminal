//! 启动/手动更新检查(GitHub Releases,09 篇 M10.5 采纳点 23–26)。
//!
//! 全程 best-effort:断网、坏 JSON、GitHub 限流一律落 debug 日志,不弹错、
//! 不阻塞 UI。启动检查是后台任务注册表的 12s 一次性任务,等首窗与首个
//! 会话安顿后再联网;立即段(水合/缓存提示)不受延迟影响。
//!
//! 版本与资产纪律(下载器不信任 UI 数据):
//! - 版本比较是手写点分段数值比较,段内后缀忽略;拒绝降级、等版本不提示;
//! - 资产 = 名称/架构/版本/官方 URL 前缀四重精确匹配;
//! - digest 字段优先,release body 内嵌 checksum 回退(`SHA256SUMS` 同源);
//! - REST 403/429 才走官方 latest release 页面的有界回退(因果链见
//!   `fallback.rs`;嵌 token/猜包 URL/把 API 失败当最新三件均被拒绝)。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub(crate) mod assets;
mod fallback;
#[cfg(any(test, feature = "update-test-source"))]
pub(crate) mod test_source;

/// GitHub Releases 坐标(12 篇发布面登记值;本模块只消费)。
pub(crate) const RELEASES_API: &str =
    "https://api.github.com/repos/LiuShenLan/slTerminal/releases/latest";
/// 手动下载页;消费点 = 更新弹窗的「前往下载」与无可安装资产时的提示。
#[allow(dead_code)]
pub(crate) const RELEASES_PAGE: &str = "https://github.com/LiuShenLan/slTerminal/releases";
/// 安装包下载前缀;资产合同与检查/下载两侧共享同一常量。
pub(crate) const RELEASE_DOWNLOAD_PREFIX: &str =
    "https://github.com/LiuShenLan/slTerminal/releases/download/";

const UPDATE_STATE_FILE: &str = "update_state.json";
/// 「稍后提醒」窗口:3 天(采纳点 24 契约值照抄上游)。
const REMIND_LATER_SECS: u64 = 3 * 24 * 60 * 60;
/// release JSON 正文上限:正常响应远小于此,防御异常大响应。
const RELEASE_BODY_LIMIT: u64 = 2 * 1024 * 1024;
/// 首个 Inno 安装基版本(D12-4):更早的历史安装(Tauri 时代)不构成
/// 可更新对象,更新链只提示「需手动重装」,绝不自动下载/安装。
pub(crate) const FIRST_INSTALLER_VERSION: &str = "0.3.0";

/// 进程内提示状态写锁:与文件锁组成双闸(同进程串行 + 跨进程互斥)。
static UPDATE_STATE_LOCK: Mutex<()> = Mutex::new(());

/// 资产四重校验后的可信形态。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct UpdateAsset {
    pub version: String,
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
    /// SHA-256 小写 hex;缺失时下载器拒绝下载(无法验证的安装包不执行)。
    pub digest: Option<String>,
}

/// 检查完整结果:启动检查与手动检查共用同一版本/资产/校验元数据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UpdateCheckResult {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    /// 当前安装早于首个 Inno 安装基:`asset` 即使存在也不得自动安装,
    /// 提示文案改为「需手动重装」。
    pub manual_reinstall: bool,
    pub asset: Option<UpdateAsset>,
}

#[derive(Debug)]
pub(super) struct LatestRelease {
    pub(super) version: String,
    pub(super) asset: Option<UpdateAsset>,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubReleaseAsset>,
    /// 防御性渠道过滤:API 的 latest 端点本不返回预发布/草稿,但坏数据源
    /// 或镜像不得让预发布包进入安装链。
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

#[derive(Deserialize)]
pub(super) struct GitHubReleaseAsset {
    pub(super) name: String,
    pub(super) browser_download_url: String,
    #[serde(default)]
    pub(super) size: u64,
    #[serde(default)]
    pub(super) digest: Option<String>,
}

/// 按版本生效的提示状态(`update_state.json`,与用户设置正文解耦)。
///
/// 后台检查绝不改写设置文件;新版本不继承旧版本的延迟或跳过选择。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct UpdatePromptState {
    #[serde(default)]
    prompted: HashSet<String>,
    #[serde(default)]
    skipped: HashSet<String>,
    /// 最近一次「稍后提醒」的到期秒级时间戳(只对该版本有效;新版本未
    /// 出现在 prompted 集合时直接可提示)。
    #[serde(default)]
    remind_after: Option<u64>,
}

impl UpdatePromptState {
    fn should_prompt(&self, version: &str, now: u64) -> bool {
        if self.skipped.contains(version) {
            return false;
        }
        if !self.prompted.contains(version) {
            return true;
        }
        self.remind_after.is_some_and(|deadline| now >= deadline)
    }

    fn mark_prompted(&mut self, version: &str) {
        self.prompted.insert(version.to_owned());
        self.remind_after = None;
    }

    fn remind_later(&mut self, version: &str, now: u64) {
        self.prompted.insert(version.to_owned());
        // 「稍后提醒」重新启用该版本(撤销同版本 skip)。
        self.skipped.remove(version);
        self.remind_after = Some(now.saturating_add(REMIND_LATER_SECS));
    }

    fn skip(&mut self, version: &str) {
        self.skipped.insert(version.to_owned());
        self.prompted.insert(version.to_owned());
        self.remind_after = None;
    }
}

/// 启动检查入口(GPUI background executor 载体):结果经回调送回壳事件
/// 队列(检查逻辑不依赖壳类型)。
///
/// 注册表 `autoUpdateCheck` 是 Once 任务:订阅 + 重入闸门保证一次进程只跑
/// 一轮,12s 延迟取注册表契约值(不另立常量)。顺序保持原启动线程语义:
/// hydrate 双证据恢复 → 缓存资产可装/失败未见过 → 提示 →
/// `auto_check_updates` 闸门 → 延迟 → 联网检查。外部托管渠道直接退出。
pub(crate) fn start_gpui(
    cx: &mut gpui::App,
    mut deliver: impl FnMut(UpdateCheckResult) + Send + 'static,
) {
    use gpui::AppContext as _;

    if crate::platform::distribution::current().externally_managed() {
        return;
    }
    let Some(subscription) =
        crate::background_tasks::subscribe(crate::background_tasks::AUTO_UPDATE_CHECK)
    else {
        return;
    };
    let delay = crate::background_tasks::startup_delay(crate::background_tasks::AUTO_UPDATE_CHECK);
    let executor = cx.background_executor().clone();
    cx.background_spawn(async move {
        // 订阅与一次性闸门覆盖立即段和延迟段;失败/退出后不再重跑。
        let _subscription = subscription;
        let Some(_run) = crate::background_tasks::begin(crate::background_tasks::AUTO_UPDATE_CHECK)
        else {
            return;
        };
        // 无网启动也能恢复上次就绪/失败态(cache + handoff 双证据)。
        crate::update_download::hydrate();
        deliver_cached(&mut deliver);
        if !slterm_settings::RuntimeSettings::load().auto_check_updates {
            return;
        }
        executor.timer(delay).await;
        deliver_online(&mut deliver);
    })
    .detach();
}

/// 立即段:上次会话留下的缓存资产/失败态可直接提示,不必等联网。
fn deliver_cached(deliver: &mut impl FnMut(UpdateCheckResult)) {
    let Some(asset) = crate::update_download::cached_asset() else {
        return;
    };
    let failed = matches!(
        crate::update_download::status(&asset),
        crate::update_download::DownloadStatus::InstallFailed(_)
    );
    if (failed || can_install_version(&asset.version).unwrap_or(false))
        && (should_prompt(&asset.version)
            || (failed
                && crate::update_download::installation_failure_unseen(&update_state_path())))
    {
        deliver(UpdateCheckResult {
            current: env!("CARGO_PKG_VERSION").into(),
            latest: asset.version.clone(),
            update_available: true,
            manual_reinstall: requires_manual_reinstall(env!("CARGO_PKG_VERSION")),
            asset: Some(asset),
        });
    }
}

/// 延迟段:联网检查;全部失败只落 debug 日志。
fn deliver_online(deliver: &mut impl FnMut(UpdateCheckResult)) {
    let result = match check_now() {
        Ok(result) => result,
        Err(error) => {
            log::debug!("update-check: {error}");
            return;
        }
    };
    if !result.update_available {
        log::debug!(
            "update-check: v{} is current (latest v{})",
            result.current,
            result.latest
        );
        return;
    }
    if !should_prompt(&result.latest) {
        log::debug!(
            "update-check: automatic prompt suppressed for v{}",
            result.latest
        );
        return;
    }
    deliver(result);
}

/// 立即检查 GitHub 最新 release;调用方必须放到后台执行,避免网络等待
/// 阻塞 UI 线程(设置页手动检查消费点)。
pub(crate) fn check_now() -> Result<UpdateCheckResult, String> {
    crate::platform::distribution::require_direct_update()?;
    let release = fetch_latest_release()?;
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let update_available = can_install_version(&release.version)?;
    Ok(UpdateCheckResult {
        update_available,
        manual_reinstall: requires_manual_reinstall(&current),
        current,
        latest: release.version,
        asset: release.asset,
    })
}

/// 假 release 服务器演练/单元测试入口:显式 URL,永不回落公网。
/// 消费点 = 全链演练用例;feature 构建下由人工彩排脚本调用。
#[allow(dead_code)]
#[cfg(any(test, feature = "update-test-source"))]
pub(crate) fn check_from(agent: &ureq::Agent, url: &str) -> Result<UpdateCheckResult, String> {
    let release = fetch_release_with_agent(agent, url)?;
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let update_available = can_install_version(&release.version)?;
    Ok(UpdateCheckResult {
        update_available,
        manual_reinstall: requires_manual_reinstall(&current),
        current,
        latest: release.version,
        asset: release.asset,
    })
}

fn update_state_path() -> PathBuf {
    update_state_path_at(crate::platform::dirs::data_dir())
}

fn update_state_path_at(root: &Path) -> PathBuf {
    root.join(UPDATE_STATE_FILE)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn load_prompt_state_at(path: &Path) -> UpdatePromptState {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(state) => state,
            Err(error) => {
                log::warn!("update-check: ignored invalid {}: {error}", path.display());
                UpdatePromptState::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => UpdatePromptState::default(),
        Err(error) => {
            log::warn!("update-check: failed to read {}: {error}", path.display());
            UpdatePromptState::default()
        }
    }
}

/// 双闸写入口:进程内 Mutex 串行 + 跨进程短锁;写失败/被占用返回错误,
/// 绝不半写状态文件。
fn update_prompt_state_at(
    path: &Path,
    change: impl FnOnce(&mut UpdatePromptState),
) -> Result<(), String> {
    let _guard = UPDATE_STATE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(_file_lock) = crate::atomic_file::try_lock(path)
        .map_err(|error| format!("无法锁定更新提醒状态:{error}"))?
    else {
        return Err("更新提醒状态正由另一个 slTerminal 进程写入".to_owned());
    };
    let mut state = load_prompt_state_at(path);
    change(&mut state);
    let bytes = serde_json::to_vec_pretty(&state)
        .map_err(|error| format!("无法序列化更新提醒状态:{error}"))?;
    crate::atomic_file::write(path, &bytes).map_err(|error| format!("无法保存更新提醒状态:{error}"))
}

fn should_prompt_at(path: &Path, version: &str, now: u64) -> bool {
    let _guard = UPDATE_STATE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    load_prompt_state_at(path).should_prompt(version, now)
}

pub(crate) fn should_prompt(version: &str) -> bool {
    should_prompt_at(&update_state_path(), version, now_secs())
}

/// 更新弹窗打开即记 prompted(消费者归更新提示步)。
#[allow(dead_code)]
pub(crate) fn mark_prompted(version: &str) -> Result<(), String> {
    update_prompt_state_at(&update_state_path(), |state| state.mark_prompted(version))
}

/// 「稍后提醒」:now + 3 天;只影响该版本。
#[allow(dead_code)]
pub(crate) fn remind_later(version: &str) -> Result<(), String> {
    let now = now_secs();
    update_prompt_state_at(&update_state_path(), |state| {
        state.remind_later(version, now)
    })
}

/// 跳过该版本(记录落盘;下次启动仍按 is_newer 再判,上游行为保留)。
#[allow(dead_code)]
pub(crate) fn skip_version(version: &str) -> Result<(), String> {
    update_prompt_state_at(&update_state_path(), |state| state.skip(version))
}

fn fetch_latest_release() -> Result<LatestRelease, String> {
    #[cfg(any(test, feature = "update-test-source"))]
    if let Some(origin) = test_source::origin()? {
        let url = format!("{origin}/release.json");
        let agent = test_source::agent(Duration::from_secs(10));
        return fetch_release_with_agent(&agent, &url);
    }
    let agent = crate::update_proxy::agent(RELEASES_API, Duration::from_secs(10));
    fetch_release_with_fallback(&agent, RELEASES_API, fallback::fetch_latest)
}

/// 显式本地演练/测试:只请求给定 URL,失败不回退公网。
#[cfg(any(test, feature = "update-test-source"))]
fn fetch_release_with_agent(agent: &ureq::Agent, url: &str) -> Result<LatestRelease, String> {
    fetch_release_with_fallback(agent, url, |status| Err(format!("GitHub HTTP {status}")))
}

/// 仅 REST 403/429 触发回退;其余错误原样报错,绝不把 API 失败当最新。
fn fetch_release_with_fallback(
    agent: &ureq::Agent,
    url: &str,
    on_rate_limit: impl FnOnce(u16) -> Result<LatestRelease, String>,
) -> Result<LatestRelease, String> {
    let response = agent
        .get(url)
        .header("User-Agent", "slterm")
        .header("Accept", "application/vnd.github+json")
        .call();
    let mut response = match response {
        Err(ureq::Error::StatusCode(status @ (403 | 429))) => return on_rate_limit(status),
        other => other.map_err(|error| format!("GitHub 请求失败:{error}"))?,
    };
    let bytes = response
        .body_mut()
        .with_config()
        .limit(RELEASE_BODY_LIMIT)
        .read_to_vec()
        .map_err(|error| format!("GitHub 请求失败:{error}"))?;
    parse_latest_release(&bytes)
}

fn parse_latest_release(bytes: &[u8]) -> Result<LatestRelease, String> {
    let release: GitHubRelease =
        serde_json::from_slice(bytes).map_err(|error| format!("GitHub 返回了无效数据:{error}"))?;
    if release.prerelease || release.draft {
        return Err("GitHub release 不是稳定发布渠道".to_owned());
    }
    let version = release.tag_name.trim().trim_start_matches(['v', 'V']);
    if !valid_version(version) {
        return Err("GitHub release 的版本号无效".to_owned());
    }
    let version = version.to_owned();
    let asset = assets::select(
        &version,
        release.body.as_deref().unwrap_or_default(),
        &release.assets,
        &assets::native_names(&version),
    );
    Ok(LatestRelease { version, asset })
}

/// 版本号字面量只允许安装包命名规则内的字符(防路径/查询串注入)。
fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
}

/// slTerminal 单一资产合同(09 篇改造节 7:三元组塌缩,无 legacy 品牌回退)。
pub(crate) fn windows_x64_installer_names(version: &str) -> [String; 1] {
    [format!("slTerminal-v{version}-windows-x64-setup.exe")]
}

/// 资产选择单点(测试与解析共用;生产解析走 [`assets::select`])。
#[allow(dead_code)] // 消费点 = 资产合同单元测试;解析路径经 assets::select。
pub(crate) fn select_windows_x64_installer(
    version: &str,
    release_body: &str,
    assets: &[GitHubReleaseAsset],
) -> Option<UpdateAsset> {
    assets::select(
        version,
        release_body,
        assets,
        &windows_x64_installer_names(version),
    )
}

/// `digest` 字段解析:`sha256:<64 hex>`,大小写/空白容忍。
fn normalize_sha256(value: &str) -> Option<String> {
    let hex = value.trim().strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// release body 内嵌 checksum 回退:同一行必须同时含精确资产名与 64 hex。
fn checksum_from_release_body(body: &str, asset_name: &str) -> Option<String> {
    body.lines().find_map(|line| {
        if !line.contains(&format!("`{asset_name}`")) {
            return None;
        }
        line.split('`').find_map(|part| {
            (part.len() == 64 && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .then(|| part.to_ascii_lowercase())
        })
    })
}

/// 发现与计划安装共用同一版本策略。演练通道允许同版本重装,但永不降级。
pub(crate) fn can_install_version(latest: &str) -> Result<bool, String> {
    #[allow(unused_mut)]
    let mut rehearsal = false;
    #[cfg(any(test, feature = "update-test-source"))]
    {
        rehearsal = rehearsal || test_source::origin()?.is_some();
    }
    Ok(version_is_installable(
        latest,
        env!("CARGO_PKG_VERSION"),
        rehearsal,
    ))
}

fn version_is_installable(latest: &str, current: &str, rehearsal: bool) -> bool {
    is_newer(latest, current) || (rehearsal && latest == current)
}

/// 当前安装早于首个 Inno 安装基(D12-4):Tauri 时代安装基无法被 setup.exe
/// 覆盖,更新链只提示手动重装,自动下载/安装必须保持关闭。
pub(crate) fn requires_manual_reinstall(current: &str) -> bool {
    is_newer(FIRST_INSTALLER_VERSION, current)
}

/// 点分段数值比较("0.7.10" > "0.7.9");段内后缀忽略("1.0.0-rc1" = [1,0,0])。
pub(crate) fn is_newer(latest: &str, current: &str) -> bool {
    fn segments(version: &str) -> Vec<u64> {
        version
            .split('.')
            .map(|segment| {
                let digits: String = segment.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .collect()
    }
    let (latest, current) = (segments(latest), segments(current));
    for index in 0..latest.len().max(current.len()) {
        let new = latest.get(index).copied().unwrap_or(0);
        let old = current.get(index).copied().unwrap_or(0);
        if new != old {
            return new > old;
        }
    }
    false
}

#[cfg(test)]
mod update_check_tests {
    use super::{
        GitHubReleaseAsset, REMIND_LATER_SECS, UpdatePromptState, assets,
        checksum_from_release_body, is_newer, normalize_sha256, parse_latest_release,
        should_prompt_at, update_prompt_state_at, update_state_path_at,
    };

    fn release_asset(name: &str) -> GitHubReleaseAsset {
        GitHubReleaseAsset {
            name: name.to_owned(),
            browser_download_url: format!(
                "https://github.com/LiuShenLan/slTerminal/releases/download/v1.6.0/{name}"
            ),
            size: 42,
            digest: Some(format!("sha256:{}", "a".repeat(64))),
        }
    }

    #[test]
    fn version_comparison_is_numeric_per_segment() {
        assert!(is_newer("0.7.1", "0.7.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.7.0", "0.7.0"));
        assert!(!is_newer("0.6.9", "0.7.0"));
        assert!(!is_newer("0.7.0-rc1", "0.7.0"));
    }

    #[test]
    fn installs_older_than_the_first_inno_base_require_manual_reinstall() {
        for old in ["0.2.0", "0.2.99", "0.1.0"] {
            assert!(super::requires_manual_reinstall(old), "{old}");
        }
        for current in ["0.3.0", "0.3.1", "1.0.0"] {
            assert!(!super::requires_manual_reinstall(current), "{current}");
        }
    }

    #[test]
    fn rehearsal_permits_only_exact_reinstall_or_upgrade() {
        for rehearsal in [false, true] {
            assert!(super::version_is_installable("1.9.0", "1.8.0", rehearsal));
            assert!(!super::version_is_installable("1.7.0", "1.8.0", rehearsal));
            assert!(!super::version_is_installable(
                "1.8.0-rc1",
                "1.8.0",
                rehearsal
            ));
        }
        assert!(!super::version_is_installable("1.8.0", "1.8.0", false));
        assert!(super::version_is_installable("1.8.0", "1.8.0", true));
    }

    #[test]
    fn prompt_state_snoozes_only_the_prompted_version_for_three_days() {
        let mut state = UpdatePromptState::default();
        assert!(state.should_prompt("1.2.0", 1_000));

        state.mark_prompted("1.2.0");
        assert!(!state.should_prompt("1.2.0", 1_000));

        state.remind_later("1.2.0", 1_000);
        assert!(!state.should_prompt("1.2.0", 1_000 + REMIND_LATER_SECS - 1));
        assert!(state.should_prompt("1.2.0", 1_000 + REMIND_LATER_SECS));
        assert!(
            state.should_prompt("1.3.0", 1_001),
            "新版本不继承旧版本延迟"
        );
    }

    #[test]
    fn prompt_state_skips_one_version_without_hiding_the_next() {
        let mut state = UpdatePromptState::default();
        state.skip("1.2.0");

        assert!(!state.should_prompt("1.2.0", 1_000));
        assert!(state.should_prompt("1.3.0", 1_000));

        state.remind_later("1.2.0", 1_000);
        assert!(!state.skipped.contains("1.2.0"), "稍后提醒应重新启用该版本");
    }

    #[test]
    fn prompt_state_persists_and_is_shared_across_reloads() {
        let root = tempfile::tempdir().unwrap();
        let path = update_state_path_at(root.path());
        update_prompt_state_at(&path, |state| state.skip("2.0.0")).unwrap();
        assert!(!should_prompt_at(&path, "2.0.0", 0));
        assert!(should_prompt_at(&path, "2.1.0", 0));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("2.0.0"));
    }

    #[test]
    fn prompt_state_write_is_refused_while_another_process_holds_the_lock() {
        let root = tempfile::tempdir().unwrap();
        let path = update_state_path_at(root.path());
        update_prompt_state_at(&path, |state| state.mark_prompted("1.0.0")).unwrap();
        let before = std::fs::read(&path).unwrap();
        let held = crate::atomic_file::try_lock(&path).unwrap().expect("lock");
        assert!(update_prompt_state_at(&path, |state| state.skip("1.0.0")).is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "被占用时不得改写状态文件"
        );
        drop(held);
        assert!(update_prompt_state_at(&path, |state| state.skip("1.0.0")).is_ok());
    }

    #[test]
    fn release_parser_selects_only_the_exact_windows_installer() {
        let json = br#"{
            "tag_name": "v1.4.0",
            "body": "",
            "assets": [
                {
                    "name": "slTerminal-v1.4.0-windows-x64.zip",
                    "browser_download_url": "https://github.com/LiuShenLan/slTerminal/releases/download/v1.4.0/slTerminal-v1.4.0-windows-x64.zip",
                    "size": 10
                },
                {
                    "name": "slTerminal-v1.4.0-windows-x64-setup.exe",
                    "browser_download_url": "https://github.com/LiuShenLan/slTerminal/releases/download/v1.4.0/slTerminal-v1.4.0-windows-x64-setup.exe",
                    "size": 42,
                    "digest": "sha256:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                }
            ]
        }"#;
        let release = parse_latest_release(json).expect("valid release");
        let asset = release.asset.expect("x64 installer");

        assert_eq!(release.version, "1.4.0");
        assert_eq!(asset.name, "slTerminal-v1.4.0-windows-x64-setup.exe");
        assert_eq!(asset.size, Some(42));
        assert_eq!(asset.digest.as_deref(), Some("b".repeat(64).as_str()));
    }

    #[test]
    fn release_parser_rejects_prerelease_and_draft_channels() {
        for channel in [r#""prerelease": true"#, r#""draft": true"#] {
            let json = format!(r#"{{"tag_name":"v1.4.0",{channel},"assets":[]}}"#);
            assert!(parse_latest_release(json.as_bytes()).is_err());
        }
        let json = br#"{"tag_name":"v1.4.0-rc1","assets":[]}"#;
        let release = parse_latest_release(json).unwrap();
        assert_eq!(release.version, "1.4.0-rc1");
    }

    #[test]
    fn release_parser_accepts_a_null_release_body() {
        let json = br#"{"tag_name":"v1.4.0","body":null,"assets":[]}"#;
        let release = parse_latest_release(json).expect("nullable GitHub release body");

        assert_eq!(release.version, "1.4.0");
        assert!(release.asset.is_none());
    }

    #[test]
    fn digest_normalization_tolerates_case_and_whitespace() {
        let hash = "aBcD".repeat(16);
        assert_eq!(
            normalize_sha256(&format!(" sha256:{hash} \n")).as_deref(),
            Some(hash.to_ascii_lowercase().as_str())
        );
        assert_eq!(normalize_sha256("sha256:not-hex"), None);
        assert_eq!(normalize_sha256(&hash), None, "缺少 sha256: 前缀");
    }

    #[test]
    fn release_body_checksum_matches_the_exact_asset_name() {
        let hash = "1234567890abcdef".repeat(4);
        let portable_hash = "fedcba0987654321".repeat(4);
        let installer = "slTerminal-v1.4.0-windows-x64-setup.exe";
        let body = format!(
            "**SHA256**\n- `slTerminal-v1.4.0-windows-x64.zip`: `{portable_hash}`\n- `{installer}`: `{hash}`"
        );

        assert_eq!(
            checksum_from_release_body(&body, installer).as_deref(),
            Some(hash.as_str())
        );
        assert_eq!(
            checksum_from_release_body(&body, "slTerminal-v1.4.1-windows-x64-setup.exe"),
            None
        );
    }

    #[test]
    fn installer_selection_rejects_other_versions_platforms_and_architectures() {
        let exact = release_asset("slTerminal-v1.6.0-windows-x64-setup.exe");
        assert!(super::select_windows_x64_installer("1.6.0", "", &[exact]).is_some());

        for name in [
            "slTerminal-v1.5.0-windows-x64-setup.exe",
            "slTerminal-v1.6.0-windows-arm64-setup.exe",
            "slTerminal-v1.6.0-windows-x64.zip",
            "slTerminal-1.6.0-windows-x64-setup.exe",
            "prefix-slTerminal-v1.6.0-windows-x64-setup.exe",
            "slTerminal-v1.6.0-windows-x64-setup.exe.bak",
        ] {
            let assets = [release_asset(name)];
            assert!(
                super::select_windows_x64_installer("1.6.0", "", &assets).is_none(),
                "{name}"
            );
        }
    }

    #[test]
    fn installer_selection_uses_body_checksum_when_digest_is_absent() {
        let name = "slTerminal-v1.6.0-windows-x64-setup.exe";
        let mut asset = release_asset(name);
        asset.digest = None;
        let hash = "b".repeat(64);
        let body = format!("- `{name}`: `{hash}`");
        let selected = super::select_windows_x64_installer("1.6.0", &body, &[asset]).unwrap();
        assert_eq!(selected.digest, Some(hash));
    }

    #[test]
    fn checksum_manifest_requires_one_exact_valid_entry() {
        let Some(name) = assets::native_names("1.9.0").into_iter().next() else {
            return;
        };
        let entry = format!("{}  {name}\n", "a".repeat(64));
        assert!(assets::from_checksums("1.9.0", &entry).is_some());
        assert!(assets::from_checksums("1.9.0", &(entry.clone() + &entry)).is_none());
        assert!(assets::from_checksums("1.9.0", &format!("bad  {name}")).is_none());
        assert!(
            assets::from_checksums("1.9.0", &entry.replace(&name, &(name.clone() + ".bak")))
                .is_none()
        );
        assert!(assets::from_checksums("1.9.1", &entry).is_none());
    }
}
