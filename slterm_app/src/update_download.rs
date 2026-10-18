//! 更新安装包的下载、校验与本地状态(09 篇 M10.5 采纳点 28–30)。
//!
//! 检查侧只提供 release 元数据;本模块再次收紧资产合同,并把大文件流式
//! 写入同目录 `.part` 文件。只有长度 / PE 文件头(`MZ`)/ SHA-256 / 512MiB
//! 上限全部通过后才原子替换为可启动的安装包——中断下载或错误响应不能
//! 变成可执行文件。
//!
//! 作废链:每个下载任务持有单调递增的 generation;取消或换版本后旧任务
//! 的进度与完成写入一律失配失效。取消不必知道旧任务句柄。
//!
//! 持久化证据两层:会话内 [`DownloadStatus`] 与磁盘 `updates/download.json`
//! 缓存;启动 `hydrate` 在无网时恢复就绪/失败态。

use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use sha2::{Digest as _, Sha256};

use crate::update_check::{RELEASE_DOWNLOAD_PREFIX, UpdateAsset};

mod cache;
pub(crate) mod handoff;

/// 安装包安全上限(契约值,照抄上游 09 篇):超过即中止。
pub(crate) const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;
/// 下载总时限:大文件经代理/慢速 CDN 的预算。
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static DOWNLOAD_SESSION: Mutex<Option<DownloadSession>> = Mutex::new(None);

/// 下载状态。`Downloading`/`Ready` 的字段是更新弹窗的进度与路径消费面
/// (UI 归更新提示步);链内只按变体分派,故字段豁免 dead_code。
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum DownloadStatus {
    /// 无会话、无缓存(未知资产)。
    Absent,
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Ready {
        path: PathBuf,
        bytes: u64,
    },
    Failed(String),
    /// 下载已就绪但 handoff 安装失败(「未见过」重提示判定用)。
    InstallFailed(String),
}

impl DownloadStatus {
    /// 消费点 = 更新弹窗的「下载完成/失败」判定(提示步接线)。
    #[allow(dead_code)]
    pub(crate) fn is_terminal(&self) -> bool {
        !matches!(self, Self::Downloading { .. })
    }
}

#[derive(Clone, Debug)]
struct DownloadSession {
    generation: u64,
    asset: UpdateAsset,
    status: DownloadStatus,
}

fn session() -> MutexGuard<'static, Option<DownloadSession>> {
    DOWNLOAD_SESSION
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// 测试隔离:L1 共用进程,下载会话是模块级单例;每个用例以空会话起步。
#[cfg(test)]
pub(crate) fn reset_session() {
    *session() = None;
}

pub(crate) fn status(asset: &UpdateAsset) -> DownloadStatus {
    session()
        .as_ref()
        .filter(|current| current.asset == *asset)
        .map(|current| current.status.clone())
        .unwrap_or(DownloadStatus::Absent)
}

/// 一个任务拥有一个 generation;取消或换资产会让旧任务的进度/完成写入
/// 全部失效(即使版本相同)。
#[derive(Clone)]
pub(crate) struct DownloadJob {
    asset: UpdateAsset,
    generation: u64,
}

impl DownloadJob {
    pub(crate) fn is_current(&self) -> bool {
        session().as_ref().is_some_and(|current| {
            current.generation == self.generation && current.asset == self.asset
        })
    }
}

pub(crate) fn begin(asset: &UpdateAsset) -> Result<Option<DownloadJob>, String> {
    crate::platform::distribution::require_direct_update()?;
    validate_asset(asset)?;
    Ok(begin_download_session(asset))
}

/// 会话所有权与平台/资产校验分离:公开入口先过闸门,测试可直接建会话。
fn begin_download_session(asset: &UpdateAsset) -> Option<DownloadJob> {
    let mut current = session();
    if let Some(existing) = current.as_ref().filter(|existing| existing.asset == *asset)
        && matches!(
            existing.status,
            DownloadStatus::Downloading { .. } | DownloadStatus::Ready { .. }
        )
    {
        return None;
    }
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    *current = Some(DownloadSession {
        generation,
        asset: asset.clone(),
        status: DownloadStatus::Downloading {
            downloaded: 0,
            total: asset.size,
        },
    });
    Some(DownloadJob {
        asset: asset.clone(),
        generation,
    })
}

