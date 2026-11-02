//! Runtime 与 handover 的独立凭据目标及生成、加载语义。

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use slterm_runtime_protocol::ControlScope;
use zeroize::Zeroizing;

use crate::platform::dirs;
use crate::secrets::{CredentialStore, SecretBytes};

const RANDOM_BYTES: usize = 32;
const CREDENTIAL_HEX_CHARS: usize = RANDOM_BYTES * 2;

/// 当前本地根下的作用域凭据目标；路径只以无损散列进入目标。
#[allow(dead_code)] // 消费点 = LocalControlServer 与 Runtime 发现链接线。
pub(crate) fn credential_target(scope: ControlScope) -> String {
    credential_target_at(scope, dirs::local_dir())
}

fn credential_target_at(scope: ControlScope, local_root: &Path) -> String {
    let scope = match scope {
        ControlScope::Runtime => "runtime",
        ControlScope::Handover => "handover",
    };
    let hash = dirs::stable_path_hash(local_root);
    let mut target = format!("slterm/local-control/{scope}/");
    for byte in hash {
        // 写入 String 不会失败；目标只包含作用域与散列。
        write!(&mut target, "{byte:02x}").expect("凭据目标格式化失败");
    }
    target
}

/// 客户端只读加载；不存在返回 `NotFound`，绝不生成或写入。
#[allow(dead_code)] // 消费点 = Runtime 客户端接线。
pub(crate) fn load_credential(
    scope: ControlScope,
    store: &dyn CredentialStore,
) -> io::Result<SecretBytes> {
    load_credential_at(scope, dirs::local_dir(), store)
}

pub(super) fn load_credential_at(
    scope: ControlScope,
    local_root: &Path,
    store: &dyn CredentialStore,
) -> io::Result<SecretBytes> {
    let target = credential_target_at(scope, local_root);
    let secret = store
        .load(&target)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "控制面凭据不存在"))?;
    validate_credential(&secret)?;
    Ok(secret)
}

/// 单写入者入口：仅已持 OS 单实例锁的首实例可调用；客户端必须只读加载。
/// 仅在凭据不存在时生成；非法持久值与后端错误均直接上抛。
#[allow(dead_code)] // 消费点 = 持单实例锁的 LocalControlServer 接线。
pub(crate) fn load_or_create_credential(
    scope: ControlScope,
    store: &dyn CredentialStore,
) -> io::Result<SecretBytes> {
    load_or_create_at(scope, dirs::local_dir(), store)
}

pub(super) fn load_or_create_at(
    scope: ControlScope,
    local_root: &Path,
    store: &dyn CredentialStore,
) -> io::Result<SecretBytes> {
    let target = credential_target_at(scope, local_root);
    if let Some(secret) = store.load(&target)? {
        validate_credential(&secret)?;
        return Ok(secret);
    }

    let secret = generate_credential()?;
    store.store(&target, secret.as_bytes())?;
    Ok(secret)
}

fn validate_credential(secret: &SecretBytes) -> io::Result<()> {
    let value = secret.as_bytes();
    if value.len() != CREDENTIAL_HEX_CHARS
        || !value
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "控制面持久凭据必须是 64 个小写 ASCII hex 字符",
        ));
    }
    Ok(())
}

fn generate_credential() -> io::Result<SecretBytes> {
    generate_credential_with(|random| {
        getrandom::fill(random)
            .map_err(|error| io::Error::other(format!("系统随机源失败: {error}")))
    })
}

