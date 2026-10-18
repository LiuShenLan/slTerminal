//! M6.3 主题编辑关键路径(L4 虚拟窗口):草稿/基线双快照、首次保存恒
//! fork、保存与应用分离、WCAG 保存拒绝、取色推荐点选、主题解析热应用。

use super::*;
use crate::app_shell::settings_pane::SettingsPane;
use crate::app_shell::settings_pane::ThemeColorSlot;

// ── M6.3 主题编辑关键路径 ──

/// 在临时数据目录打开设置页并进入主题编辑器。
fn open_theme_editor_at(
    cx: &mut TestAppContext,
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    path: std::path::PathBuf,
) -> gpui::Entity<SettingsPane> {
    let pane = open_settings_at(cx, workspace, window, path);
    click_selector(window, "settings-nav-appearance");
    draw_frame(window);
    click_selector(window, "settings-theme-edit");
    draw_frame(window);
    assert!(
        window.debug_bounds("settings-theme-name").is_some(),
        "主题编辑器必须渲染"
    );
    pane
}

/// 编辑器的草稿/基线双快照 + 首次保存恒 fork + 保存不应用。
#[gpui::test]
fn theme_editor_keeps_draft_and_baseline_and_first_save_forks(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let themes = directory.path().join("themes");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path.clone());

    let (draft, baseline, source_id, templates) = cx.update(|cx| {
        let pane = pane.read(cx);
        (
            pane.theme_editor_draft_for_test().unwrap().clone(),
            pane.theme_editor_baseline_for_test().unwrap().clone(),
            pane.theme_editor_source_id_for_test().unwrap(),
            pane.theme_editor_template_count_for_test(),
        )
    });
    assert_eq!(draft, baseline, "打开即草稿 = 基线");
    assert!(source_id.is_none(), "内置模板无源库文档");
    assert_eq!(
        templates,
        slterm_settings::ThemeName::BUILTIN.len(),
        "模板列表 = 内置目录"
    );

    let before_palette = cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .palette
            .clone()
    });
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_color_for_test(
                ThemeColorSlot::Foreground,
                [0xf0, 0xf0, 0xf0],
                window,
                cx,
            );
        });
    });
    window.run_until_parked();
    cx.update(|cx| {
        let pane = pane.read(cx);
        let draft = pane.theme_editor_draft_for_test().unwrap();
        assert_eq!(draft.terminal.foreground, [0xf0, 0xf0, 0xf0]);
        assert_eq!(
            pane.theme_editor_baseline_for_test()
                .unwrap()
                .terminal
                .foreground,
            baseline.terminal.foreground,
            "改草稿不得动基线"
        );
        assert_eq!(
            cx.global::<crate::app_shell::config::Settings>()
                .palette
                .foreground,
            before_palette.foreground,
            "编辑期间不得热应用运行时"
        );
    });

    let saved = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(false, window, cx)
        })
    });
    assert!(
        saved,
        "保存必须成功: {:?}",
        cx.update(|cx| pane
            .read(cx)
            .theme_editor_error_for_test()
            .map(str::to_owned))
    );
    assert!(!settings_path.exists(), "保存只落主题库,应用键必须未动");
    let mut files: Vec<_> = std::fs::read_dir(&themes)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".slterm-theme.json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 1, "首次保存必须 fork 出新库文件: {files:?}");
    let (first_id, first_revision) = cx.update(|cx| {
        let pane = pane.read(cx);
        (
            pane.theme_editor_saved_id_for_test().unwrap().unwrap(),
            pane.theme_editor_saved_revision_for_test().unwrap(),
        )
    });
    assert_eq!(first_revision, 1);
    cx.update(|cx| {
        assert_eq!(
            cx.global::<crate::app_shell::config::Settings>()
                .palette
                .foreground,
            before_palette.foreground,
            "保存不等于应用"
        );
    });

    // 第二次保存续用同一 id,修订递增,不产生第二个 fork。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_color_for_test(
                ThemeColorSlot::Foreground,
                [0xe4, 0xe5, 0xe6],
                window,
                cx,
            );
        });
    });
    let saved = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(false, window, cx)
        })
    });
    assert!(saved);
    let (second_id, second_revision) = cx.update(|cx| {
        let pane = pane.read(cx);
        (
            pane.theme_editor_saved_id_for_test().unwrap().unwrap(),
            pane.theme_editor_saved_revision_for_test().unwrap(),
        )
    });
    assert_eq!(second_id, first_id, "后续保存必须更新同一主题");
    assert_eq!(second_revision, 2);
    assert_eq!(
        std::fs::read_dir(&themes)
            .unwrap()
            .filter(|entry| entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".slterm-theme.json"))
            .count(),
        1,
        "第二次保存不得二次 fork"
    );
    assert!(!settings_path.exists(), "二次保存仍不应用");
}