/// 消费点 = 用户取消下载/切换版本(更新弹窗步);L1 作废链测试已覆盖。
#[allow(dead_code)]
pub(crate) fn cancel(asset: &UpdateAsset) {
    let mut current = session();
    if current
        .as_ref()
        .is_some_and(|current| current.asset == *asset)
    {
        *current = None;
    }
}

/// 下载上下文:数据根目录 + 可选演练 origin(`None` = 官方 URL)。
pub(crate) struct DownloadContext {
    pub(crate) root: PathBuf,
    /// 仅测试/演练构建可能为 `Some`;生产构建恒 `None`。
    #[cfg_attr(not(any(test, feature = "update-test-source")), allow(dead_code))]
    pub(crate) origin: Option<String>,
}

impl DownloadContext {
    fn current() -> Result<Self, String> {
        #[cfg(any(test, feature = "update-test-source"))]
        let origin = crate::update_check::test_source::origin()?;
        #[cfg(not(any(test, feature = "update-test-source")))]
        let origin = None;
        Ok(Self {
            root: crate::platform::dirs::data_dir().to_owned(),
            origin,
        })
    }
}

/// 后台线程全程执行;缓存文件在 Ready 前总会重新校验。
pub(crate) fn run(job: DownloadJob) {
    // 已被取消/替换的旧任务不产生任何磁盘与状态副作用。
    if !job.is_current() {
        return;
    }
    let context = DownloadContext::current();
    let outcome = match &context {
        Ok(context) => download_and_verify(&job.asset, context, Some(&job)),
        Err(error) => Err(error.clone()),
    };
    finish_job(
        job,
        context.map_or_else(|_| crate::platform::dirs::data_dir().to_owned(), |c| c.root),
        outcome,
    );
}

/// 测试/演练入口:显式数据根与 origin,不读环境、不碰真实目录。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn run_in(job: DownloadJob, context: &DownloadContext) {
    if !job.is_current() {
        return;
    }
    let outcome = download_and_verify(&job.asset, context, Some(&job));
    finish_job(job, context.root.clone(), outcome);
}

fn finish_job(job: DownloadJob, root: PathBuf, outcome: Result<(PathBuf, u64), String>) {
    let status = match outcome {
        Ok((path, bytes)) => DownloadStatus::Ready { path, bytes },
        Err(error) => DownloadStatus::Failed(error),
    };
    {
        let mut current = session();
        let Some(current) = current
            .as_mut()
            .filter(|current| current.generation == job.generation)
        else {
            return;
        };
        current.status = status.clone();
    }
    // 文件 sync 在繁忙磁盘上可能耗时数秒;UI 状态轮询从不等待它。
    if let Err(error) = cache::save_job(&root, &job, &status) {
        log::warn!("Could not persist update download state: {error}");
    }
}

/// 启动恢复:handoff 失败证据优先,其次下载缓存;无网也能恢复终态。
/// 用户已启动的新任务优先(会话非空时不覆盖)。
pub(crate) fn hydrate() {
    let executable = std::env::current_exe().ok();
    hydrate_at(crate::platform::dirs::data_dir(), executable.as_deref());
}

pub(crate) fn hydrate_at(root: &Path, executable: Option<&Path>) {
    if crate::platform::distribution::current().externally_managed() {
        return;
    }
    // 崩溃遗留的未 commit 事务按 cancel 安全默认收尾(文件是持久证据)。
    handoff::recover_stale_handoffs(root);
    let cached = executable
        .and_then(|executable| handoff::failed_update_at(root, executable))
        .map(|(asset, error)| (asset, DownloadStatus::InstallFailed(error)))
        .or_else(|| cache::load_at(root));
    let Some((asset, status)) = cached else {
        return;
    };
    let mut current = session();
    if current.is_none() {
        *current = Some(DownloadSession {
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            asset,
            status,
        });
    }
}

pub(crate) fn cached_asset() -> Option<UpdateAsset> {
    session().as_ref().map(|current| current.asset.clone())
}

/// 失败发生在用户选「稍后」之后 = 新信息;查看/忽略细节后恢复常规提醒抑制。
pub(crate) fn installation_failure_unseen(prompt_state: &Path) -> bool {
    let root = crate::platform::dirs::data_dir();
    let executable = std::env::current_exe().ok();
    handoff::failure_unseen(root, executable.as_deref(), prompt_state)
        || cache::failure_unseen_at(root, prompt_state)
}

