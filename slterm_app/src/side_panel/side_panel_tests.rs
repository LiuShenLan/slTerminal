//! 文件树模型用例:根跟随/窗口内浏览/展开/刷新/搜索恢复(语义)。

use super::*;
use std::time::{Duration, Instant};

fn wait_for_index(panel: &mut SidePanel, matches: impl Fn(&SidePanel) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        panel.harvest_pending();
        if matches(panel) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("side panel did not reach the expected state");
}

fn remove_test_directory(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error)
                if cfg!(windows)
                    && matches!(error.raw_os_error(), Some(32 | 33))
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!(
                "could not remove test directory {}: {error}",
                path.display()
            ),
        }
    }
}

/// 根跟随:聚焦 cwd 变化切换根,同 cwd 的手动浏览保留到位置真的变化。
#[test]
fn custom_root_survives_same_cwd_but_releases_when_terminal_moves() {
    let base = std::env::temp_dir().join(format!("slterm-panel-root-test-{}", std::process::id()));
    let cwd = base.join("cwd");
    let custom = base.join("custom");
    let next_cwd = base.join("next-cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::create_dir_all(&next_cwd).unwrap();

    let mut panel = SidePanel::new();
    assert!(panel.sync(Some(cwd.clone())));
    assert!(panel.set_custom_root(custom.clone()));
    assert!(panel.custom_root_active());

    panel.sync(Some(cwd));
    assert_eq!(panel.root(), Some(custom.as_path()));
    assert!(panel.custom_root_active());

    assert!(panel.sync(Some(next_cwd.clone())));
    assert!(!panel.custom_root_active());
    assert_eq!(panel.root(), Some(next_cwd.as_path()));

    panel.wait_snapshot();
    panel.file_index.release_for_test();
    remove_test_directory(&base);
}

/// 指定根失效:回跟随最近 cwd,并给出可本地化提示。
#[test]
fn missing_custom_root_returns_to_latest_cwd_with_visible_feedback() {
    let base = std::env::temp_dir().join(format!(
        "slterm-panel-missing-root-test-{}",
        std::process::id()
    ));
    let cwd = base.join("cwd");
    let custom = base.join("custom");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&custom).unwrap();

    let mut panel = SidePanel::new();
    panel.sync(Some(cwd.clone()));
    assert!(panel.set_custom_root(custom.clone()));
    std::fs::remove_dir_all(&custom).unwrap();

    assert!(panel.sync(Some(cwd.clone())));
    assert!(!panel.custom_root_active());
    assert_eq!(panel.root(), Some(cwd.as_path()));
    assert_eq!(panel.root_notice(), Some(&PanelNotice::FollowingDirectory));
    assert_eq!(
        panel.localized_root_notice(UiLanguage::EnUs),
        Some("The selected directory is unavailable; following the current directory")
    );

    panel.wait_snapshot();
    panel.file_index.release_for_test();
    remove_test_directory(&base);
}

/// 目录行:目录在前、点一次展开、`..` 行切换窗口内根且不碰终端 cwd。
#[test]
fn tree_lists_dirs_first_and_expands_on_click() {
    let base = std::env::temp_dir().join(format!("slterm-panel-test-{}", std::process::id()));
    let sub = base.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(base.join("a.txt"), "x").unwrap();
    std::fs::write(sub.join("inner.txt"), "y").unwrap();

    let mut panel = SidePanel::new();
    panel.sync(Some(base.clone()));
    panel.wait_snapshot();
    {
        let rows = panel.file_rows();
        assert_eq!(rows[0].name, "..", "上级导航行常驻顶行");
        assert!(rows[0].is_parent);
        assert_eq!(rows[1].name, "sub", "目录排在文件前");
        assert_eq!(rows.len(), 3, "折叠目录隐藏子项");
    }

    assert!(panel.click_row(1), "点目录切换展开");
    {
        let rows = panel.file_rows();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2].name, "inner.txt");
        assert_eq!(rows[2].depth, 1);
    }

    assert!(panel.click_row(0), "点 `..` 切换窗口内根");
    assert_eq!(panel.root(), Some(base.parent().unwrap()));
    assert!(panel.custom_root_active(), "向上浏览是窗口内覆盖");

    panel.wait_snapshot();
    panel.file_index.release_for_test();
    remove_test_directory(&base);
}

