//! M6.3 主题编辑关键路径(L4 虚拟窗口):草稿/基线双快照、首次保存恒
//! fork、保存与应用分离、WCAG 保存拒绝、取色推荐点选、主题解析热应用。

use super::*;
use crate::app_shell::settings_pane::SettingsPane;
use crate::app_shell::settings_pane::ThemeColorSlot;
use crate::theme_library::ThemeLibraryStore;
use crate::theme_library::package::{Author, CheckedPackage, export_package};
use slterm_settings::BlurMode;

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
    // A2:条件写盘在后台执行,排水后文件与热应用才可见。
    window.run_until_parked();
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

// ── 高级字段:字距/字重/布局/模糊 ──

/// 读取主题库唯一文件并解析为文档(保存后 round-trip 断言)。
fn read_single_theme_document(directory: &std::path::Path) -> crate::theme_library::ThemeDocument {
    let themes = directory.join("themes");
    let mut paths = std::fs::read_dir(&themes)
        .expect("保存后必须存在主题库目录")
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".slterm-theme.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 1, "主题库必须恰有一个文件: {paths:?}");
    let bytes = std::fs::read(&paths[0]).unwrap();
    crate::theme_library::ThemeDocument::from_json_bytes(&bytes).unwrap()
}

/// 高级排版/布局/模糊字段:Save 后主题文档 round-trip 全量保留。
#[gpui::test]
fn theme_editor_saves_advanced_typography_layout_and_blur(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path.clone());
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            for (field, value) in [
                ("letter_spacing", "2.5"),
                ("font_weight", "600"),
                ("layout_radius", "18"),
                ("layout_gutter", "10"),
                ("layout_divider", "2"),
                ("padding_x", "12"),
                ("padding_y", "6"),
            ] {
                pane.set_theme_editor_input_for_test(field, value, window, cx);
            }
            pane.set_theme_editor_blur_for_test(Some(4), window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        let draft = pane.theme_editor_draft_for_test().unwrap();
        assert_eq!(draft.typography.letter_spacing, Some(2.5));
        assert_eq!(draft.typography.font_weight, Some(600));
        assert_eq!(draft.layout.card.radius, 18.0);
        assert_eq!(draft.layout.card.gutter, 10.0);
        assert_eq!(draft.layout.card.divider, 2.0);
        assert_eq!(draft.layout.padding_x, Some(12.0));
        assert_eq!(draft.layout.padding_y, Some(6.0));
        assert_eq!(draft.effects.blur, Some(BlurMode::Acrylic));
        assert_eq!(pane.theme_editor_blur_index_for_test(cx), Some(4));
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
    let document = read_single_theme_document(directory.path());
    let definition = document.definition().unwrap();
    assert_eq!(definition.typography.letter_spacing, Some(2.5));
    assert_eq!(definition.typography.font_weight, Some(600));
    assert_eq!(definition.layout.card.radius, 18.0);
    assert_eq!(definition.layout.card.gutter, 10.0);
    assert_eq!(definition.layout.card.divider, 2.0);
    assert_eq!(definition.layout.padding_x, Some(12.0));
    assert_eq!(definition.layout.padding_y, Some(6.0));
    assert_eq!(definition.effects.blur, Some(BlurMode::Acrylic));
    assert_eq!(
        document.to_value()["typography"]["enabled"].as_bool(),
        Some(true),
        "非空排版字段必须自动开启 typography.enabled"
    );
}

/// C7:非聚焦窗格压暗开关随主题文档 round-trip;Reset/模板切换遵循既有字段模式。
#[gpui::test]
fn theme_editor_pane_veil_switch_round_trips_through_the_library(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path);
    cx.update(|cx| {
        assert_eq!(
            pane.read(cx)
                .theme_editor_draft_for_test()
                .unwrap()
                .effects
                .veil,
            None,
            "内置模板未设置 veil(界面显示为开)"
        );
    });

    // 关闭开关 → 草稿显式 false。
    window.update(|_window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_editor_veil_for_test(false, cx);
        });
    });
    cx.update(|cx| {
        assert_eq!(
            pane.read(cx)
                .theme_editor_draft_for_test()
                .unwrap()
                .effects
                .veil,
            Some(false)
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
    let document = read_single_theme_document(directory.path());
    assert_eq!(
        document.definition().unwrap().effects.veil,
        Some(false),
        "关闭状态必须写入主题文档并可回读"
    );
    assert_eq!(
        document.to_value()["effects"]["veil"].as_bool(),
        Some(false)
    );

    // Reset 回到基线;模板切换回到新模板的「未设置」。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_editor_veil_for_test(true, cx);
            pane.reset_theme_editor_for_test(window, cx);
            assert_eq!(
                pane.theme_editor_draft_for_test().unwrap().effects.veil,
                Some(false),
                "Reset 必须回到已保存基线"
            );
            pane.switch_theme_template_for_test(3, window, cx);
            assert_eq!(
                pane.theme_editor_draft_for_test().unwrap().effects.veil,
                None,
                "模板切换后回到模板快照的未设置状态"
            );
        });
    });
}