/// 消费点 = handoff prepare(安装授权步)与更新弹窗的「立即安装」按钮。
#[allow(dead_code)]
pub(crate) fn ready_path(asset: &UpdateAsset) -> Result<PathBuf, String> {
    ready_path_at(crate::platform::dirs::data_dir(), asset)
}

/// `_at` 形态:handoff 事务与测试使用显式数据根。
#[allow(dead_code)]
pub(crate) fn ready_path_at(root: &Path, asset: &UpdateAsset) -> Result<PathBuf, String> {
    let path = match status(asset) {
        DownloadStatus::Ready { path, .. } => path,
        _ => return Err("安装包尚未下载并通过校验".to_owned()),
    };
    let (_, expected_path) = download_paths_at(root, asset)?;
    if path != expected_path || !path.is_file() {
        return Err("已校验的安装包不存在或路径已改变".to_owned());
    }
    // Ready 只表示下载完成时通过过校验;安装前再读一遍,避免缓存文件在
    // 等待用户确认期间被替换后仍直接执行。
    verify_file(&path, asset).map_err(|error| format!("安装前重新校验失败:{error}"))?;
    Ok(path)
}

fn download_and_verify(
    asset: &UpdateAsset,
    context: &DownloadContext,
    job: Option<&DownloadJob>,
) -> Result<(PathBuf, u64), String> {
    crate::platform::distribution::require_direct_update()?;
    validate_asset(asset)?;
    let (partial_path, final_path) = download_paths_at(&context.root, asset)?;
    let _download_lock = crate::atomic_file::try_lifetime_lock(&final_path)
        .map_err(|error| format!("无法锁定更新下载目录:{error}"))?
        .ok_or_else(|| "另一个 slTerminal 进程正在下载这项更新".to_owned())?;

    if final_path.is_file()
        && let Ok(bytes) = verify_file(&final_path, asset)
    {
        return Ok((final_path, bytes));
    }

    let result = download_to_partial(asset, &partial_path, context, job).and_then(|bytes| {
        if job.is_some_and(|job| !job.is_current()) {
            return Err("Download cancelled".into());
        }
        crate::atomic_file::replace(&partial_path, &final_path)
            .map_err(|error| format!("无法保存已校验的更新安装包:{error}"))?;
        Ok((final_path.clone(), bytes))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&partial_path);
    }
    result
}

fn download_to_partial(
    asset: &UpdateAsset,
    partial_path: &Path,
    context: &DownloadContext,
    job: Option<&DownloadJob>,
) -> Result<u64, String> {
    // 生产构建 origin 恒 None,分支整体不编译;显式消费避免 unused 告警。
    #[cfg(not(any(test, feature = "update-test-source")))]
    let _ = context;
    #[cfg(any(test, feature = "update-test-source"))]
    if let Some(origin) = context.origin.as_deref() {
        let agent = crate::update_check::test_source::agent(DOWNLOAD_TIMEOUT);
        return download_with_job(asset, partial_path, &agent, job, Some(origin));
    }
    let agent = crate::update_proxy::agent(&asset.url, DOWNLOAD_TIMEOUT);
    download_with_job(asset, partial_path, &agent, job, None)
}