/// 模板切换只替换草稿;源主题与主题库文件不动。
#[gpui::test]
fn theme_editor_template_switch_replaces_only_the_draft(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path);
    // 先保存一次,让库里有源文档与修订。
    let saved = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(false, window, cx)
        })
    });
    assert!(saved);
    let (saved_id, saved_revision, file_before) = cx.update(|cx| {
        let pane = pane.read(cx);
        (
            pane.theme_editor_saved_id_for_test().unwrap().unwrap(),
            pane.theme_editor_saved_revision_for_test().unwrap(),
            {
                let themes = directory.path().join("themes");
                std::fs::read_dir(&themes)
                    .unwrap()
                    .filter(|entry| {
                        entry
                            .as_ref()
                            .unwrap()
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".slterm-theme.json")
                    })
                    .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
                    .collect::<Vec<_>>()
            },
        )
    });
    let name_before = cx.update(|cx| {
        pane.read(cx)
            .theme_editor_draft_for_test()
            .unwrap()
            .name
            .clone()
    });

    // 切到第二个内置模板(Nord;目录首位 Linear)。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.switch_theme_template_for_test(3, window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        let draft = pane.theme_editor_draft_for_test().unwrap();
        assert_eq!(draft.base, slterm_settings::ThemeName::Nord);
        assert_eq!(draft.name, name_before, "换模板不改用户命名");
        assert_eq!(
            pane.theme_editor_source_id_for_test().unwrap(),
            None,
            "内置模板无源文档"
        );
        assert_eq!(
            pane.theme_editor_saved_revision_for_test(),
            None,
            "换模板后保存路径重新从 fork 开始"
        );
        assert_eq!(
            pane.theme_editor_saved_id_for_test().unwrap(),
            None,
            "源主题未被替换"
        );
    });
    let themes = directory.path().join("themes");
    let file_after: Vec<_> = std::fs::read_dir(&themes)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".slterm-theme.json")
        })
        .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
        .collect();
    assert_eq!(file_after, file_before, "模板切换不得改主题库文件");
    assert_eq!(saved_id, first_saved_id(&file_before));
    assert_eq!(saved_revision, 1);
}

/// 从库文件名反推 id(测试用;文件名即 `<id>.slterm-theme.json`)。
fn first_saved_id(files: &[Vec<u8>]) -> String {
    assert_eq!(files.len(), 1);
    let value: serde_json::Value = serde_json::from_slice(&files[0]).unwrap();
    value["id"].as_str().unwrap().to_owned()
}

