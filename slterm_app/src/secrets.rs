//! 凭据域门面(10 篇 M10.9)。
//!
//! 三层:
//! - [`SecretBytes`]:出域句柄,`Zeroizing` 即用即焚,`Debug` 只报长度;
//! - 三原语门面 [`store_generic_secret`]/[`load_generic_secret`]/
//!   [`delete_generic_secret`]:任何域消费凭据管理器只经此入口,禁止旁路
//!   直调 Win32 或 `platform::credentials`;
//! - [`WindowsCredentialStore`]:[`CredentialStore`] 消费 trait 的真实后端,
//!   由生产调用点直传(`ai_fix` 后台请求)。
//!
//! 纪律:
//! - 凭据值出域即包 [`SecretBytes`],即用即焚;不提供 `String`/`Vec<u8>`
//!   裸载体。
//! - 明文永不进入任何元数据文件或日志;`Debug` 只报长度。
//! - target 命名空间归调用方(`ai_providers::credential_target` 产出
//!   `Slterm/AI/<id>`),本层不解释 id 语义。

use std::fmt;
use std::io;

use zeroize::Zeroizing;

use crate::platform::credentials;

/// 出域密文句柄:Drop 时擦除字节。
///
/// 只借出 `&[u8]`;转字符串时先校验 UTF-8 再复制进新的 `Zeroizing<String>`,
/// 原句柄仍按 Drop 语义擦除,拒绝路径不产生新的明文载体。
pub(crate) struct SecretBytes(Zeroizing<Vec<u8>>);

#[allow(dead_code)] // 消费点 = M9.5 请求头组装 / M10.9 凭据后端;本步由用例锁定语义。
impl SecretBytes {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// UTF-8 校验并入出域句柄;非法字节返回 `None`,不构造字符串。
    pub(crate) fn into_string(self) -> Option<Zeroizing<String>> {
        std::str::from_utf8(self.as_bytes()).ok()?;
        // 已通过校验,`from_utf8` 不会失败;字符串本体由 Zeroizing 持有。
        String::from_utf8(self.0.to_vec()).ok().map(Zeroizing::new)
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 只报长度:调试输出同样不得泄露凭据。
        formatter
            .debug_struct("SecretBytes")
            .field("len", &self.0.len())
            .finish()
    }
}

/// 凭据域唯一入口:非 SSH 功能消费凭据管理器只经此门面,禁止旁路直调
/// Win32 或 `platform::credentials` 之外的第二封装。
///
/// Keeping the target namespace at the caller makes the secret store reusable
/// without duplicating Windows API code or ever exposing the value to UI state.
pub(crate) fn store_generic_secret(target: &str, secret: &[u8]) -> io::Result<()> {
    credentials::store(target, secret)
}

/// 读取凭据并立即包成出域句柄;不存在返回 `Ok(None)`。
pub(crate) fn load_generic_secret(target: &str) -> io::Result<Option<SecretBytes>> {
    credentials::load(target).map(|secret| secret.map(SecretBytes::new))
}

/// 删除凭据;不存在视为成功(幂等)。
pub(crate) fn delete_generic_secret(target: &str) -> io::Result<()> {
    credentials::delete(target)
}

/// 凭据后端消费接口(10 篇三原语的应用层投影)。
///
/// 生产实现 = [`WindowsCredentialStore`];测试经注入 mock 断言「先写凭据
/// 后回写引用」「先删凭据再删元数据」等调用契约。`exists` 默认经 `load`
/// 推导,真实后端可覆写为不取回明文的探测。
pub(crate) trait CredentialStore {
    fn load(&self, target: &str) -> io::Result<Option<SecretBytes>>;
    fn store(&self, target: &str, secret: &[u8]) -> io::Result<()>;
    fn delete(&self, target: &str) -> io::Result<()>;

    #[allow(dead_code)] // 消费点 = 设置页凭据状态展示(M10.9 后);本步由 mock 用例锁定语义。
    fn exists(&self, target: &str) -> io::Result<bool> {
        Ok(self.load(target)?.is_some())
    }
}

/// [`CredentialStore`] 的真实后端:Windows 凭据管理器。
///
/// 所有原语经 [`store_generic_secret`]/[`load_generic_secret`]/
/// [`delete_generic_secret`] 门面落到 `platform::credentials`,本类型只做
/// 句柄化适配。
#[derive(Debug, Default)]
pub(crate) struct WindowsCredentialStore;

impl CredentialStore for WindowsCredentialStore {
    fn load(&self, target: &str) -> io::Result<Option<SecretBytes>> {
        load_generic_secret(target)
    }

    fn store(&self, target: &str, secret: &[u8]) -> io::Result<()> {
        store_generic_secret(target, secret)
    }

    fn delete(&self, target: &str) -> io::Result<()> {
        delete_generic_secret(target)
    }
}