fn generate_credential_with(
    fill_random: impl FnOnce(&mut [u8]) -> io::Result<()>,
) -> io::Result<SecretBytes> {
    let mut random = Zeroizing::new([0_u8; RANDOM_BYTES]);
    fill_random(&mut random[..])?;
    let mut encoded = Zeroizing::new(String::with_capacity(CREDENTIAL_HEX_CHARS));
    for byte in random.iter() {
        // hex 只含 ASCII，避免任意字节进入后续 JSON 协议。
        write!(&mut encoded, "{byte:02x}").expect("凭据 hex 格式化失败");
    }
    Ok(SecretBytes::new(encoded.as_bytes().to_vec()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use slterm_runtime_protocol::ControlScope;

    use super::*;
    use crate::secrets::{CredentialStore, SecretBytes};

    #[derive(Default)]
    struct MemoryCredentialStore {
        entries: Mutex<HashMap<String, Vec<u8>>>,
        writes: AtomicUsize,
        fail_load: bool,
        fail_store: bool,
    }

    impl MemoryCredentialStore {
        fn insert(&self, target: &str, value: Vec<u8>) {
            self.entries
                .lock()
                .expect("内存凭据锁")
                .insert(target.to_owned(), value);
        }

        fn stored(&self, target: &str) -> Option<Vec<u8>> {
            self.entries
                .lock()
                .expect("内存凭据锁")
                .get(target)
                .cloned()
        }

        fn writes(&self) -> usize {
            self.writes.load(Ordering::SeqCst)
        }
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load(&self, target: &str) -> io::Result<Option<SecretBytes>> {
            if self.fail_load {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected credential load failure",
                ));
            }
            Ok(self.stored(target).map(SecretBytes::new))
        }

        fn store(&self, target: &str, secret: &[u8]) -> io::Result<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.fail_store {
                return Err(io::Error::other("injected credential store failure"));
            }
            self.insert(target, secret.to_vec());
            Ok(())
        }

        fn delete(&self, target: &str) -> io::Result<()> {
            self.entries.lock().expect("内存凭据锁").remove(target);
            Ok(())
        }
    }

    fn secret_text(secret: SecretBytes) -> String {
        secret
            .into_string()
            .expect("控制面凭据必须是 UTF-8 hex")
            .to_string()
    }

    #[test]
    fn credential_targets_are_deterministic_hashed_and_root_isolated() {
        use sha2::{Digest as _, Sha256};
        use std::os::windows::ffi::OsStrExt;

        let root = Path::new(r"C:\slterm-test\root-a");
        let other_root = Path::new(r"C:\slterm-test\root-b");
        let target = credential_target_at(ControlScope::Runtime, root);
        assert_eq!(target, credential_target_at(ControlScope::Runtime, root));
        assert_ne!(
            target,
            credential_target_at(ControlScope::Runtime, other_root)
        );

        let wide_bytes: Vec<u8> = root
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect();
        let expected_hash: String = Sha256::digest(&wide_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            target,
            format!("slterm/local-control/runtime/{expected_hash}")
        );
        assert!(!target.contains("root-a"));
        assert!(!target.contains(&root.to_string_lossy().into_owned()));
    }

    #[test]
    fn runtime_and_handover_targets_are_independent() {
        let root = Path::new(r"C:\slterm-test\shared-root");
        assert_ne!(
            credential_target_at(ControlScope::Runtime, root),
            credential_target_at(ControlScope::Handover, root)
        );
    }

    #[test]
    fn windows_non_unicode_roots_cannot_collide_through_lossy_conversion() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let first = PathBuf::from(OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xD800,
        ]));
        let second = PathBuf::from(OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xD801,
        ]));
        assert_ne!(
            credential_target_at(ControlScope::Runtime, &first),
            credential_target_at(ControlScope::Runtime, &second)
        );
    }

    #[test]
    fn read_only_load_requires_existing_credential_without_writing() {
        let store = MemoryCredentialStore::default();
        let root = Path::new(r"C:\slterm-test\read-only-missing");

        let error = load_credential_at(ControlScope::Runtime, root, &store)
            .expect_err("客户端缺少凭据必须失败，不得生成");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(store.writes(), 0);
        assert!(
            store
                .stored(&credential_target_at(ControlScope::Runtime, root))
                .is_none()
        );
    }

    #[test]
    fn read_only_load_returns_existing_credential_without_writing() {
        let store = MemoryCredentialStore::default();
        let root = Path::new(r"C:\slterm-test\read-only-existing");
        let target = credential_target_at(ControlScope::Runtime, root);
        let persisted = "a".repeat(64).into_bytes();
        store.insert(&target, persisted.clone());

        let loaded = secret_text(
            load_credential_at(ControlScope::Runtime, root, &store)
                .expect("客户端只读加载已有凭据"),
        );
        assert_eq!(loaded.into_bytes(), persisted);
        assert_eq!(store.writes(), 0);
    }

    #[test]
    fn generated_credential_reloads_without_rotation() {
        let store = MemoryCredentialStore::default();
        let first = secret_text(
            load_or_create_credential(ControlScope::Runtime, &store)
                .expect("首次生成 Runtime 凭据"),
        );
        let second = secret_text(
            load_or_create_credential(ControlScope::Runtime, &store)
                .expect("再次加载 Runtime 凭据"),
        );

        assert_eq!(first, second);
        assert_eq!(store.writes(), 1, "重载不得再次写入或轮换凭据");
        assert_eq!(
            store.stored(&credential_target(ControlScope::Runtime)),
            Some(first.into_bytes())
        );
    }

    #[test]
    fn runtime_and_handover_credentials_have_independent_values() {
        let store = MemoryCredentialStore::default();
        let root = Path::new(r"C:\slterm-test\scope-values");
        let runtime = secret_text(
            load_or_create_at(ControlScope::Runtime, root, &store).expect("生成 Runtime 凭据"),
        );
        let handover = secret_text(
            load_or_create_at(ControlScope::Handover, root, &store).expect("生成 handover 凭据"),
        );

        assert_ne!(runtime, handover);
        assert_eq!(store.writes(), 2);
        assert_eq!(
            store.stored(&credential_target_at(ControlScope::Runtime, root)),
            Some(runtime.into_bytes())
        );
        assert_eq!(
            store.stored(&credential_target_at(ControlScope::Handover, root)),
            Some(handover.into_bytes())
        );
    }

    #[test]
    fn generation_consumes_exactly_256_bits_and_encodes_lowercase_hex() {
        let secret = generate_credential_with(|random| {
            assert_eq!(random.len(), 32);
            for (index, byte) in random.iter_mut().enumerate() {
                *byte = index as u8;
            }
            Ok(())
        })
        .expect("注入随机源生成凭据");
        let text = secret_text(secret);

        assert_eq!(text.len(), 64);
        assert!(
            text.bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        assert_eq!(
            text,
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        );
    }

    #[test]
    fn invalid_persisted_credentials_are_rejected_without_overwrite() {
        let root = Path::new(r"C:\slterm-test\invalid-values");
        let target = credential_target_at(ControlScope::Runtime, root);
        for invalid in [
            Vec::new(),
            vec![b'a'; 63],
            vec![b'a'; 65],
            vec![b'A'; 64],
            b"sk-test-not-hex".to_vec(),
        ] {
            let store = MemoryCredentialStore::default();
            store.insert(&target, invalid.clone());

            let error = load_or_create_at(ControlScope::Runtime, root, &store)
                .expect_err("非法持久凭据必须明确失败");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            let error = load_credential_at(ControlScope::Runtime, root, &store)
                .expect_err("只读加载同样拒绝非法持久凭据");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(store.stored(&target), Some(invalid), "失败不得覆盖原值");
            assert_eq!(store.writes(), 0, "非法持久值不得触发轮换写入");
        }
    }

    #[test]
    fn credential_backend_errors_are_propagated() {
        let root = Path::new(r"C:\slterm-test\backend-errors");
        let load_failure = MemoryCredentialStore {
            fail_load: true,
            ..MemoryCredentialStore::default()
        };
        let error = load_or_create_at(ControlScope::Runtime, root, &load_failure)
            .expect_err("读取失败必须上抛");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(load_failure.writes(), 0);

        let store_failure = MemoryCredentialStore {
            fail_store: true,
            ..MemoryCredentialStore::default()
        };
        let error = load_or_create_at(ControlScope::Runtime, root, &store_failure)
            .expect_err("写入失败不得伪装成功");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(store_failure.writes(), 1);
        assert!(
            store_failure
                .stored(&credential_target_at(ControlScope::Runtime, root))
                .is_none()
        );
    }

    #[test]
    fn random_source_errors_are_propagated() {
        let error = generate_credential_with(|_| {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "injected random source failure",
            ))
        })
        .expect_err("随机源失败必须上抛");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn credential_values_do_not_appear_in_debug_or_validation_errors() {
        let secret = generate_credential_with(|random| {
            random.fill(0xab);
            Ok(())
        })
        .expect("生成调试边界夹具");
        let plaintext = "ab".repeat(32);
        assert!(!format!("{secret:?}").contains(&plaintext));

        const INVALID: &[u8] = b"sk-test-sensitive-fixture";
        let root = Path::new(r"C:\slterm-test\error-redaction");
        let store = MemoryCredentialStore::default();
        store.insert(
            &credential_target_at(ControlScope::Runtime, root),
            INVALID.to_vec(),
        );
        let error =
            load_or_create_at(ControlScope::Runtime, root, &store).expect_err("非法凭据必须失败");
        assert!(
            !error
                .to_string()
                .contains(std::str::from_utf8(INVALID).unwrap())
        );
        assert!(!format!("{error:?}").contains(std::str::from_utf8(INVALID).unwrap()));
    }
}
