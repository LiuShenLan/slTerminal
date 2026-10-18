//! handoff 文件态机、restore ticket 与假 release 服务器全链演练。
//!
//! 全部用例使用 tempdir 数据根 + 本地回环夹具;不触碰真实用户目录与网络。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::json;
use sha2::{Digest as _, Sha256};

use super::*;
use crate::update_download::{DownloadContext, begin, begin_download_session, run_in, status};

fn installer_asset(body: &[u8]) -> UpdateAsset {
    let name = "slTerminal-v1.6.0-windows-x64-setup.exe";
    UpdateAsset {
        version: "1.6.0".to_owned(),
        url: format!("https://github.com/LiuShenLan/slTerminal/releases/download/v1.6.0/{name}"),
        name: name.to_owned(),
        size: Some(body.len() as u64),
        digest: Some(
            Sha256::digest(body)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        ),
    }
}

/// 把已验证安装包写进 `<root>/updates`,并以真实下载状态机置 Ready。
fn stage_ready_package(root: &Path, asset: &UpdateAsset, body: &[u8]) {
    crate::update_download::reset_session();
    let directory = root.join("updates");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(&asset.name), body).unwrap();
    let context = DownloadContext {
        root: root.to_owned(),
        origin: None,
    };
    let job = begin_download_session(asset).unwrap();
    run_in(job, &context);
    assert!(
        matches!(
            status(asset),
            crate::update_download::DownloadStatus::Ready { .. }
        ),
        "staged package must be Ready"
    );
}

fn fixture_paths(root: &Path) -> (PreparePaths, PathBuf) {
    let installation = root.join("install");
    std::fs::create_dir_all(&installation).unwrap();
    std::fs::write(installation.join("unins000.exe"), b"MZ").unwrap();
    let executable = std::env::current_exe().unwrap();
    (
        PreparePaths {
            root: root.to_owned(),
            executable: executable.clone(),
            installation,
            config_directory: root.to_owned(),
        },
        executable,
    )
}