fn download_with_job(
    asset: &UpdateAsset,
    partial_path: &Path,
    agent: &ureq::Agent,
    job: Option<&DownloadJob>,
    origin: Option<&str>,
) -> Result<u64, String> {
    if job.is_some_and(|job| !job.is_current()) {
        return Err("Download cancelled".into());
    }
    let download_url = origin.map_or_else(
        || asset.url.clone(),
        |origin| format!("{origin}/{}", asset.name),
    );
    let mut response = agent
        .get(&download_url)
        .header("User-Agent", "slterm-updater")
        .header("Accept", "application/octet-stream")
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(network_error_text)?;

    let response_size = response.body().content_length();
    if let (Some(expected), Some(actual)) = (asset.size, response_size)
        && expected != actual
    {
        return Err(format!(
            "安装包长度与 release 元数据不一致({actual} / {expected} 字节)"
        ));
    }
    let total = asset.size.or(response_size);
    if total.is_some_and(|bytes| bytes > MAX_INSTALLER_BYTES) {
        return Err("安装包超过 512 MiB 安全上限".to_owned());
    }

    let mut output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(partial_path)
        .map_err(|error| format!("无法创建更新临时文件:{error}"))?;
    let mut reader = response.body_mut().as_reader();
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut pe_header = Vec::with_capacity(2);
    let mut buffer = vec![0_u8; DOWNLOAD_CHUNK_BYTES];
    loop {
        if job.is_some_and(|job| !job.is_current()) {
            return Err("Download cancelled".into());
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| network_error_text(ureq::Error::Io(error)))?;
        if read == 0 {
            break;
        }
        downloaded = downloaded.saturating_add(read as u64);
        if downloaded > MAX_INSTALLER_BYTES {
            return Err("安装包超过 512 MiB 安全上限".to_owned());
        }
        if pe_header.len() < 2 {
            let take = (2 - pe_header.len()).min(read);
            pe_header.extend_from_slice(&buffer[..take]);
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|error| format!("写入更新临时文件失败:{error}"))?;
        set_progress(job, downloaded, total);
    }
    output
        .sync_all()
        .map_err(|error| format!("同步更新临时文件失败:{error}"))?;

    verify_download(downloaded, &pe_header, hasher.finalize(), asset)?;
    Ok(downloaded)
}

/// 网络错误用稳定类别解释;不把代理 URL、认证信息或 CDN 查询串拼进状态。
fn network_error_text(error: ureq::Error) -> String {
    use std::io::ErrorKind;

    use ureq::Error;

    match error {
        Error::HostNotFound => "更新服务器域名解析失败".to_owned(),
        Error::Tls(_) | Error::Rustls(_) | Error::TlsRequired => {
            "更新服务器 TLS 校验失败".to_owned()
        }
        Error::Timeout(_) => "更新请求超时".to_owned(),
        Error::Io(ref io) if io.kind() == ErrorKind::ConnectionRefused => {
            "更新服务器拒绝连接".to_owned()
        }
        Error::Io(ref io) if io.kind() == ErrorKind::TimedOut => "更新请求超时".to_owned(),
        Error::Io(ref io)
            if matches!(
                io.kind(),
                ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe
            ) =>
        {
            "更新下载中断".to_owned()
        }
        Error::ConnectProxyFailed(_) | Error::InvalidProxyUrl => "更新代理不可用".to_owned(),
        Error::StatusCode(status) => format!("更新服务器返回 HTTP {status}"),
        _ => "更新网络请求失败".to_owned(),
    }
}

fn verify_file(path: &Path, asset: &UpdateAsset) -> Result<u64, String> {
    let mut file = File::open(path).map_err(|error| format!("无法读取更新缓存:{error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("无法读取更新缓存大小:{error}"))?;
    let bytes = metadata.len();
    if bytes > MAX_INSTALLER_BYTES {
        return Err("更新缓存超过 512 MiB 安全上限".to_owned());
    }
    let mut hasher = Sha256::new();
    let mut pe_header = Vec::with_capacity(2);
    let mut buffer = vec![0_u8; DOWNLOAD_CHUNK_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("读取更新缓存失败:{error}"))?;
        if read == 0 {
            break;
        }
        if pe_header.len() < 2 {
            let take = (2 - pe_header.len()).min(read);
            pe_header.extend_from_slice(&buffer[..take]);
        }
        hasher.update(&buffer[..read]);
    }
    verify_download(bytes, &pe_header, hasher.finalize(), asset)?;
    Ok(bytes)
}

/// 四重校验的终局:长度、MZ 头、SHA-256;512MiB 上限在流式阶段已拦。
fn verify_download(
    bytes: u64,
    pe_header: &[u8],
    digest: impl AsRef<[u8]>,
    asset: &UpdateAsset,
) -> Result<(), String> {
    if bytes == 0 || asset.size.is_some_and(|expected| expected != bytes) {
        return Err(format!("安装包长度校验失败(实际 {bytes} 字节)"));
    }
    if pe_header != b"MZ" {
        return Err("下载内容不是 Windows PE 安装包".to_owned());
    }
    let expected = asset
        .digest
        .as_deref()
        .ok_or_else(|| "release 未提供 SHA-256".to_owned())?;
    let mut actual = String::with_capacity(64);
    for byte in digest.as_ref() {
        let _ = write!(&mut actual, "{byte:02x}");
    }
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!("安装包 SHA-256 校验失败(实际 {actual})"));
    }
    Ok(())
}

