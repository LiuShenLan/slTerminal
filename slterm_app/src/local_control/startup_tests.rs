//! 单实例启动分支的真实锁、发现、凭据与 loopback 移交组合测试。

use std::collections::HashMap;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use slterm_runtime_protocol::{
    ControlScope, DiscoveryRecord, DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError,
    JsonRpcErrorData,
};

use super::credentials::load_or_create_at;
use super::discovery::publish_at;
use super::service::LocalControlService;
use super::startup::{InstanceStartup, start_instance_at};
use crate::app_shell::SltermShellEvent;
use crate::persistence::metadata::{RunMarker, WorkspaceMetadata};
use crate::platform::instance::acquire_at;
use crate::secrets::{CredentialStore, SecretBytes};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct MemoryCredentialStore {
    entries: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryCredentialStore {
    fn replace_only_credential(&self) {
        let mut entries = self.entries.lock().expect("内存凭据锁");
        assert_eq!(entries.len(), 1, "服务只应创建一项 handover 凭据");
        let value = entries.values_mut().next().expect("唯一凭据");
        let replacement = if value.first() == Some(&b'0') {
            b'1'
        } else {
            b'0'
        };
        value.fill(replacement);
    }
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

struct CurrentDirectoryGuard(PathBuf);

impl Drop for CurrentDirectoryGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).expect("恢复测试前当前目录");
    }
}

fn enter_current_directory(path: &Path) -> CurrentDirectoryGuard {
    let previous = std::env::current_dir().expect("读取测试前当前目录");
    std::env::set_current_dir(path).expect("进入隔离当前目录");
    CurrentDirectoryGuard(previous)
}

fn metadata() -> WorkspaceMetadata {
    WorkspaceMetadata {
        store_epoch: "store-startup-test".to_owned(),
        run: RunMarker {
            run_id: "run-startup-test".to_owned(),
            process_id: 4321,
            process_created_at_ms: 1_700_000_000_123,
        },
        clean_shutdown: false,
    }
}

fn start_at(
    root: &Path,
    store: &dyn CredentialStore,
    working_directory: Option<&Path>,
) -> io::Result<InstanceStartup> {
    start_instance_at(
        &root.join("instance"),
        &root.join("endpoint.json"),
        root,
        working_directory,
        store,
    )
}

fn start_service(
    root: &Path,
    store: &dyn CredentialStore,
) -> (LocalControlService, mpsc::Receiver<SltermShellEvent>) {
    let (events, receiver) = mpsc::sync_channel(64);
    let mut service = LocalControlService::start_at(
        metadata(),
        events,
        store,
        &root.join("endpoint.json"),
        TEST_TIMEOUT,
    )
    .expect("绑定真实 loopback 服务");
    service.publish_discovery().expect("开放真实服务");
    (service, receiver)
}

fn reply(
    receiver: mpsc::Receiver<SltermShellEvent>,
    response: Result<HandoverResponse, JsonRpcError>,
) -> JoinHandle<HandoverParams> {
    std::thread::spawn(move || {
        let SltermShellEvent::Handover { params, completion } =
            receiver.recv_timeout(TEST_TIMEOUT).unwrap()
        else {
            panic!("服务只能投递 handover 事件");
        };
        completion.try_send(response).unwrap();
        params
    })
}

fn expect_error(result: io::Result<InstanceStartup>) -> io::Error {
    match result {
        Err(error) => error,
        Ok(InstanceStartup::Primary(_)) => panic!("失败路径不得返回可创建窗口的 Primary"),
        Ok(InstanceStartup::HandedOver) => panic!("失败路径不得返回 HandedOver"),
    }
}

#[test]
fn primary_does_not_evaluate_invalid_handover_directory() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let missing = temp.path().join("missing-project");

    let startup =
        start_at(temp.path(), &store, Some(&missing)).expect("Primary 不得提前校验移交目录");

    assert!(matches!(startup, InstanceStartup::Primary(_)));
    assert!(!temp.path().join("endpoint.json").exists());
    assert!(store.entries.lock().unwrap().is_empty());
}

#[test]
fn lock_conflict_focus_handover_returns_handed_over() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let (mut service, receiver) = start_service(temp.path(), &store);
    let foreground = reply(receiver, Ok(HandoverResponse::accepted()));

    let startup = start_at(temp.path(), &store, None).expect("Focus 移交必须成功");

    assert!(matches!(startup, InstanceStartup::HandedOver));
    assert_eq!(foreground.join().unwrap(), HandoverParams::Focus);
    service.shutdown().unwrap();
}

