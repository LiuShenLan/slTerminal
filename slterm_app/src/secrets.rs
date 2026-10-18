//! 凭据域消费接口(10 篇;M9.2 过渡形态)。
//!
//! 本模块只定义 app 侧消费凭据的 trait 与出域句柄,不触碰任何真实凭据
//! 存储。真实三原语(Windows 凭据管理器 `CredReadW`/`CredWriteW`/
//! `CredDeleteW`)与 target 命名空间实现归 M10.9;M9.2 用注入的 mock 实现
//! 断言调用契约,测试只允许 `sk-test` 形态假值。
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

/// 凭据后端消费接口(10 篇三原语的应用层投影)。
///
/// 真实实现归 M10.9;本步只以 mock 断言「先写凭据后回写引用」「先删凭据
/// 再删元数据」等调用契约。`exists` 默认经 `load` 推导,真实后端可覆写为
/// 不取回明文的探测。
pub(crate) trait CredentialStore {
    fn load(&self, target: &str) -> io::Result<Option<SecretBytes>>;
    fn store(&self, target: &str, secret: &[u8]) -> io::Result<()>;
    fn delete(&self, target: &str) -> io::Result<()>;

    #[allow(dead_code)] // 消费点 = 设置页凭据状态展示(M10.9 后);本步由 mock 用例锁定语义。
    fn exists(&self, target: &str) -> io::Result<bool> {
        Ok(self.load(target)?.is_some())
    }
}

#[cfg(test)]
mod secrets_tests {
    use super::*;

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
}