fn set_progress(job: Option<&DownloadJob>, downloaded: u64, total: Option<u64>) {
    let Some(job) = job else {
        return;
    };
    let mut current = session();
    if let Some(current) = current
        .as_mut()
        .filter(|current| current.generation == job.generation)
    {
        current.status = DownloadStatus::Downloading { downloaded, total };
    }
}

fn download_paths_at(root: &Path, asset: &UpdateAsset) -> Result<(PathBuf, PathBuf), String> {
    let directory = root.join("updates");
    std::fs::create_dir_all(&directory).map_err(|error| format!("无法创建更新下载目录:{error}"))?;
    let final_path = directory.join(&asset.name);
    let partial_path = directory.join(format!("{}.part", asset.name));
    Ok((partial_path, final_path))
}

fn validate_asset(asset: &UpdateAsset) -> Result<(), String> {
    validate_asset_contract(
        asset,
        &crate::update_check::assets::native_names(&asset.version),
    )
}

/// 下载器侧的二次收紧:名称 / 版本 / 官方 URL 前缀 / 大小 / 摘要格式。
fn validate_asset_contract(asset: &UpdateAsset, names: &[String]) -> Result<(), String> {
    if asset.version.is_empty()
        || !asset
            .version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err("release 版本号不符合安装包命名规则".to_owned());
    }
    if !names.contains(&asset.name) {
        return Err("release 资产不是当前平台的精确安装包".to_owned());
    }
    if asset.url != format!("{RELEASE_DOWNLOAD_PREFIX}v{}/{}", asset.version, asset.name) {
        return Err("release 安装包 URL 不属于 slTerminal 官方仓库".to_owned());
    }
    if asset
        .size
        .is_some_and(|bytes| bytes == 0 || bytes > MAX_INSTALLER_BYTES)
    {
        return Err("release 安装包大小无效".to_owned());
    }
    let digest = asset.digest.as_deref().ok_or_else(|| {
        "release 未提供可验证的 SHA-256;为避免执行未知安装包,已停止自动下载".to_owned()
    })?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("release 提供的 SHA-256 格式无效".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod update_download_tests {
    use sha2::{Digest as _, Sha256};

    use super::{
        DownloadContext, MAX_INSTALLER_BYTES, UpdateAsset, validate_asset_contract, verify_download,
    };
    use crate::update_proxy::test_support::{Server, response};

    fn asset_with(url: &str, digest: Option<&str>) -> UpdateAsset {
        UpdateAsset {
            version: "1.4.0".to_owned(),
            name: "slTerminal-v1.4.0-windows-x64-setup.exe".to_owned(),
            url: url.to_owned(),
            size: Some(42),
            digest: digest.map(str::to_owned),
        }
    }

    fn branded_asset() -> UpdateAsset {
        let name = "slTerminal-v1.6.0-windows-x64-setup.exe";
        UpdateAsset {
            version: "1.6.0".to_owned(),
            url: format!(
                "https://github.com/LiuShenLan/slTerminal/releases/download/v1.6.0/{name}"
            ),
            name: name.to_owned(),
            size: Some(42),
            digest: Some("b".repeat(64)),
        }
    }

    fn sha256(body: &[u8]) -> String {
        Sha256::digest(body)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn cancel_then_retry_rejects_old_progress_and_old_completion() {
        super::reset_session();
        let asset = branded_asset();
        super::validate_asset(&asset).unwrap();
        super::cancel(&asset);
        let old = super::begin_download_session(&asset).unwrap();
        assert!(
            super::begin_download_session(&asset).is_none(),
            "duplicate click owns no second task"
        );
        super::cancel(&asset);
        let current = super::begin_download_session(&asset).unwrap();
        super::set_progress(Some(&old), 100, Some(200));
        super::run(old.clone());
        assert!(!old.is_current());
        assert!(current.is_current());
        assert!(matches!(
            super::status(&asset),
            super::DownloadStatus::Downloading { downloaded: 0, .. }
        ));
        super::set_progress(Some(&current), 25, Some(200));
        assert!(matches!(
            super::status(&asset),
            super::DownloadStatus::Downloading { downloaded: 25, .. }
        ));

        // 换版本同样经代际失配作废旧任务。
        let mut newer = asset.clone();
        newer.version = "1.7.0".to_owned();
        newer.name = "slTerminal-v1.7.0-windows-x64-setup.exe".to_owned();
        newer.url = format!(
            "https://github.com/LiuShenLan/slTerminal/releases/download/v1.7.0/{}",
            newer.name
        );
        let switched = super::begin_download_session(&newer).unwrap();
        assert!(!current.is_current(), "换版本后旧任务失效");
        assert!(switched.is_current());
        super::cancel(&newer);
        super::cancel(&asset);
    }

    #[test]
    fn begin_preserves_platform_and_asset_validation() {
        super::reset_session();
        let asset = branded_asset();
        assert!(super::validate_asset(&asset).is_ok());
        assert!(super::begin(&asset).is_ok());
        super::cancel(&asset);
        let mut asset = branded_asset();
        asset.url = "https://example.invalid/untrusted.exe".into();
        assert!(
            super::begin(&asset).is_err(),
            "the session must not bypass asset validation"
        );
    }

    #[test]
    fn proxy_download_follows_redirect_and_verifies_the_streamed_installer() {
        let body = "MZinstaller over a proxy";
        let server = Server::start(vec![
            response(
                "302 Found",
                "Location: http://cdn.update.invalid/installer\r\n",
                "",
            ),
            response("200 OK", "", body),
        ]);
        let mut asset = branded_asset();
        asset.url = "http://release.update.invalid/asset".into();
        asset.size = Some(body.len() as u64);
        asset.digest = Some(sha256(body.as_bytes()));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("installer.part");
        let bytes =
            super::download_with_job(&asset, &path, &server.agent(&[]), None, None).unwrap();
        assert_eq!(bytes, body.len() as u64);
        assert_eq!(std::fs::read(&path).unwrap(), body.as_bytes());
        assert!(super::verify_file(&path, &asset).is_ok());
        let requests = server.finish();
        assert!(requests.len() >= 2);
        // 夹具记录 CONNECT 之后的内层请求行(源形式路径,不含主机名)。
        assert!(requests[0].contains("/asset"), "{requests:?}");
        assert!(requests[1].contains("/installer"), "{requests:?}");
    }

    #[test]
    fn proxy_download_rejects_http_errors_truncated_bodies_and_bad_digests() {
        for reply in [
            response("503 Service Unavailable", "", "unavailable"),
            "HTTP/1.1 200 OK\r\nContent-Length: 42\r\nConnection: close\r\n\r\nMZshort".into(),
            response("200 OK", "", &format!("MZ{}", "x".repeat(40))),
        ] {
            let server = Server::start(vec![reply]);
            let mut asset = branded_asset();
            asset.url = "http://release.update.invalid/asset".into();
            let directory = tempfile::tempdir().unwrap();
            let result = super::download_with_job(
                &asset,
                &directory.path().join("installer.part"),
                &server.agent(&[]),
                None,
                None,
            );
            assert!(result.is_err());
            server.finish();
        }
    }

    #[test]
    fn rehearsal_origin_rewrites_transport_but_keeps_the_official_contract() {
        super::reset_session();
        let body = "MZlocal rehearsal installer";
        let server = Server::start(vec![response("200 OK", "", body)]);
        let mut asset = branded_asset();
        asset.size = Some(body.len() as u64);
        asset.digest = Some(sha256(body.as_bytes()));
        let directory = tempfile::tempdir().unwrap();
        let context = DownloadContext {
            root: directory.path().to_owned(),
            origin: Some(format!("http://{}", server.address)),
        };
        let job = super::begin_download_session(&asset).unwrap();
        super::run_in(job, &context);
        let request = server.finish();
        assert!(request[0].contains(&asset.name), "{request:?}");
        assert!(matches!(
            super::status(&asset),
            super::DownloadStatus::Ready { bytes, .. } if bytes == body.len() as u64
        ));
        super::cancel(&asset);
    }

    #[test]
    fn network_failures_have_actionable_messages_without_credentials() {
        use std::io::{Error as IoError, ErrorKind};
        use ureq::Error;

        for (error, expected) in [
            (Error::HostNotFound, "更新服务器域名解析失败"),
            (Error::Tls("invalid certificate"), "更新服务器 TLS 校验失败"),
            (
                Error::Io(IoError::from(ErrorKind::ConnectionRefused)),
                "更新服务器拒绝连接",
            ),
            (
                Error::Io(IoError::from(ErrorKind::TimedOut)),
                "更新请求超时",
            ),
            (
                Error::Io(IoError::from(ErrorKind::UnexpectedEof)),
                "更新下载中断",
            ),
            (
                Error::ConnectProxyFailed("http://user:secret@proxy.local".into()),
                "更新代理不可用",
            ),
            (Error::ConnectionFailed, "更新网络请求失败"),
        ] {
            let text = super::network_error_text(error);
            assert_eq!(text, expected);
            assert!(!text.contains("secret"));
        }
        assert!(super::network_error_text(Error::StatusCode(503)).contains("HTTP 503"));
    }

    #[test]
    fn accepts_exact_official_asset_contract() {
        let url = "https://github.com/LiuShenLan/slTerminal/releases/download/v1.4.0/slTerminal-v1.4.0-windows-x64-setup.exe";
        let hash = "a".repeat(64);
        assert!(
            validate_asset_contract(
                &asset_with(url, Some(&hash)),
                &["slTerminal-v1.4.0-windows-x64-setup.exe".to_owned()]
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_untrusted_url_or_missing_digest() {
        let official = "https://github.com/LiuShenLan/slTerminal/releases/download/v1.4.0/slTerminal-v1.4.0-windows-x64-setup.exe";
        let untrusted = "https://example.invalid/slTerminal-v1.4.0-windows-x64-setup.exe";
        let hash = "a".repeat(64);
        let names = ["slTerminal-v1.4.0-windows-x64-setup.exe".to_owned()];
        assert!(validate_asset_contract(&asset_with(untrusted, Some(&hash)), &names).is_err());
        assert!(validate_asset_contract(&asset_with(official, None), &names).is_err());
    }

    #[test]
    fn rejects_non_x64_names_invalid_versions_sizes_and_hashes() {
        let names = ["slTerminal-v1.6.0-windows-x64-setup.exe".to_owned()];
        let base = branded_asset();
        for name in [
            "slTerminal-v1.6.0-windows-arm64-setup.exe",
            "slTerminal-v1.6.0-windows-x64.zip",
            "slTerminal-v1.6.0-linux-x86_64.AppImage",
            "../slTerminal-v1.6.0-windows-x64-setup.exe",
        ] {
            let mut candidate = base.clone();
            candidate.name = name.to_owned();
            assert!(validate_asset_contract(&candidate, &names).is_err());
        }
        for version in ["", "../1.6.0", "1.6.0?download=1", "1.6.0\n"] {
            let mut candidate = base.clone();
            candidate.version = version.to_owned();
            assert!(validate_asset_contract(&candidate, &names).is_err());
        }
        for size in [0, MAX_INSTALLER_BYTES + 1] {
            let mut candidate = base.clone();
            candidate.size = Some(size);
            assert!(validate_asset_contract(&candidate, &names).is_err());
        }
        for digest in [None, Some("a".repeat(63)), Some("g".repeat(64))] {
            let mut candidate = base.clone();
            candidate.digest = digest;
            assert!(validate_asset_contract(&candidate, &names).is_err());
        }
    }

    #[test]
    fn rejects_corrupt_or_non_executable_downloads() {
        let bytes = b"MZinstaller fixture";
        let digest = Sha256::digest(bytes);
        let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut candidate = branded_asset();
        candidate.size = Some(bytes.len() as u64);
        candidate.digest = Some(hash);
        assert!(verify_download(bytes.len() as u64, b"MZ", digest, &candidate).is_ok());
        assert!(verify_download(0, b"MZ", digest, &candidate).is_err());
        assert!(verify_download(bytes.len() as u64 - 1, b"MZ", digest, &candidate).is_err());
        assert!(verify_download(bytes.len() as u64, b"<!", digest, &candidate).is_err());
        assert!(
            verify_download(
                bytes.len() as u64,
                b"MZ",
                Sha256::digest(b"changed"),
                &candidate
            )
            .is_err()
        );
    }
}