/// Apply:已有运行时键即时生效;无运行时键的字段只留在主题文档。
#[gpui::test]
fn theme_editor_apply_publishes_runtime_keys_and_keeps_document_only_fields(
    cx: &mut TestAppContext,
) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path.clone());
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            for (field, value) in [
                ("letter_spacing", "1.5"),
                ("font_weight", "500"),
                ("layout_radius", "18"),
                ("layout_gutter", "10"),
                ("layout_divider", "2"),
                ("padding_x", "12"),
                ("padding_y", "6"),
            ] {
                pane.set_theme_editor_input_for_test(field, value, window, cx);
            }
            pane.set_theme_editor_blur_for_test(Some(4), window, cx);
        });
    });
    let applied = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(true, window, cx)
        })
    });
    // A2:条件写盘在后台执行,排水后文件与热应用才可见。
    window.run_until_parked();
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
    assert_eq!(
        appearance.get("blur").and_then(|value| value.as_str()),
        Some("acrylic"),
        "blur 已有运行时键,必须映射"
    );
    assert_eq!(
        appearance
            .get("pane_card_radius")
            .and_then(|value| value.as_f64()),
        Some(18.0)
    );
    assert_eq!(
        appearance
            .get("pane_card_gutter")
            .and_then(|value| value.as_f64()),
        Some(10.0)
    );
    assert_eq!(
        appearance
            .get("pane_card_divider")
            .and_then(|value| value.as_f64()),
        Some(2.0)
    );
    for key in ["letter_spacing", "font_weight", "padding_x", "padding_y"] {
        assert!(
            appearance.get(key).is_none(),
            "无运行时键的字段不得伪造设置键: {key}"
        );
    }
    cx.update(|cx| {
        let pane = pane.read(cx);
        let runtime = pane.runtime_for_test();
        assert_eq!(runtime.blur, BlurMode::Acrylic);
        assert_eq!(runtime.pane_card_radius, Some(18.0));
        assert_eq!(runtime.pane_card_gutter, Some(10.0));
        assert_eq!(runtime.pane_card_divider, Some(2.0));
        assert!(!pane.theme_editor_open_for_test(), "应用后关闭编辑器");
    });
    // 无运行时键的字段仍必须在已保存主题文档中可读。
    let document = read_single_theme_document(directory.path());
    let definition = document.definition().unwrap();
    assert_eq!(definition.typography.letter_spacing, Some(1.5));
    assert_eq!(definition.typography.font_weight, Some(500));
    assert_eq!(definition.layout.padding_x, Some(12.0));
    assert_eq!(definition.layout.padding_y, Some(6.0));
}

/// 高级字段随 Reset/模板切换回填,不与草稿状态脱节。
#[gpui::test]
fn theme_editor_advanced_fields_follow_reset_and_template_switch(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path);
    // 先以一组高级值保存,让基线带上它们。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_editor_input_for_test("letter_spacing", "2.5", window, cx);
            pane.set_theme_editor_input_for_test("font_weight", "600", window, cx);
            pane.set_theme_editor_input_for_test("padding_x", "12", window, cx);
            pane.set_theme_editor_blur_for_test(Some(4), window, cx);
        });
    });
    let saved = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(false, window, cx)
        })
    });
    assert!(saved);
    // 改草稿后 Reset:输入框与模糊选择必须回到基线。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_editor_input_for_test("letter_spacing", "5", window, cx);
            pane.set_theme_editor_blur_for_test(Some(1), window, cx);
        });
    });
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.reset_theme_editor_for_test(window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert_eq!(
            pane.theme_editor_input_value_for_test("letter_spacing", cx)
                .as_deref(),
            Some("2.50"),
            "Reset 后字距输入框回到基线"
        );
        assert_eq!(
            pane.theme_editor_input_value_for_test("font_weight", cx)
                .as_deref(),
            Some("600")
        );
        assert_eq!(
            pane.theme_editor_input_value_for_test("padding_x", cx)
                .as_deref(),
            Some("12.0")
        );
        assert_eq!(pane.theme_editor_blur_index_for_test(cx), Some(4));
    });
    // 换模板:高级字段随新模板快照重填,模糊回到未设置。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.switch_theme_template_for_test(3, window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert_eq!(
            pane.theme_editor_draft_for_test()
                .unwrap()
                .typography
                .letter_spacing,
            None
        );
        assert_eq!(
            pane.theme_editor_input_value_for_test("letter_spacing", cx)
                .as_deref(),
            Some("")
        );
        assert_eq!(
            pane.theme_editor_input_value_for_test("font_weight", cx)
                .as_deref(),
            Some("")
        );
        assert_eq!(pane.theme_editor_blur_index_for_test(cx), None);
        let nord = slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
        assert_eq!(
            pane.theme_editor_input_value_for_test("layout_radius", cx),
            Some(format!("{:.1}", nord.layout.card.radius)),
            "布局输入框随模板快照重填"
        );
    });
}