/// 脚本化 helper:写 ready.json 后等 commit/cancel;commit 后写成功结果。
struct FakeHelper {
    alive: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl HelperProcess for FakeHelper {
    fn running(&mut self) -> Result<bool, String> {
        Ok(self.alive.load(Ordering::SeqCst))
    }

    fn terminate(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn spawn_fake_helper(directory: &Path, plan_path: &Path) -> Result<Box<dyn HelperProcess>, String> {
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(plan_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let transaction = plan["transaction"]
        .as_str()
        .ok_or("missing transaction")?
        .to_owned();
    let version = plan["version"]
        .as_str()
        .ok_or("missing version")?
        .to_owned();
    crate::atomic_file::write(
        &directory.join("ready.json"),
        &serde_json::to_vec(&json!({ "transaction": transaction })).unwrap(),
    )
    .map_err(|error| error.to_string())?;

    let alive = Arc::new(AtomicBool::new(true));
    let worker_alive = alive.clone();
    let directory = directory.to_owned();
    let handle = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while worker_alive.load(Ordering::SeqCst) && Instant::now() < deadline {
            if directory.join("commit.json").is_file() {
                let result = json!({
                    "transaction": transaction,
                    "success": true,
                    "version": version,
                });
                let _ = crate::atomic_file::write(
                    &directory.join("result.json"),
                    &serde_json::to_vec(&result).unwrap(),
                );
                break;
            }
            if directory.join("cancel.json").is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        worker_alive.store(false, Ordering::SeqCst);
    });
    Ok(Box::new(FakeHelper {
        alive,
        handle: Some(handle),
    }))
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn handoff_file_machine_commits_and_cancels_without_installation_authority() {
    let root = tempfile::tempdir().unwrap();
    let body = b"MZhandoff fixture installer";
    let asset = installer_asset(body);
    stage_ready_package(root.path(), &asset, body);
    let (paths, _) = fixture_paths(root.path());

    // plan → ready → commit。
    let mut prepared = prepare_with(&asset, paths.clone(), spawn_fake_helper).unwrap();
    let directory = prepared.directory.clone();
    assert!(directory.join("plan.json").is_file());
    assert!(directory.join("ready.json").is_file());
    assert!(!directory.join("commit.json").exists());
    assert_eq!(transaction_state(&directory), TransactionState::Prepared);

    let workspace = vec![Session::new(0, Vec::new())];
    prepared.commit(&workspace).unwrap();
    assert!(directory.join("commit.json").is_file());
    assert!(directory.join("workspace.json").is_file());
    wait_for(&directory.join("result.json"));
    assert_eq!(transaction_state(&directory), TransactionState::Finished);
    drop(prepared);

    // plan → ready → cancel(Drop 未 commit)。
    let mut prepared = prepare_with(&asset, paths, spawn_fake_helper).unwrap();
    let directory = prepared.directory.clone();
    assert!(directory.join("ready.json").is_file());
    prepared.helper.terminate();
    drop(prepared);
    assert!(directory.join("cancel.json").is_file());
    assert!(!directory.join("commit.json").exists());
    assert_eq!(transaction_state(&directory), TransactionState::Cancelled);
}

#[test]
fn orphan_and_prepared_transactions_are_cancelled_on_recovery() {
    let root = tempfile::tempdir().unwrap();
    let handoffs = root.path().join("updates/handoffs");
    let make = |name: &str, files: &[&str]| {
        let directory = handoffs.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        for file in files {
            std::fs::write(directory.join(file), b"{}").unwrap();
        }
        directory
    };
    let orphan = make("orphan", &["plan.json"]);
    let prepared = make("prepared", &["plan.json", "ready.json"]);
    let committed = make("committed", &["plan.json", "ready.json", "commit.json"]);
    let finished = make("finished", &["plan.json", "ready.json", "result.json"]);

    recover_stale_handoffs(root.path());

    assert!(
        orphan.join("cancel.json").is_file(),
        "孤儿 plan 必须按 cancel 收尾"
    );
    assert!(
        prepared.join("cancel.json").is_file(),
        "未 commit 的 ready 事务必须取消"
    );
    assert!(
        !committed.join("cancel.json").exists(),
        "已 commit 的事务有安装权威"
    );
    assert!(!finished.join("cancel.json").exists(), "终态事务不得改写");
    assert_eq!(transaction_state(&orphan), TransactionState::Cancelled);
}

#[test]
fn restore_ticket_covers_success_rollback_attempts_and_acknowledgement() {
    crate::update_download::handoff::reset_restore_state();
    let root = tempfile::tempdir().unwrap();
    let executable = std::env::current_exe().unwrap();
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let directory = root.path().join("updates/handoffs/restore-test");
    std::fs::create_dir_all(&directory).unwrap();
    let plan = Plan {
        schema: PLAN_SCHEMA,
        asset: installer_asset(b"MZrestore fixture"),
        transaction: "restore-test".into(),
        executable: canonical(&executable).unwrap(),
        installation: root.path().join("install"),
        config_directory: root.path().to_owned(),
        installer: root.path().join("fixture"),
        sha256: String::new(),
        bytes: 1,
        version: version.clone(),
        original_version: version.clone(),
        guard_path: root.path().join("guard"),
        participants: Vec::new(),
    };
    let save = |name: &str, value: serde_json::Value| {
        std::fs::write(directory.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    };
    std::fs::write(
        root.path().join("updates/last-handoff.json"),
        serde_json::to_vec(&directory.join("plan.json")).unwrap(),
    )
    .unwrap();
    save(
        "workspace.json",
        serde_json::to_value(vec![Session::new(0, Vec::new())]).unwrap(),
    );
    save("plan.json", serde_json::to_value(&plan).unwrap());

    // 事务不匹配 → 拒绝。
    save(
        "result.json",
        json!({ "transaction": "wrong", "success": true }),
    );
    assert!(restore_ticket_at(root.path(), &executable, &version).is_none());

    // 成功恢复 → 计数 +1;确认窗口数不符不留档,相符才留档。
    save(
        "result.json",
        json!({ "transaction": "restore-test", "success": true }),
    );
    assert_eq!(
        restore_ticket_at(root.path(), &executable, &version)
            .unwrap()
            .len(),
        1
    );
    acknowledge_restore(2);
    assert!(!directory.join("restored.json").exists());
    acknowledge_restore(1);
    assert!(directory.join("restored.json").exists());
    assert!(
        restore_ticket_at(root.path(), &executable, &version).is_none(),
        "已确认的恢复不得重放"
    );
    std::fs::remove_file(directory.join("restored.json")).unwrap();

    // 崩溃循环上限:第 3 次尝试后拒绝。
    save("restore-attempts.json", json!(MAX_RESTORE_ATTEMPTS));
    assert!(restore_ticket_at(root.path(), &executable, &version).is_none());
    save("restore-attempts.json", json!(0));

    // 回滚恢复:失败但原版本仍在运行。
    save(
        "result.json",
        json!({
            "transaction": "restore-test",
            "success": false,
            "recovered_original": true,
        }),
    );
    assert!(restore_ticket_at(root.path(), &executable, &version).is_some());

    // 版本不匹配 → 拒绝。
    assert!(restore_ticket_at(root.path(), &executable, "9.9.9").is_none());
}

#[test]
fn schedule_requires_a_ready_package_and_writes_the_arm_file() {
    let root = tempfile::tempdir().unwrap();
    let body = b"MZschedule fixture";
    let asset = installer_asset(body);
    stage_ready_package(root.path(), &asset, body);

    schedule_at(root.path(), &asset).unwrap();
    let armed: UpdateAsset = serde_json::from_slice(
        &std::fs::read(root.path().join("updates/install-next.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(armed, asset);
}

#[test]
fn stale_failure_evidence_is_superseded_by_a_newer_download_record() {
    let root = tempfile::tempdir().unwrap();
    let body = b"MZfailure fixture";
    let asset = installer_asset(body);
    stage_ready_package(root.path(), &asset, body);
    let (paths, executable) = fixture_paths(root.path());
    let mut prepared = prepare_with(&asset, paths, spawn_fake_helper).unwrap();
    let directory = prepared.directory.clone();
    committed_failure(&mut prepared, &directory);

    // result 比 download.json 新 → 失败可见。
    let seen = failed_update_at(root.path(), &executable).expect("failed update");
    assert_eq!(seen.0, asset);
    assert_eq!(seen.1, "setup failed");

    // 「未见过」判定 = result.json mtime 晚于提示状态 mtime。
    let prompt = root.path().join("update_state.json");
    std::fs::write(&prompt, b"{}").unwrap();
    assert!(!super::failure_unseen(
        root.path(),
        Some(&executable),
        &prompt
    ));
    std::thread::sleep(Duration::from_millis(20));
    let transaction = directory.file_name().unwrap().to_str().unwrap().to_owned();
    crate::atomic_file::write(
        &directory.join("result.json"),
        &serde_json::to_vec(&json!({
            "transaction": transaction,
            "success": false,
            "recovered_original": false,
            "error": "setup failed",
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(super::failure_unseen(
        root.path(),
        Some(&executable),
        &prompt
    ));

    // 新下载记录(re-download)比 result 新 → 旧失败不再重提示。
    std::thread::sleep(Duration::from_millis(20));
    stage_ready_package(root.path(), &asset, body);
    assert!(failed_update_at(root.path(), &executable).is_none());
}

/// 走完 commit 授权,再写一份失败 result(模拟 installer 失败)。
fn committed_failure(prepared: &mut PreparedUpdate, directory: &Path) {
    prepared
        .commit(&[Session::new(0, Vec::new())])
        .expect("commit");
    // 先终止脚本化 helper,避免它抢写成功 result(真实 helper 的失败
    // result 由 helper 侧原子写入,这里由用例直接模拟)。
    prepared.helper.terminate();
    let transaction = prepared.transaction.clone();
    let result = json!({
        "transaction": transaction,
        "success": false,
        "recovered_original": false,
        "error": "setup failed",
    });
    crate::atomic_file::write(
        &directory.join("result.json"),
        &serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
}

#[test]
fn installation_guard_lock_reports_an_in_progress_transaction() {
    let root = tempfile::tempdir().unwrap();
    let executable = root.path().join("slterm.exe");
    std::fs::write(&executable, b"MZ").unwrap();
    assert!(!installation_in_progress_at(&executable).unwrap());
    let guard = crate::atomic_file::try_lifetime_lock(
        &crate::platform::update_installation::guard_base(&canonical(&executable).unwrap()),
    )
    .unwrap()
    .expect("guard lock");
    assert!(installation_in_progress_at(&executable).unwrap());
    drop(guard);
    assert!(!installation_in_progress_at(&executable).unwrap());
}

/// 假 release 服务器全链:检查 → 资产选择 → 流式下载校验 → handoff 事务。
#[test]
fn fake_release_server_full_chain_rehearsal_is_green() {
    use crate::update_proxy::test_support::{Server, response};

    let body = b"MZfull-chain rehearsal installer";
    let asset = installer_asset(body);
    let release = json!({
        "tag_name": "v1.6.0",
        "body": "",
        "assets": [{
            "name": asset.name,
            "browser_download_url": asset.url,
            "size": body.len(),
            "digest": format!("sha256:{}", asset.digest.clone().unwrap()),
        }],
    });
    let server = Server::start(vec![
        response(
            "200 OK",
            "Content-Type: application/json\r\n",
            &serde_json::to_string(&release).unwrap(),
        ),
        response("200 OK", "", std::str::from_utf8(body).unwrap()),
    ]);

    // 1. 检查(API 请求经夹具代理,URL 用 http 避免本地 TLS)。
    let checked =
        crate::update_check::check_from(&server.agent(&[]), "http://api.update.invalid/latest")
            .expect("fake release check");
    assert!(checked.update_available);
    assert_eq!(checked.latest, "1.6.0");
    let asset = checked.asset.expect("exact x64 installer");

    // 2. 下载 + 四重校验(origin 指向同一夹具,直连)。
    let root = tempfile::tempdir().unwrap();
    crate::update_download::reset_session();
    let job = begin(&asset).unwrap().expect("download session");
    run_in(
        job,
        &DownloadContext {
            root: root.path().to_owned(),
            origin: Some(format!("http://{}", server.address)),
        },
    );
    match status(&asset) {
        crate::update_download::DownloadStatus::Ready { path, bytes } => {
            assert_eq!(bytes, body.len() as u64);
            assert_eq!(std::fs::read(path).unwrap(), body);
        }
        other => panic!("download must be ready, got {other:?}"),
    }

    // 3. handoff:plan → ready → commit;helper 写成功结果。
    let (paths, _) = fixture_paths(root.path());
    let mut prepared = prepare_with(&asset, paths, spawn_fake_helper).expect("handoff prepare");
    let directory = prepared.directory.clone();
    assert!(directory.join("plan.json").is_file());
    assert!(directory.join("ready.json").is_file());
    prepared
        .commit(&[Session::new(0, Vec::new())])
        .expect("commit authorizes installation");
    wait_for(&directory.join("result.json"));
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["success"], true);
    assert_eq!(result["version"], "1.6.0");

    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    // 经代理的检查请求记录内层路径;直连的下载请求记录完整来源路径。
    assert!(requests[0].contains("/latest"), "{requests:?}");
    assert!(requests[1].contains(&asset.name), "{requests:?}");
}
