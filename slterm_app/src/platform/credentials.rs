//! Windows 凭据管理器通用封装。
//!
//! 分层:本模块是凭据管理器的唯一封装入口,target 命名空间归调用方;
//! Win32 原语在 [`windows`]。业务域消费只经 `crate::secrets` 三原语门面,
//! 禁止旁路直调本模块或 Win32。
//!
//! 与上游的差异:legacy 双身份迁移链与多平台后端(macOS Keychain/Linux
//! secret-tool)按定位约束砍除,`CredentialIdentity` 收敛为单一 `Slterm`
//! 单态;平台层即 Windows 原生落点,不再撒平台分支。

use std::borrow::Cow;
use std::io;

mod windows;

/// 应用级服务身份(写入凭据的 UserName 展示字段;读取/删除只认 target)。
const SERVICE: &str = "Slterm";

/// 凭据身份单态:service 恒 `Slterm`,target 由调用方按域命名
/// (`Slterm/AI/<id>` 等)。无 legacy 回退。
struct CredentialIdentity<'a> {
    service: &'static str,
    target: Cow<'a, str>,
}

impl<'a> CredentialIdentity<'a> {
    fn current(target: &'a str) -> Self {
        Self {
            service: SERVICE,
            target: Cow::Borrowed(target),
        }
    }
}

/// 读取 target 对应凭据;不存在返回 `Ok(None)`,后端失败返回 `Err`。
///
/// 返回裸字节仅限本层与 `crate::secrets` 门面;出域句柄由门面包
/// `SecretBytes`(`Zeroizing`)。
pub(crate) fn load(target: &str) -> io::Result<Option<Vec<u8>>> {
    windows::load_secret(&CredentialIdentity::current(target).target)
}

/// 写入凭据(覆盖同 target 旧值),UserName 恒为 `Slterm`。
pub(crate) fn store(target: &str, secret: &[u8]) -> io::Result<()> {
    let identity = CredentialIdentity::current(target);
    store_with_username(&identity.target, identity.service, secret)
}

/// 显式用户名写入(上游签名的保留面;Windows 仅影响凭据管理器的展示
/// 字段,不参与 target 查找)。
pub(crate) fn store_with_username(target: &str, username: &str, secret: &[u8]) -> io::Result<()> {
    if secret.is_empty() {
        // 空 blob 在读取侧归为「无凭据」,写入侧先拒以保证往返语义不歧义。
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential secret is empty",
        ));
    }
    windows::save_secret(target, username, secret)
}

/// 删除 target 对应凭据;不存在视为成功(幂等)。
pub(crate) fn delete(target: &str) -> io::Result<()> {
    windows::delete_secret(&CredentialIdentity::current(target).target)
}

#[cfg(test)]
mod credentials_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 测试 target 专用前缀:每次运行唯一,绝不触碰用户真实凭据。
    fn test_target(label: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时钟早于 UNIX_EPOCH")
            .as_nanos();
        format!("Slterm/Test/{label}-{}-{nanos}", std::process::id())
    }

    /// 用例结束(含 panic 展开)清理测试 target,不污染用户凭据库。
    struct Cleanup(String);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = delete(&self.0);
        }
    }

    #[test]
    fn empty_secret_is_rejected_before_the_backend() {
        let target = test_target("empty-secret");
        let error = store(&target, b"").expect_err("空凭据必须拒绝");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(load(&target).expect("读取空 target").is_none());
    }

    #[test]
    fn store_with_username_round_trips_and_replaces() {
        let target = test_target("with-username");
        let _cleanup = Cleanup(target.clone());
        store_with_username(&target, "slterm-test", b"sk-test-username-first").expect("首次写入");
        assert_eq!(
            load(&target).expect("读取").as_deref(),
            Some(b"sk-test-username-first".as_slice())
        );
        store_with_username(&target, "slterm-test", b"sk-test-username-second").expect("覆盖写入");
        assert_eq!(
            load(&target).expect("读取").as_deref(),
            Some(b"sk-test-username-second".as_slice())
        );
        delete(&target).expect("删除");
        assert!(load(&target).expect("读取已删除 target").is_none());
    }
}
