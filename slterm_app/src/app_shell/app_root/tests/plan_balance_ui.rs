//! 设置页套餐余量：凭据保存/删除、手动刷新与只读结果展示。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::Modifiers;

use super::*;
use crate::plan_balance::client::PlanBalanceError;
use crate::plan_balance::{
    AmountInfo, PlanBalanceInfo, PlanBalanceSource, WindowInfo, WindowsInfo,
};
use crate::secrets::{CredentialStore, SecretBytes};

#[derive(Default)]
struct MemoryCredentialStore {
    secrets: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn load(&self, target: &str) -> std::io::Result<Option<SecretBytes>> {
        Ok(self
            .secrets
            .lock()
            .unwrap()
            .get(target)
            .cloned()
            .map(SecretBytes::new))
    }

    fn store(&self, target: &str, secret: &[u8]) -> std::io::Result<()> {
        self.secrets
            .lock()
            .unwrap()
            .insert(target.to_owned(), secret.to_vec());
        Ok(())
    }

    fn delete(&self, target: &str) -> std::io::Result<()> {
        self.secrets.lock().unwrap().remove(target);
        Ok(())
    }
}

fn wait_until(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    mut predicate: impl FnMut(&AppRoot) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        window.run_until_parked();
        cx.run_until_parked();
        if root.read_with(cx, |root, _| predicate(root)) {
            return;
        }
        assert!(Instant::now() < deadline, "等待套餐余量状态超时");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn open_plan_balance(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
    super::settings_pages::focus_workspace(root, window);
    super::settings_pages::open_settings(window);
    draw_frame(window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(window);
    let open = window.debug_bounds("settings-open-plan-balance").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(window);
}

/// 应用页提供套餐凭据入口，列出两来源并可关闭。
#[gpui::test]
fn settings_application_page_offers_plan_balance_dialog(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    open_plan_balance(&root, &mut window);

    assert!(
        window.debug_bounds("settings-plan-balance").is_some(),
        "应用页必须显示套餐余量只读汇总"
    );
    assert!(window.debug_bounds("plan-balance-dialog").is_some());
    assert!(window.debug_bounds("plan-balance-source-0").is_some());
    assert!(window.debug_bounds("plan-balance-source-1").is_some());
    assert!(window.debug_bounds("plan-balance-save-token").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(window.debug_bounds("plan-balance-dialog").is_none());
}

/// Token 写入只经凭据域；保存后可删除，状态随之更新。
#[gpui::test]
fn plan_balance_token_save_and_delete_round_trip(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryCredentialStore::default());
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.set_plan_balance_store_for_test(store.clone())
    });
    root.update(cx, |root, cx| {
        root.restart_plan_balance_runtime_for_test(cx)
    });
    open_plan_balance(&root, &mut window);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.set_plan_balance_token_for_test("sk-test-plan-token", window, cx);
        });
    });
    let save = window.debug_bounds("plan-balance-save-token").unwrap();
    window.simulate_click(save.center(), Modifiers::default());
    wait_until(&root, &mut window, cx, |root| {
        root.plan_balance_configured(PlanBalanceSource::DeepSeek)
    });
    let target = PlanBalanceSource::DeepSeek.credential_target(std::path::Path::new(""));
    assert_eq!(
        store.load(&target).unwrap().unwrap().as_bytes(),
        b"sk-test-plan-token",
        "Token 必须写入专用凭据目标"
    );

    let delete = window.debug_bounds("plan-balance-delete-token").unwrap();
    window.simulate_click(delete.center(), Modifiers::default());
    wait_until(&root, &mut window, cx, |root| {
        !root.plan_balance_configured(PlanBalanceSource::DeepSeek)
    });
    assert!(store.load(&target).unwrap().is_none());
}

