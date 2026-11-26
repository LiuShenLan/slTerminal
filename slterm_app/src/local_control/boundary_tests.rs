//! Runtime 凭据边界集成锚：发现输出、轮换隔离与端点材料不含 token。

use std::collections::HashMap;
use std::io;
use std::sync::Mutex;

use slterm_path::AppPaths;
use slterm_runtime_protocol::{ControlScope, DiscoveryRecord};

use super::credentials::{
    credential_target_at, load_credential_at, load_or_create_at, rotate_runtime_at,
};
use super::discovery::{discover_json_at, publish_at};
use crate::secrets::{CredentialStore, SecretBytes};

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
            .map(|value| SecretBytes::new(value.clone())))
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

fn secret_text(secret: SecretBytes) -> String {
    secret
        .into_string()
        .expect("控制面凭据必须是 UTF-8 hex")
        .to_string()
}

fn record() -> DiscoveryRecord {
    DiscoveryRecord {
        protocol_version: 2,
        endpoint: "127.0.0.1:4242".parse().unwrap(),
        run_id: "run-boundary".to_owned(),
        process_id: 123,
        process_created_at_ms: 456,
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() >= 16
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// 真实临时数据根上的发现与轮换边界：
/// 端点文件不带 token；discover JSON 只输出 Runtime token；
/// 轮换后旧 token 失效、handover 凭据原样保留。
#[test]
fn integration_runtime_credential_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let endpoint = paths.local_root().join("local-control-endpoint.json");
    let mut guard = publish_at(&endpoint, record()).unwrap();
    let store = MemoryCredentialStore::default();
    let runtime =
        secret_text(load_or_create_at(ControlScope::Runtime, paths.local_root(), &store).unwrap());
    let handover =
        secret_text(load_or_create_at(ControlScope::Handover, paths.local_root(), &store).unwrap());
    assert_ne!(runtime, handover, "两个作用域凭据必须独立生成");

    // 端点文件只含非敏感元数据，任何凭据都不得出现。
    let published = std::fs::read(&endpoint).unwrap();
    assert!(!contains_bytes(&published, runtime.as_bytes()));
    assert!(!contains_bytes(&published, handover.as_bytes()));
    assert!(String::from_utf8_lossy(&published).contains("run-boundary"));

    // discover --json 是唯一 token 输出：含 Runtime token，绝不含 handover token。
    let discovered: serde_json::Value =
        serde_json::from_str(&discover_json_at(&paths, &store).unwrap()).unwrap();
    assert_eq!(discovered["protocol_version"], 2);
    assert_eq!(discovered["endpoint"], "127.0.0.1:4242");
    assert_eq!(discovered["run_id"], "run-boundary");
    assert_eq!(
        discovered["runtime_token"],
        serde_json::Value::String(runtime.clone())
    );
    assert!(discovered.get("handover_token").is_none());
    assert!(!discovered.to_string().contains(&handover));

    // 轮换只换 Runtime：旧值失效，handover 原值保留。
    rotate_runtime_at(&paths, &store).unwrap();
    let rotated =
        secret_text(load_credential_at(ControlScope::Runtime, paths.local_root(), &store).unwrap());
    assert_ne!(rotated, runtime);
    assert_eq!(
        secret_text(
            load_credential_at(ControlScope::Handover, paths.local_root(), &store).unwrap()
        ),
        handover
    );
    let persisted = store
        .load(&credential_target_at(
            ControlScope::Runtime,
            paths.local_root(),
        ))
        .unwrap()
        .expect("轮换必须持久写入 Runtime 目标");
    assert_eq!(secret_text(persisted), rotated, "持久值必须等于轮换后的值");

    let after: serde_json::Value =
        serde_json::from_str(&discover_json_at(&paths, &store).unwrap()).unwrap();
    assert_eq!(
        after["runtime_token"],
        serde_json::Value::String(rotated.clone())
    );
    assert_ne!(
        after["runtime_token"],
        serde_json::Value::String(runtime.clone())
    );

    // 轮换前后端点文件都不含任一 token。
    let published = std::fs::read(&endpoint).unwrap();
    for token in [&runtime, &rotated, &handover] {
        assert!(!contains_bytes(&published, token.as_bytes()));
    }

    // 设置文件不属于凭据域：创建与轮换都不得把任一 token 写入设置材料。
    let settings = std::fs::read(paths.settings_path()).unwrap_or_default();
    for token in [&runtime, &rotated, &handover] {
        assert!(!contains_bytes(&settings, token.as_bytes()));
    }

    guard.revoke().unwrap();
}