/// 非法字距/字重:显示错误、草稿保留最后一次有效值、保存被拒。
#[gpui::test]
fn theme_editor_rejects_invalid_letter_spacing_and_font_weight(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_theme_editor_at(cx, &workspace, &mut window, settings_path.clone());
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_editor_input_for_test("font_weight", "600", window, cx);
            pane.set_theme_editor_input_for_test("letter_spacing", "9", window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        let draft = pane.theme_editor_draft_for_test().unwrap();
        assert_eq!(
            draft.typography.letter_spacing, None,
            "越界字距不得进入草稿"
        );
        assert_eq!(
            draft.typography.font_weight,
            Some(600),
            "已有有效值不得被非法输入污染"
        );
        assert!(
            pane.theme_editor_error_for_test().is_some(),
            "非法输入必须显示错误"
        );
    });
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_theme_editor_input_for_test("font_weight", "50", window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert_eq!(
            pane.theme_editor_draft_for_test()
                .unwrap()
                .typography
                .font_weight,
            Some(600),
            "越界字重不得覆盖最后一次有效值"
        );
        assert!(pane.theme_editor_error_for_test().is_some());
    });
    let saved = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_theme_editor_for_test(false, window, cx)
        })
    });
    assert!(!saved, "存在无效输入时保存必须被拒");
    assert!(!settings_path.exists(), "拒绝路径不得写设置");
    assert!(
        !directory.path().join("themes").exists()
            || std::fs::read_dir(directory.path().join("themes"))
                .unwrap()
                .next()
                .is_none(),
        "拒绝路径不得写主题库文件"
    );
}

// ── B19 主题包导入/导出 UI ──

/// 在 tempdir 主题库写入一个自定义主题文档,返回其 id。
fn seed_custom_theme(directory: &std::path::Path, name: &str) -> String {
    let store = ThemeLibraryStore::new(directory.join("themes"));
    let mut definition =
        slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
    definition.name = name.to_owned();
    let document = crate::theme_library::from_definition(&definition).unwrap();
    store.import(&document).unwrap().id().unwrap().to_owned()
}

/// 用导出器生成一个合法主题包(导入用例的来源夹具)。
fn write_theme_package(
    directory: &std::path::Path,
    name: &str,
    file_name: &str,
) -> std::path::PathBuf {
    let mut definition =
        slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
    definition.name = name.to_owned();
    let document = crate::theme_library::from_definition(&definition).unwrap();
    let output = directory.join(file_name);
    export_package(
        &document,
        Author {
            name: crate::brand::NAME.to_owned(),
            github: None,
        },
        "1.0.0".into(),
        "MIT".into(),
        &output,
        None,
    )
    .unwrap();
    output
}

/// 导入合法包:库增项、状态提示成功,且导入主题进入编辑器模板下拉。
#[gpui::test]
fn theme_import_adds_a_selectable_library_entry(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());
    let package = write_theme_package(directory.path(), "Imported Aurora", "aurora.zip");

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.import_theme_package_for_test(window, cx)
        });
    });
    assert!(window.did_prompt_for_paths(), "导入必须打开文件选择器");
    window.simulate_path_prompt_response(|_| Some(vec![package]));
    window.run_until_parked();

    let store = ThemeLibraryStore::new(directory.path().join("themes"));
    let snapshot = store.list().unwrap();
    assert_eq!(snapshot.custom.len(), 1, "导入必须新增库条目");
    let imported = &snapshot.custom[0];
    assert_eq!(imported.name(), "Imported Aurora");
    let imported_id = imported.id().unwrap().to_owned();
    cx.update(|cx| {
        let pane = pane.read(cx);
        let status = pane.status_for_test().expect("导入成功必须给出提示");
        assert!(!status.is_error(), "导入成功不得标为错误: {status:?}");
        assert!(!pane.theme_import_busy_for_test(), "结束后忙态必须复位");
    });
    assert!(!settings_path.exists(), "导入不得改设置");

    // 主题编辑器模板下拉从 store 派生:导入项可选。
    click_selector(&mut window, "settings-nav-appearance");
    draw_frame(&mut window);
    click_selector(&mut window, "settings-theme-edit");
    draw_frame(&mut window);
    let templates = cx.update(|cx| pane.read(cx).theme_editor_template_count_for_test());
    assert_eq!(
        templates,
        slterm_settings::ThemeName::BUILTIN.len() + 1,
        "模板下拉必须包含导入主题"
    );
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.switch_theme_template_for_test(templates - 1, window, cx);
        });
    });
    cx.update(|cx| {
        assert_eq!(
            pane.read(cx).theme_editor_source_id_for_test(),
            Some(Some(imported_id)),
            "导入主题必须能在模板下拉中选中"
        );
    });
}

