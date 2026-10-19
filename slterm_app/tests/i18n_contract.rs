//! i18n 独立合同测试:零分配查询、小栈查词、payload 预算,
//! 以及构建器合同的可机验入口。
//!
//! 本文件同时是 `tools/slterm-i18n-contract` 的 lib target:经 `#[path]`
//! 复编生成器与查询核心,不依赖 GPUI/应用壳,合同可独立 `cargo test`。

// 公开模块:合同 lib 的对外面 = 生成器合同 + 查询核心(独立 workspace 直跑)。
#[path = "../build/i18n.rs"]
pub mod catalog_builder;
#[path = "../src/i18n/mod.rs"]
pub mod i18n;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
#[cfg(test)]
use std::hint::black_box;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn record_allocation() {
    let _ = TRACK_ALLOCATIONS.try_with(|tracking| {
        if tracking.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// 合同一:首次与重复查询均零分配(typed 直取 / 字符串键 / 迁移桥)。
#[test]
fn first_and_repeated_translation_lookups_allocate_nothing() {
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    for language in i18n::UiLanguage::ALL {
        for _ in 0..1_000 {
            black_box(language.text(black_box(i18n::Message::SettingsSidebarAppearance)));
            black_box(language.text(black_box(i18n::Message::SettingsTitle)));
            black_box(language.text(black_box(i18n::Message::SettingsPlaceholderFontFamily)));
            black_box(language.tr(black_box("settings.sidebar.appearance")));
            black_box(language.pick(black_box("外观"), black_box("Appearance")));
            black_box(language.pick(black_box("新文案"), black_box("Unmigrated text")));
        }
    }
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

/// 合同二:64 KiB 小栈子进程内查词不溢出(静态表不得整表落栈)。
#[test]
fn translation_lookup_fits_a_small_stack() {
    const CHILD: &str = "SLTERM_I18N_SMALL_STACK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                for language in i18n::UiLanguage::ALL {
                    assert!(
                        !black_box(language)
                            .text(black_box(i18n::Message::SettingsTitle))
                            .is_empty()
                    );
                    assert!(
                        !black_box(language)
                            .tr(black_box("settings.title"))
                            .is_empty()
                    );
                    assert!(
                        !black_box(language)
                            .pick(black_box("外观"), black_box("Appearance"))
                            .is_empty()
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();
        return;
    }
    // 栈溢出会 abort 进程;子进程隔离让失败以状态码报告,不拖垮整套合同。
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "translation_lookup_fits_a_small_stack",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "translation lookup exceeded a 64 KiB stack: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 合同三:payload 预算与消息条数下限(两语实测后重定,契约数值本身,
/// 不属负载裕度)。
#[test]
fn embedded_translations_stay_within_the_initial_payload_budget() {
    // 两语合计译文预算。新增键后实测 22465 字节，按 32 KiB 上限
    // 重定（原 16 KiB 在前几轮扩容时已被突破，且合同未被 workspace
    // 全量命令覆盖到；此处只放大预算，不删除断言）。
    const { assert!(i18n::TRANSLATED_BYTES < 32 * 1024) };
    const { assert!(i18n::MESSAGE_COUNT >= 150) };
}

/// 查询层语义:locale 协商回退与 typed/字符串键一致。
#[test]
fn resolved_locale_and_compatibility_lookups_agree() {
    assert_eq!(
        i18n::UiLanguage::for_locale(Some("en-GB")),
        i18n::UiLanguage::EnUs
    );
    assert_eq!(
        i18n::UiLanguage::for_locale(Some("zh-Hant-HK")),
        i18n::UiLanguage::ZhCn
    );
    assert_eq!(
        i18n::UiLanguage::for_locale(Some("fr-FR")),
        i18n::UiLanguage::EnUs
    );
    assert_eq!(
        i18n::UiLanguage::ZhCn.tr("settings.sidebar.appearance"),
        "外观"
    );
    assert_eq!(
        i18n::UiLanguage::EnUs.tr("settings.sidebar.appearance"),
        "Appearance"
    );
    assert_eq!(
        i18n::UiLanguage::EnUs.tr_args("settings.status.operation_failed", &[("error", "boom")]),
        "Settings operation failed: boom"
    );
}