#[test]
fn lock_conflict_without_discovery_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    load_or_create_at(ControlScope::Handover, temp.path(), &store).unwrap();

    let error = expect_error(start_at(temp.path(), &store, None));

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn stale_discovery_endpoint_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    load_or_create_at(ControlScope::Handover, temp.path(), &store).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    drop(listener);
    let _discovery = publish_at(
        &temp.path().join("endpoint.json"),
        DiscoveryRecord {
            protocol_version: 2,
            endpoint,
            run_id: "run-stale-startup-test".to_owned(),
            process_id: 4321,
            process_created_at_ms: 1_700_000_000_123,
        },
    )
    .unwrap();

    expect_error(start_at(temp.path(), &store, None));
}

#[test]
fn wrong_handover_credential_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let (mut service, _receiver) = start_service(temp.path(), &store);
    store.replace_only_credential();

    expect_error(start_at(temp.path(), &store, None));
    service.shutdown().unwrap();
}

#[test]
fn closing_service_is_an_error_without_primary() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let (mut service, receiver) = start_service(temp.path(), &store);
    let foreground = reply(
        receiver,
        Err(JsonRpcError::new(
            -32004,
            "closing",
            Some(JsonRpcErrorData {
                code: DomainErrorCode::Closing,
            }),
        )),
    );

    expect_error(start_at(temp.path(), &store, None));
    foreground.join().unwrap();
    service.shutdown().unwrap();
}

#[test]
fn rejected_handover_is_an_error_without_primary() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let (mut service, receiver) = start_service(temp.path(), &store);
    let foreground = reply(
        receiver,
        Err(JsonRpcError::new(
            -32001,
            "not ready",
            Some(JsonRpcErrorData {
                code: DomainErrorCode::NotReady,
            }),
        )),
    );

    expect_error(start_at(temp.path(), &store, None));
    foreground.join().unwrap();
    service.shutdown().unwrap();
}

#[test]
fn valid_directory_maps_to_absolute_open_project() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let _current_directory = enter_current_directory(temp.path());
    let relative_project = Path::new("project");
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let (mut service, receiver) = start_service(temp.path(), &store);
    let foreground = reply(receiver, Ok(HandoverResponse::accepted()));

    let startup =
        start_at(temp.path(), &store, Some(relative_project)).expect("OpenProject 必须成功");

    assert!(matches!(startup, InstanceStartup::HandedOver));
    let expected = std::path::absolute(relative_project)
        .unwrap()
        .into_os_string()
        .into_string()
        .unwrap();
    assert_eq!(
        foreground.join().unwrap(),
        HandoverParams::OpenProject {
            project_root: expected,
        }
    );
    service.shutdown().unwrap();
}

#[test]
fn missing_handover_directory_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let missing = temp.path().join("missing-project");

    let error = expect_error(start_at(temp.path(), &store, Some(&missing)));

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn handover_file_path_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let file = temp.path().join("project.txt");
    std::fs::write(&file, "not a directory").unwrap();

    let error = expect_error(start_at(temp.path(), &store, Some(&file)));

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn non_unicode_handover_directory_is_rejected() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt as _;

    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let _primary = acquire_at(&temp.path().join("instance"))
        .unwrap()
        .expect("测试首实例必须持锁");
    let project: PathBuf = temp
        .path()
        .join(OsString::from_wide(&[b'p' as u16, 0xd800]));
    std::fs::create_dir(&project).unwrap();

    let error = expect_error(start_at(temp.path(), &store, Some(&project)));

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn non_conflict_lock_error_does_not_attempt_handover() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    std::fs::create_dir(temp.path().join("instance")).unwrap();

    let error = expect_error(start_at(temp.path(), &store, None));

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn ordinary_file_parent_already_exists_is_not_a_lock_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryCredentialStore::default();
    let parent = temp.path().join("ordinary-file");
    std::fs::write(&parent, "not a directory").unwrap();

    let error = expect_error(start_instance_at(
        &parent.join("instance"),
        &temp.path().join("missing-endpoint.json"),
        temp.path(),
        None,
        &store,
    ));

    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
}