/// 非法包:状态条给可读错误,主题库不变(零部分写入)。
#[gpui::test]
fn invalid_theme_package_reports_an_error_without_touching_the_library(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());
    let broken = directory.path().join("broken.zip");
    std::fs::write(&broken, b"not a zip archive").unwrap();

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.import_theme_package_for_test(window, cx)
        });
    });
    window.simulate_path_prompt_response(|_| Some(vec![broken]));
    window.run_until_parked();

    cx.update(|cx| {
        let pane = pane.read(cx);
        let status = pane.status_for_test().expect("失败必须留下状态");
        assert!(status.is_error(), "非法包必须标为错误: {status:?}");
        assert!(!pane.theme_import_busy_for_test());
    });
    let store = ThemeLibraryStore::new(directory.path().join("themes"));
    assert!(
        store.list().unwrap().custom.is_empty(),
        "非法包不得产生库条目"
    );
    assert!(
        !directory.path().join("themes").exists()
            || std::fs::read_dir(directory.path().join("themes"))
                .unwrap()
                .next()
                .is_none(),
        "非法包不得留下部分写入"
    );
    assert!(!settings_path.exists(), "失败不得改设置");
}

/// 导出当前自定义主题:成品可自检,文档与库内主题一致。
#[gpui::test]
fn theme_export_round_trips_the_active_custom_theme(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let theme_id = seed_custom_theme(directory.path(), "Exported Theme");
    std::fs::write(
        &settings_path,
        format!(r#"{{"appearance":{{"custom_theme":"{theme_id}"}}}}"#),
    )
    .unwrap();
    let settings_before = std::fs::read(&settings_path).unwrap();
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());
    let output = directory.path().join("exported.zip");

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.export_theme_package_for_test(window, cx)
        });
    });
    window.simulate_new_path_selection(|_| Some(output.clone()));
    window.run_until_parked();

    assert!(output.exists(), "导出必须生成目标文件");
    let checked = CheckedPackage::open(&output).expect("导出成品必须能通过整包自检");
    let current = ThemeLibraryStore::new(directory.path().join("themes"))
        .load(&theme_id)
        .unwrap();
    assert_eq!(checked.manifest.name, current.name());
    assert_eq!(
        checked.document.definition().unwrap(),
        current.definition().unwrap(),
        "导出文档必须与当前自定义主题一致"
    );
    assert_eq!(checked.manifest.author.name, crate::brand::NAME);
    assert_eq!(checked.manifest.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(checked.manifest.license, "GPL-3.0");
    cx.update(|cx| {
        let pane = pane.read(cx);
        let status = pane.status_for_test().expect("导出成功必须给出提示");
        assert!(!status.is_error(), "{status:?}");
        assert!(!pane.theme_export_busy_for_test());
    });
    assert_eq!(
        std::fs::read(&settings_path).unwrap(),
        settings_before,
        "导出不得改设置"
    );
}

/// 两个对话框取消:零写入、忙态复位。
#[gpui::test]
fn cancelling_theme_package_dialogs_writes_nothing(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());

    // 导入取消。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.import_theme_package_for_test(window, cx)
        });
    });
    assert!(window.did_prompt_for_paths());
    window.simulate_path_prompt_response(|_| None);
    window.run_until_parked();
    cx.update(|cx| {
        assert!(!pane.read(cx).theme_import_busy_for_test());
        assert!(pane.read(cx).status_for_test().is_none());
    });
    assert!(!settings_path.exists(), "取消导入不得写设置");

    // 导出取消:关掉旧设置 tab 后以带自定义主题的设置文件重建面板。
    click_selector(&mut window, "settings-back");
    draw_frame(&mut window);
    let theme_id = seed_custom_theme(directory.path(), "Cancel Export");
    std::fs::write(
        &settings_path,
        format!(r#"{{"appearance":{{"custom_theme":"{theme_id}"}}}}"#),
    )
    .unwrap();
    let settings_before = std::fs::read(&settings_path).unwrap();
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.export_theme_package_for_test(window, cx)
        });
    });
    window.simulate_new_path_selection(|_| None);
    window.run_until_parked();
    cx.update(|cx| {
        assert!(!pane.read(cx).theme_export_busy_for_test());
        assert!(pane.read(cx).status_for_test().is_none());
    });
    assert_eq!(
        std::fs::read(&settings_path).unwrap(),
        settings_before,
        "取消导出不得写设置"
    );
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".zip")),
        "取消导出不得产出包文件"
    );
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
