//! 全量验收锚：单一权威、崩溃恢复、排除面、平台降级与敏感材料。
//!
//! 只含测试：判据来自真实磁盘、封闭类型或源码单点，不读取私有运行态。

use std::path::Path;

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn production_source(relative: &str) -> String {
    let source = std::fs::read_to_string(crate_root().join(relative)).unwrap_or_default();
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod acceptance_tests {
    use std::str::FromStr as _;

    use super::*;
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::workspace::WorkspaceDatabase;
    use slterm_path::{AppPaths, CanonicalPath};
    use slterm_workspace::{ProjectId, ProjectRegistry, ProjectSnapshot};

    fn fixture() -> (tempfile::TempDir, AppPaths) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        (temp, paths)
    }

    fn run_marker(run_id: &str) -> RunMarker {
        RunMarker {
            run_id: run_id.to_owned(),
            process_id: std::process::id(),
            process_created_at_ms: 1,
        }
    }

    fn project_snapshot(root: &Path) -> ProjectSnapshot {
        let project_id = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let mut registry = ProjectRegistry::new();
        registry
            .add_project(project_id, CanonicalPath::new(root).unwrap())
            .unwrap();
        ProjectSnapshot::restore(registry, Vec::new()).unwrap()
    }

    /// 锚：所有者判据可观察、可复现且不依赖内部状态。
    ///
    /// 正常退出与崩溃的判据完全来自磁盘上的工作区运行事实。
    #[test]
    fn acceptance_user_contract_observable() {
        let (_temp, paths) = fixture();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .begin_run("uuid-crash", run_marker("run-crash"))
            .unwrap();
        // 崩溃：进程退出但未标记正常关闭。
        drop(store);

        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let previous = store.metadata().unwrap().expect("运行事实必须存在");
        assert!(!previous.clean_shutdown, "崩溃后磁盘必须显示非正常退出");
        store
            .begin_run("uuid-normal", run_marker("run-normal"))
            .unwrap();
        store
            .mark_clean_shutdown("run-normal")
            .expect("正常退出必须落盘");
        drop(store);

        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(
            store.metadata().unwrap().unwrap().clean_shutdown,
            "正常退出后磁盘必须显示 clean_shutdown"
        );
    }

    /// 锚：崩溃恢复——未正常退出的运行再次打开后恢复同一快照事实。
    #[test]
    fn integration_crash_recovery() {
        let (temp, paths) = fixture();
        let snapshot = project_snapshot(temp.path());
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .begin_run("uuid-crash", run_marker("run-crash"))
            .unwrap();
        store.save_project_snapshot(&snapshot).unwrap();
        // 无正常退出标记即模拟崩溃。
        drop(store);

        let mut recovered = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            recovered.load_project_snapshot().unwrap(),
            snapshot,
            "崩溃后必须恢复同一项目快照"
        );
        assert!(!recovered.metadata().unwrap().unwrap().clean_shutdown);
        // 恢复后新运行可继续写入并正常退出。
        recovered
            .begin_run("uuid-next", run_marker("run-next"))
            .unwrap();
        recovered.save_project_snapshot(&snapshot).unwrap();
        recovered.mark_clean_shutdown("run-next").unwrap();
        drop(recovered);

        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(store.metadata().unwrap().unwrap().clean_shutdown);
        assert_eq!(store.load_project_snapshot().unwrap(), snapshot);
    }

    /// 锚：跨域集成不存在第二缓冲、授权或关闭通道（源码单点）。
    #[test]
    fn integration_single_authority_channels() {
        let git_changes = production_source("src/app_shell/app_root/git_changes.rs");
        assert!(git_changes.contains("pub(crate) fn rollback_unstaged"));
        assert!(git_changes.contains("pub(crate) fn delete_change"));

        // Commit 面板与 Runtime 写都只调用共享执行体，不自建 git 子进程。
        for relative in [
            "src/app_shell/app_root/commit.rs",
            "src/local_control/git.rs",
        ] {
            let source = production_source(relative);
            assert!(
                !source.contains("std::process::Command"),
                "{relative} 不得另建 git 执行体"
            );
            assert!(
                source.contains("git_changes"),
                "{relative} 必须经 git_changes 单一执行体"
            );
        }

        // Runtime 命令只有一个派发入口，事件入口消费同一派发类型。
        let snapshot = production_source("src/app_shell/app_root/runtime_snapshot.rs");
        assert!(snapshot.contains("fn execute_runtime_mutation"));
        let events = production_source("src/app_shell/app_root/events.rs");
        assert!(events.contains("RuntimeMutationDispatch"));
        assert!(!events.contains("fn execute_runtime_mutation"));

        // 关闭写只经工作区 writer client 与全局关闭请求通道。
        let writer = production_source("src/persistence/workspace_writer.rs");
        assert!(writer.contains("pub(crate) fn save_sidebar_state"));
        let close = production_source("src/app_shell/app_root/application_close.rs");
        assert!(close.contains("GlobalCloseRequestError"));
    }

    /// 锚：最终排除项没有隐藏入口或迁移占位。
    #[test]
    fn acceptance_excluded_surface_absent() {
        const FORBIDDEN: &[&str] = &[
            "legacy_session",
            "session_v1",
            "migrate_old",
            "old_schema",
            "SessionJson",
            "ImageTabView",
            "image_viewable",
            "openable_in_app",
            "DiffTabView",
            "FileTreeDrag",
            // 卡片几何已从设置与主题值模型删除，不得重新引入。
            "pane_card_",
        ];
        // 扫描全部应用源码的生产段，而不是固定入口文件清单。
        fn scan(directory: &Path, violations: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(directory) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan(&path, violations);
                    continue;
                }
                if path.extension().is_none_or(|extension| extension != "rs")
                    || path.file_name().is_some_and(|name| name == "acceptance.rs")
                {
                    continue;
                }
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                let production = source.split("#[cfg(test)]").next().unwrap_or_default();
                for forbidden in FORBIDDEN {
                    if production.contains(forbidden) {
                        violations.push(format!(
                            "{} 不得含旧通道或排除面 {forbidden}",
                            path.display()
                        ));
                    }
                }
            }
        }

        let mut violations = Vec::new();
        scan(&crate_root().join("src"), &mut violations);
        assert!(
            violations.is_empty(),
            "最终排除项仍有隐藏实现：\n{}",
            violations.join("\n")
        );
        assert_eq!(crate::brand::NAME, "slTerminal");
        assert!(!crate::brand::WINDOWS_APP_ID.is_empty());
    }

    /// 锚：平台或第三方能力缺失按边界降级或失败。
    #[test]
    fn integration_platform_boundary_failure() {
        use crate::platform::material::{BlurSupport, blur_support_for};
        use crate::platform::windows_integration::{FakeWindowsIntegration, start_hidden_with};
        use slterm_settings::BlurMode;

        // 旧系统材质不可用时明确不可选，而不是静默宣称生效。
        assert_eq!(
            blur_support_for(BlurMode::Mica, 0),
            BlurSupport::NoneFallback
        );
        // 托盘不可用时静默启动回退为可见窗口。
        let windows = FakeWindowsIntegration::default();
        windows.set_tray_available(false);
        assert!(!start_hidden_with(true, true, &windows));
        // 更新资产缺哈希时不进入下载。
        let asset = crate::update_check::UpdateAsset {
            version: "0.3.0".to_owned(),
            name: "slTerminal-v0.3.0-windows-x64-setup.exe".to_owned(),
            url: "https://github.com/LiuShenLan/slTerminal/releases/download/v0.3.0/slTerminal-v0.3.0-windows-x64-setup.exe".to_owned(),
            size: Some(42),
            digest: None,
        };
        assert!(crate::update_download::begin(&asset).is_err());
    }

    /// 锚：不可自动化面有豁免、兜底和人工记录。
    #[test]
    fn acceptance_exemptions_registered() {
        let path = crate_root().join("../.claude/docs/testing/exemptions.md");
        let text = std::fs::read_to_string(&path).expect("豁免表必须存在");
        assert!(
            text.contains("| 复审结论/登记锚点 |"),
            "豁免表必须声明结论列"
        );
        assert!(
            text.matches("| 保留 |").count() >= 10,
            "保留项必须有兜底登记"
        );
        assert!(
            text.contains("人工验收") || text.contains("人工核验"),
            "保留项必须有兜底或人工记录"
        );
    }

    /// 锚：性能数值必须带测量条件，或明确属于契约数值。
    ///
    /// 数值只有伴随契约语（上限／预算／超时／去抖等）或实测日期锚才算成立。
    #[test]
    fn performance_claims_have_conditions() {
        const MEASURE_UNITS: &[&str] = &[
            "KiB", "MiB", "GiB", "kB", "KB", "MB", "GB", "B/s", "ms", "µs", "fps", "毫秒", "秒",
        ];
        const CONDITION_WORDS: &[&str] = &[
            "上限",
            "预算",
            "超时",
            "不超过",
            "最多",
            "至少",
            "超过",
            "大于",
            "小于",
            "以内",
            "阈值",
            "下限",
            "实测",
            "按实测",
            "承诺",
            "合同",
            "不得",
            "最低",
            "最高",
            "约",
            "期限",
            "去抖",
            "稳定窗",
            "冷却",
            "可配置",
            "触发",
            "测试验证",
            "后保存",
        ];
        fn has_measurement(line: &str) -> bool {
            MEASURE_UNITS.iter().any(|unit| {
                line.match_indices(unit).any(|(index, _)| {
                    line[..index]
                        .chars()
                        .rev()
                        .find(|character| !character.is_whitespace())
                        .is_some_and(|character| character.is_ascii_digit())
                })
            })
        }
        fn contains_date_anchor(line: &str) -> bool {
            let bytes = line.as_bytes();
            bytes.windows(10).any(|window| {
                window[0] == b'2'
                    && window[1] == b'0'
                    && window[4] == b'-'
                    && window[7] == b'-'
                    && window[2..4].iter().all(u8::is_ascii_digit)
                    && window[5..7].iter().all(u8::is_ascii_digit)
                    && window[8..10].iter().all(u8::is_ascii_digit)
            })
        }
        fn scan_file(path: &Path, violations: &mut Vec<String>) {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let mut dated_section = false;
            for (index, line) in text.lines().enumerate() {
                dated_section |= contains_date_anchor(line);
                if has_measurement(line)
                    && !dated_section
                    && !CONDITION_WORDS.iter().any(|word| line.contains(word))
                {
                    violations.push(format!("{}:{} {}", path.display(), index + 1, line.trim()));
                }
            }
        }
        fn scan_tree(root: &Path, violations: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_tree(&path, violations);
                } else if path.extension().is_some_and(|extension| extension == "md") {
                    scan_file(&path, violations);
                }
            }
        }

        let repo = crate_root().join("..");
        let mut violations = Vec::new();
        for relative in [".claude/docs", "docs/devPlan", "release-notes"] {
            scan_tree(&repo.join(relative), &mut violations);
        }
        for relative in ["CONTEXT.md", "README.md"] {
            let path = repo.join(relative);
            if path.is_file() {
                scan_file(&path, &mut violations);
            }
        }
        assert!(
            violations.is_empty(),
            "性能数值缺少测量条件或契约语：\n{}",
            violations.join("\n")
        );
    }

    /// 锚：日志、错误、备份和诊断不含敏感正文或凭据。
    #[test]
    fn sensitive_material_excluded_from_outputs() {
        use crate::audit::{AuditEvent, AuditResult};

        let token = "sk-test-acceptance-secret";
        let mut report = crate::diagnostics::DiagnosticReport::new(
            crate::diagnostics::DiagnosticSnapshot::current(false, false),
        );
        report.push_event(&format!("runtime handshake token={token}"));
        assert!(!report.to_json().contains(token));

        let capture = crate::audit::test_support::capture_events();
        crate::audit::record(
            AuditEvent::new("cleanup", "acceptance", AuditResult::Ok)
                .with_path(Path::new("D:/work/file.log")),
        );
        let text = capture.text();
        assert!(text.contains("operation=\"cleanup\""));
        assert!(!text.contains(token), "审计事件不得携带凭据值");
        assert!(!text.contains("secret="), "审计事件不得插值任意内容");
    }
}