/// 搜索与树快照分层:非空 query 换平铺结果;清空立即恢复 tree_rows 不走盘。
#[test]
fn clearing_search_restores_tree_rows_without_walking_the_disk() {
    let directory = tempfile::tempdir().unwrap();
    let needle = directory.path().join("needle.txt");
    std::fs::write(&needle, b"").unwrap();

    let mut panel = SidePanel::new();
    panel.sync(Some(directory.path().to_owned()));
    panel.wait_snapshot();
    assert!(panel.file_rows().iter().any(|row| row.name == "needle.txt"));

    panel.set_file_search_query("needle".to_owned());
    wait_for_index(&mut panel, |panel| {
        !panel.file_search_pending()
            && panel.search_memory.is_some()
            && panel.file_rows().iter().any(|row| row.name == "needle.txt")
    });
    let epoch = panel.search_index_epoch;
    let generation = panel.search_generation;

    // 磁盘上删掉文件后清空 query:恢复的是已缓存的 tree_rows,不重新走盘,
    // 所以旧行仍在——这正是不走盘的证据。
    std::fs::remove_file(&needle).unwrap();
    panel.set_file_search_query(String::new());
    assert!(panel.file_rows().iter().any(|row| row.name == "needle.txt"));
    assert_eq!(panel.search_index_epoch, epoch);
    assert_ne!(
        panel.search_generation, generation,
        "清空同样推进 generation"
    );

    panel.file_index.release_for_test();
}

/// 根变化推进搜索 epoch;同根刷新不推进(缓存复用)。
#[test]
fn search_index_epoch_changes_only_with_the_root() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut panel = SidePanel::new();
    panel.root = Some(first.path().to_owned());
    panel.search = "needle".to_owned();
    panel.sync_file_index();
    let epoch = panel.search_index_epoch;
    let generation = panel.search_generation;
    panel.sync_file_index();
    assert_eq!(panel.search_index_epoch, epoch);
    assert_eq!(panel.search_generation, generation);

    panel.root = Some(second.path().to_owned());
    panel.sync_file_index();
    assert_ne!(panel.search_index_epoch, epoch);
    assert_ne!(panel.search_generation, generation);
    assert!(panel.rows.is_empty());
    panel.file_index.release_for_test();
}

/// generation 守卫:worker 跑动期间根又变时,旧快照被丢弃并重臂。
#[test]
fn stale_snapshot_generation_is_discarded_and_rearmed() {
    let root = std::env::temp_dir();
    let mut panel = SidePanel::new();
    panel.root = Some(root.clone());
    panel.snapshot_generation = 2;
    *panel.snapshot_slot.lock().unwrap() = Some(PanelSnapshot {
        generation: 1,
        root: root.clone(),
        rows: vec![super::FileRow {
            path: root.join("stale"),
            name: "stale".into(),
            depth: 0,
            is_dir: false,
            expanded: false,
            is_parent: false,
            ignored: false,
        }],
        enumeration_ok: true,
        git: None,
    });
    assert!(
        panel.harvest_snapshot(),
        "丢弃旧快照也要唤醒 UI,重臂刷新才有下一帧"
    );
    assert!(panel.tree_rows.is_empty(), "旧 generation 的行不能落地");
    assert!(panel.needs_refresh, "丢弃后必须为新快照重臂");
}

/// 目录行先发布、VCS 后补全:中间快照不清空已显示的 git 状态。
#[test]
fn partial_snapshot_keeps_vcs_until_the_final_snapshot() {
    let root = PathBuf::from("D:/repo");
    let mut panel = SidePanel::new();
    panel.root = Some(root.clone());
    panel.snapshot_generation = 1;
    panel.git = Some(Arc::new(GitInfo {
        branch: "keep-until-ready".to_owned(),
        ..GitInfo::default()
    }));

    *panel.snapshot_slot.lock().unwrap() = Some(PanelSnapshot {
        generation: 1,
        root: root.clone(),
        rows: Vec::new(),
        enumeration_ok: true,
        git: None,
    });
    assert!(panel.harvest_snapshot());
    assert_eq!(
        panel.git().map(|git| git.branch.as_str()),
        Some("keep-until-ready")
    );

    *panel.snapshot_slot.lock().unwrap() = Some(PanelSnapshot {
        generation: 1,
        root,
        rows: Vec::new(),
        enumeration_ok: true,
        git: Some(None),
    });
    assert!(panel.harvest_snapshot());
    assert!(panel.git().is_none(), "最终快照明确「不在仓库中」才清空");
}

/// 枚举失败与真空目录必须可区分。
#[test]
fn failed_enumeration_is_not_reported_as_an_empty_directory() {
    let mut panel = SidePanel::new();
    panel.root = Some(PathBuf::from("D:/does-not-exist"));
    panel.snapshot_generation = 1;
    *panel.snapshot_slot.lock().unwrap() = Some(PanelSnapshot {
        generation: 1,
        root: PathBuf::from("D:/does-not-exist"),
        rows: Vec::new(),
        enumeration_ok: false,
        git: None,
    });
    assert!(panel.harvest_snapshot());
    assert!(panel.enumeration_failed());
    assert!(panel.file_rows().is_empty());
}