/// 保存与应用分离:应用写入应用键、热应用全局调色板并关闭编辑器。
#[gpui::test]
fn theme_editor_apply_is_a_separate_action_from_save(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path.clone());
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_color_for_test(
                ThemeColorSlot::Foreground,
                [0xf2, 0xf4, 0xf6],
                window,
                cx,
            );
        });
    });
    let applied = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(true, window, cx)
        })
    });
    assert!(
        applied,
        "应用必须成功: {:?}",
        cx.update(|cx| pane
            .read(cx)
            .theme_editor_error_for_test()
            .map(str::to_owned))
    );
    let tree = slterm_settings::json::parse(&std::fs::read(&settings_path).unwrap()).unwrap();
    let appearance = tree.get("appearance").unwrap();
    let id = appearance
        .get("custom_theme")
        .and_then(|value| value.as_str())
        .expect("应用必须写 custom_theme")
        .to_owned();
    assert_eq!(
        appearance.get("theme").and_then(|value| value.as_str()),
        Some("Linear")
    );
    cx.update(|cx| {
        assert_eq!(
            cx.global::<crate::app_shell::config::Settings>()
                .palette
                .foreground,
            crate::gpui_terminal::colors::from_rgb8([0xf2, 0xf4, 0xf6]),
            "应用必须热更新全局调色板"
        );
        assert!(
            !pane.read(cx).theme_editor_open_for_test(),
            "应用后关闭编辑器"
        );
        assert_eq!(
            pane.read(cx).runtime_for_test().custom_theme.as_deref(),
            Some(id.as_str())
        );
    });
}

/// WCAG 保存路径:前景=背景的不达标组合拒绝落盘,库文件不新增。
#[gpui::test]
fn theme_editor_save_rejects_low_contrast_combinations(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path.clone());
    let background = cx.update(|cx| {
        pane.read(cx)
            .theme_editor_draft_for_test()
            .unwrap()
            .terminal
            .background
    });
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_color_for_test(ThemeColorSlot::Foreground, background, window, cx);
        });
    });
    let saved = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(false, window, cx)
        })
    });
    assert!(!saved, "不达标组合必须拒绝保存");
    cx.update(|cx| {
        let error = pane
            .read(cx)
            .theme_editor_error_for_test()
            .expect("必须留下可感知错误");
        assert!(error.contains("contrast"), "{error}");
    });
    assert!(!settings_path.exists());
    assert!(
        !directory.path().join("themes").exists()
            || std::fs::read_dir(directory.path().join("themes"))
                .unwrap()
                .next()
                .is_none(),
        "拒绝路径不得写主题库文件"
    );
}

/// 取色推荐:L4 点选推荐色块 → 草稿前景更新;原色恒第一且候选达标。
#[gpui::test]
fn theme_editor_color_recommendations_keep_original_and_apply_on_click(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path);
    let (recommendations, background) = cx.update(|cx| {
        let pane = pane.read(cx);
        (
            pane.theme_editor_recommendations_for_test().unwrap(),
            pane.theme_editor_draft_for_test()
                .unwrap()
                .terminal
                .background,
        )
    });
    assert_eq!(
        recommendations[0],
        cx.update(|cx| pane
            .read(cx)
            .theme_editor_draft_for_test()
            .unwrap()
            .terminal
            .foreground),
        "原色永远第一"
    );
    for candidate in &recommendations[1..] {
        assert!(
            slterm_settings::meets_wcag_aa(*candidate, background),
            "推荐候选必须满足 WCAG AA: {candidate:?}"
        );
    }
    let target = recommendations[1];
    click_selector(&mut window, "settings-theme-recommendation-1");
    cx.update(|cx| {
        assert_eq!(
            pane.read(cx)
                .theme_editor_draft_for_test()
                .unwrap()
                .terminal
                .foreground,
            target,
            "点选推荐色块必须写入草稿"
        );
    });
}

/// 设置页主题下拉接真实解析:选择内置主题后全局调色板立即更新。
#[gpui::test]
fn settings_theme_select_resolves_the_builtin_palette(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, path.clone());
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.commit_select_for_test("theme", "Nord", window, cx);
        });
    });
    window.run_until_parked();
    cx.update(|cx| {
        let nord = slterm_settings::ThemeName::Nord.term_theme();
        assert_eq!(
            cx.global::<crate::app_shell::config::Settings>()
                .palette
                .background,
            crate::gpui_terminal::colors::from_rgb8(nord.background),
            "主题选择必须解析终端调色板"
        );
    });
    let tree = slterm_settings::json::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        tree.get("appearance")
            .and_then(|section| section.get("theme"))
            .and_then(|value| value.as_str()),
        Some("Nord")
    );
}