#[cfg(test)]
mod secrets_tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, Once};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn secret_bytes_borrow_string_and_debug_do_not_leak() {
        let secret = SecretBytes::new(b"sk-test-value".to_vec());
        assert_eq!(secret.as_bytes(), b"sk-test-value");
        assert!(!secret.is_empty());
        let debug = format!("{secret:?}");
        assert!(debug.contains("len"));
        assert!(!debug.contains("sk-test-value"));
        let text = secret.into_string().expect("utf-8 secret");
        assert_eq!(text.as_str(), "sk-test-value");
    }

    #[test]
    fn invalid_utf8_secret_returns_none() {
        assert!(SecretBytes::new(vec![0xff, 0xfe]).into_string().is_none());
    }

    // ---- 三件套往返:真实 Windows 凭据管理器(隔离 target + Drop 清理) ----

    /// 测试 target 专用前缀:每次运行唯一,绝不触碰用户真实凭据。
    fn unique_test_target(label: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时钟早于 UNIX_EPOCH")
            .as_nanos();
        format!("Slterm/Test/{label}-{}-{nanos}", std::process::id())
    }

    /// 用例结束(含 panic 展开)清理测试 target,不污染用户凭据库。
    struct CredentialCleanup(String);

    impl Drop for CredentialCleanup {
        fn drop(&mut self) {
            let _ = delete_generic_secret(&self.0);
        }
    }

    #[test]
    fn credential_three_operations_round_trip_through_windows_manager() {
        let target = unique_test_target("three-operations");
        let _cleanup = CredentialCleanup(target.clone());
        let backend = WindowsCredentialStore;

        // load 不存在 → NotFound 归一为 None。
        assert!(
            backend.load(&target).expect("读取空 target").is_none(),
            "干净 target 必须是 None 而不是后端错误"
        );

        // store → load 往返。
        store_generic_secret(&target, b"sk-test-roundtrip-first").expect("门面写入");
        assert_eq!(
            backend
                .load(&target)
                .expect("后端读取")
                .expect("已写入凭据")
                .as_bytes(),
            b"sk-test-roundtrip-first"
        );

        // 覆盖写 → load 读到新值。
        backend
            .store(&target, b"sk-test-roundtrip-second")
            .expect("后端覆盖写入");
        assert_eq!(
            load_generic_secret(&target)
                .expect("门面读取")
                .expect("已写入凭据")
                .as_bytes(),
            b"sk-test-roundtrip-second"
        );

        // delete → load None;重复 delete 幂等。
        backend.delete(&target).expect("后端删除");
        assert!(load_generic_secret(&target).expect("读取已删除").is_none());
        delete_generic_secret(&target).expect("重复删除必须幂等");
    }

    // ---- 内存后端:trait 注入契约(不依赖操作系统状态) ----

    #[derive(Default)]
    struct MemoryCredentialStore {
        entries: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load(&self, target: &str) -> io::Result<Option<SecretBytes>> {
            Ok(self
                .entries
                .lock()
                .expect("内存凭据锁")
                .get(target)
                .cloned()
                .map(SecretBytes::new))
        }

        fn store(&self, target: &str, secret: &[u8]) -> io::Result<()> {
            self.entries
                .lock()
                .expect("内存凭据锁")
                .insert(target.to_owned(), secret.to_vec());
            Ok(())
        }

        fn delete(&self, target: &str) -> io::Result<()> {
            self.entries.lock().expect("内存凭据锁").remove(target);
            Ok(())
        }
    }

    #[test]
    fn in_memory_store_round_trip_matches_the_trait_contract() {
        let store = MemoryCredentialStore::default();
        assert!(store.load("Slterm/Test/memory").unwrap().is_none());
        store
            .store("Slterm/Test/memory", b"sk-test-memory")
            .unwrap();
        assert_eq!(
            store
                .load("Slterm/Test/memory")
                .unwrap()
                .unwrap()
                .as_bytes(),
            b"sk-test-memory"
        );
        assert!(store.exists("Slterm/Test/memory").unwrap());
        store.delete("Slterm/Test/memory").unwrap();
        assert!(!store.exists("Slterm/Test/memory").unwrap());
        store.delete("Slterm/Test/memory").unwrap();
    }

    // ---- 凭据域边界(类型层):凭据域外不得暴露 token 裸载体 ----

    /// 凭据语义的读取函数名;这些函数在凭据域外只允许返回
    /// `SecretBytes` 句柄(或包着它的 `Option`/`io::Result`)。
    const CREDENTIAL_LOADERS: [&str; 5] = [
        "load_api_key",
        "load_generic_secret",
        "load_secret",
        "read_secret",
        "credential_load",
    ];

    /// OS 原始字节层:设计上允许 `Vec<u8>` 出域,由 `secrets` 门面立即
    /// 句柄化;业务模块不在白名单内。
    const OS_BYTES_LAYER: [&str; 2] =
        ["platform/credentials.rs", "platform/credentials/windows.rs"];

    fn collect_rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(root).expect("读取源码目录") {
            let path = entry.expect("源码目录项").path();
            if path.is_dir() {
                collect_rust_sources(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    fn signature_text(source: &str, start: usize) -> &str {
        let rest = &source[start..];
        let end = rest.find(['{', ';']).unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn credential_domain_boundary_rejects_bare_token_carriers() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust_sources(&root, &mut files);
        let mut inspected_loaders = 0usize;
        for path in files {
            let relative = path
                .strip_prefix(&root)
                .expect("源码路径在 src 下")
                .to_string_lossy()
                .replace('\\', "/");
            let source = std::fs::read_to_string(&path).expect("读取源码");
            for loader in CREDENTIAL_LOADERS {
                let needle = format!("fn {loader}");
                for (start, _) in source.match_indices(&needle) {
                    let signature = signature_text(&source, start);
                    let Some((_, returns)) = signature.split_once("->") else {
                        continue;
                    };
                    let bare = returns.contains("Vec<u8>")
                        || returns.contains("String")
                        || returns.contains("&str");
                    if OS_BYTES_LAYER.contains(&relative.as_str()) {
                        continue;
                    }
                    inspected_loaders += 1;
                    assert!(
                        !bare,
                        "{relative}: 凭据读取函数 {loader} 不得返回裸载体: {signature}"
                    );
                    assert!(
                        returns.contains("SecretBytes"),
                        "{relative}: 凭据读取函数 {loader} 必须返回 SecretBytes 句柄: {signature}"
                    );
                }
            }
        }
        assert!(
            inspected_loaders >= 1,
            "类型层扫描未命中任何凭据域外读取函数(扫描面失效)"
        );

        // 句柄自身不得新增裸载体出口;只扫 `impl SecretBytes` 块,避免误伤
        // 测试辅助函数的普通 `-> String`。`concat!` 拼出禁词,源码不自我命中。
        let handle_source = std::fs::read_to_string(root.join("secrets.rs")).expect("读取门面源码");
        let impl_start = handle_source
            .find("impl SecretBytes {")
            .expect("SecretBytes 实现块存在");
        let impl_end = handle_source[impl_start..]
            .find("\n}")
            .map(|offset| impl_start + offset)
            .expect("SecretBytes 实现块闭合");
        let handle_impl = &handle_source[impl_start..impl_end];
        for forbidden in [concat!("-> ", "Vec<u8>"), concat!("-> ", "String")] {
            assert!(
                !handle_impl.contains(forbidden),
                "SecretBytes 不得返回裸载体: {forbidden}"
            );
        }
    }

    // ---- 凭据域边界(日志层):任何日志不得插值 token ----

    static CAPTURED_LOGS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    static CAPTURE_LOGGER: CaptureLogger = CaptureLogger;

    struct CaptureLogger;

    impl log::Log for CaptureLogger {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            CAPTURED_LOGS
                .lock()
                .expect("日志捕获锁")
                .push((record.target().to_owned(), record.args().to_string()));
        }

        fn flush(&self) {}
    }

    struct FailingCredentialStore;

    impl CredentialStore for FailingCredentialStore {
        fn load(&self, _target: &str) -> io::Result<Option<SecretBytes>> {
            Err(io::Error::other("credential backend denied"))
        }

        fn store(&self, _target: &str, _secret: &[u8]) -> io::Result<()> {
            Err(io::Error::other("credential backend denied"))
        }

        fn delete(&self, _target: &str) -> io::Result<()> {
            Err(io::Error::other("credential backend denied"))
        }
    }

    #[test]
    fn logs_never_contain_credential_plaintext() {
        static INSTALL: Once = Once::new();
        INSTALL.call_once(|| {
            log::set_logger(&CAPTURE_LOGGER).expect("安装测试日志捕获层");
            log::set_max_level(log::LevelFilter::Trace);
        });

        const TOKEN: &str = "sk-test-log-boundary-17f3";
        let store = MemoryCredentialStore::default();

        // 全生命周期:写 → 读 → 句柄 Debug/字符串化。
        let hint =
            crate::ai_providers::save_api_key(&store, "preset-openai", TOKEN).expect("写入凭据");
        assert_eq!(hint, "••••17f3");
        let secret = crate::ai_providers::load_api_key(&store, "preset-openai")
            .expect("读取凭据")
            .expect("已写入凭据");
        assert_eq!(secret.as_bytes(), TOKEN.as_bytes());
        let debug = format!("{secret:?}");
        assert!(!debug.contains(TOKEN), "SecretBytes Debug 泄露凭据");
        let _ = secret.into_string().expect("UTF-8 凭据");

        // 失败路径:后端错误消息不得携带凭据值。
        let error =
            crate::ai_providers::save_api_key(&FailingCredentialStore, "preset-openai", TOKEN)
                .expect_err("后端失败必须上抛");
        assert!(!error.to_string().contains(TOKEN), "错误消息泄露凭据");

        // 捕获层自检:确认日志确实被捕获,后续断言不是空集。
        log::warn!(
            target: "audit",
            "credential boundary probe: target={}",
            "Slterm/Test/probe"
        );
        let captured = CAPTURED_LOGS.lock().expect("日志捕获锁");
        assert!(
            captured.iter().any(|(target, message)| target == "audit"
                && message.contains("credential boundary probe")),
            "捕获层未生效"
        );
        for (target, message) in captured.iter() {
            assert!(!target.contains(TOKEN), "日志 target 泄露凭据: {target}");
            assert!(!message.contains(TOKEN), "日志内容泄露凭据: {message}");
        }
    }
}