/// 手动刷新只查询已配置来源，并把结果写入同一状态机。
#[gpui::test]
fn plan_balance_manual_refresh_fetches_configured_sources(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryCredentialStore::default());
    let target = PlanBalanceSource::DeepSeek.credential_target(std::path::Path::new(""));
    store.store(&target, b"sk-test-plan-token").unwrap();
    let fetched = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&fetched);
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.set_plan_balance_store_for_test(store);
        root.set_plan_balance_fetch_for_test(Arc::new(move |source| {
            observed.lock().unwrap().push(source);
            Ok(PlanBalanceInfo {
                frozen: false,
                amount: Some(AmountInfo {
                    value: "12.50".to_owned(),
                    currency: "CNY".to_owned(),
                }),
                windows: Some(WindowsInfo {
                    five_hour: WindowInfo {
                        used_percent: 42,
                        resets_at: None,
                    },
                    seven_day: WindowInfo {
                        used_percent: 7,
                        resets_at: None,
                    },
                }),
            })
        }));
    });
    root.update(cx, |root, cx| {
        root.restart_plan_balance_runtime_for_test(cx)
    });
    wait_until(&root, &mut window, cx, |root| {
        root.plan_balance_configured(PlanBalanceSource::DeepSeek)
    });
    open_plan_balance(&root, &mut window);

    let refresh = window.debug_bounds("plan-balance-refresh").unwrap();
    window.simulate_click(refresh.center(), Modifiers::default());
    wait_until(&root, &mut window, cx, |root| {
        root.plan_balance_view(PlanBalanceSource::DeepSeek)
            .is_some_and(|view| view.result.is_some())
    });
    assert_eq!(
        fetched.lock().unwrap().as_slice(),
        [PlanBalanceSource::DeepSeek],
        "未配置来源不得发起查询"
    );
    let view = root
        .read_with(cx, |root, _| {
            root.plan_balance_view(PlanBalanceSource::DeepSeek)
        })
        .unwrap();
    assert_eq!(view.result.unwrap().amount.unwrap().value, "12.50");
    assert!(view.error.is_none());
}

/// 空 Token 不写凭据域并显示封闭错误。
#[gpui::test]
fn plan_balance_rejects_empty_token(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryCredentialStore::default());
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.set_plan_balance_store_for_test(store.clone())
    });
    root.update(cx, |root, cx| {
        root.restart_plan_balance_runtime_for_test(cx)
    });
    open_plan_balance(&root, &mut window);
    let save = window.debug_bounds("plan-balance-save-token").unwrap();
    window.simulate_click(save.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("plan-balance-error").is_some());
    assert!(store.secrets.lock().unwrap().is_empty());
}

/// 查询失败只映射封闭文案，状态机保留错误分类。
#[gpui::test]
fn plan_balance_fetch_failure_is_closed(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryCredentialStore::default());
    let target = PlanBalanceSource::Kimi.credential_target(std::path::Path::new(""));
    store.store(&target, b"sk-test-plan-token").unwrap();
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.set_plan_balance_store_for_test(store);
        root.set_plan_balance_fetch_for_test(Arc::new(|_| Err(PlanBalanceError::Parse)));
    });
    root.update(cx, |root, cx| {
        root.restart_plan_balance_runtime_for_test(cx)
    });
    wait_until(&root, &mut window, cx, |root| {
        root.plan_balance_configured(PlanBalanceSource::Kimi)
    });
    open_plan_balance(&root, &mut window);
    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.select_plan_balance_source(PlanBalanceSource::Kimi, cx);
        });
    });
    let refresh = window.debug_bounds("plan-balance-refresh").unwrap();
    window.simulate_click(refresh.center(), Modifiers::default());
    wait_until(&root, &mut window, cx, |root| {
        root.plan_balance_view(PlanBalanceSource::Kimi)
            .is_some_and(|view| view.error.is_some())
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("plan-balance-status-1").is_some());
}

/// 自动调度只依赖注册表节拍：导航从未打开时仍按到期查询，窗口销毁后停止。
#[gpui::test]
fn plan_balance_auto_tick_runs_without_navigation_and_stops_on_exit(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryCredentialStore::default());
    let target = PlanBalanceSource::DeepSeek.credential_target(std::path::Path::new(""));
    store.store(&target, b"sk-test-plan-token").unwrap();
    let fetched = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&fetched);
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.set_plan_balance_store_for_test(store);
        root.set_plan_balance_fetch_for_test(Arc::new(move |source| {
            observed.lock().unwrap().push(source);
            Ok(PlanBalanceInfo {
                frozen: false,
                amount: None,
                windows: None,
            })
        }));
    });
    root.update(cx, |root, cx| {
        root.restart_plan_balance_runtime_for_test(cx)
    });
    wait_until(&root, &mut window, cx, |root| {
        root.plan_balance_configured(PlanBalanceSource::DeepSeek)
    });
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "前置：导航与设置都不应打开"
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        window
            .executor()
            .advance_clock(Duration::from_millis(1_100));
        window.run_until_parked();
        cx.run_until_parked();
        if root.read_with(cx, |root, _| {
            root.plan_balance_view(PlanBalanceSource::DeepSeek)
                .is_some_and(|view| view.result.is_some())
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "自动调度未在节拍内查询套餐");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        fetched.lock().unwrap().as_slice(),
        [PlanBalanceSource::DeepSeek],
        "导航隐藏时自动调度仍必须查询已配置来源"
    );

    drop(root);
    drop(window);
    cx.run_until_parked();
    let seen = fetched.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(1_300));
    cx.run_until_parked();
    assert_eq!(
        fetched.lock().unwrap().len(),
        seen,
        "窗口销毁后自动调度必须停止"
    );
}
