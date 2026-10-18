//! 会话元数据单点索引(硬约束 #8 对应物,「会话元数据单点对应物」节)。
//!
//! 双层权威:Workspace 是 pane 实体的唯一属主(生命周期与回收全走它);
//! 本模块是模块级单例索引,供 workspace 之外的消费者(03 hook 域 /
//! 04 runtime)按 `PaneId` 查询会话元数据——反向不持有实体、不裁决生命周期。
//!
//! 纪律:写入点唯一 = workspace `new_pane`/关 pane 两处;面板只订阅、不自存。
//!
//! M3 字段最简(改造节 8):`shell_kind` 归 PTY 域回填,
//! `prompt_ready`/`agent` 归 AI 集成域回填——本体与订阅机制 M3 先立。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError, RwLock};

use slterm_split::PaneId;

/// 一份 pane 会话的元数据(M3 最简形态)。
///
/// 终态字段(`shell_kind: ShellKind` / `prompt_ready: bool` /
/// `agent: Option<AgentSessionRef>`)按 PTY 与 AI 集成域节奏回填;M3 先立索引形状
/// 与写入点纪律,空壳不代表终态。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneSessionMeta {}

/// 索引变更事件(订阅面)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryEvent {
    Registered(PaneId),
    Removed(PaneId),
}

/// 订阅句柄:Drop 即退订(对齐旧 TS 注册表家族通用契约)。
#[allow(dead_code)] // id 字段随 subscribe 消费端(M4+/runtime_api)点亮。
pub struct Subscription {
    id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut subscribers = lock_subscribers();
        subscribers.retain(|(id, _)| *id != self.id);
    }
}

type Subscriber = Box<dyn Fn(&RegistryEvent) + Send>;

static METAS: LazyLock<RwLock<HashMap<PaneId, PaneSessionMeta>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static SUBSCRIBERS: LazyLock<Mutex<Vec<(u64, Subscriber)>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
static NEXT_SUBSCRIPTION: AtomicU64 = AtomicU64::new(1);

fn lock_metas() -> std::sync::RwLockWriteGuard<'static, HashMap<PaneId, PaneSessionMeta>> {
    METAS.write().unwrap_or_else(PoisonError::into_inner)
}

fn lock_subscribers() -> MutexGuard<'static, Vec<(u64, Subscriber)>> {
    SUBSCRIBERS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn broadcast(event: &RegistryEvent) {
    // 回调不可克隆(Box<dyn Fn>),持锁派发;契约:回调内禁止再调
    // subscribe/_reset 或丢弃 Subscription(重入即死锁)。订阅者只做
    // 轻量转发(eg 进自家事件队列),重活异步化。
    for (_, callback) in lock_subscribers().iter() {
        callback(event);
    }
}

pub fn register(id: PaneId, meta: PaneSessionMeta) {
    lock_metas().insert(id, meta);
    broadcast(&RegistryEvent::Registered(id));
}

#[allow(dead_code)] // 读端归 runtime_api pane.read/M4+;M3 只有写删两侧。
pub fn get(id: PaneId) -> Option<PaneSessionMeta> {
    METAS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&id)
        .cloned()
}

pub fn remove(id: PaneId) {
    if lock_metas().remove(&id).is_some() {
        broadcast(&RegistryEvent::Removed(id));
    }
}

#[allow(dead_code)] // 订阅端归 RuntimeHub/M4+;契约本体 M3 先立。
pub fn subscribe(f: impl Fn(&RegistryEvent) + Send + 'static) -> Subscription {
    let id = NEXT_SUBSCRIPTION.fetch_add(1, Ordering::Relaxed);
    lock_subscribers().push((id, Box::new(f)));
    Subscription { id }
}

/// 测试隔离专用(beforeEach/afterEach 对);生产禁调。
#[cfg(test)]
pub fn _reset() {
    lock_metas().clear();
    lock_subscribers().clear();
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn pane(n: u64) -> PaneId {
        PaneId(n)
    }

    #[test]
    fn register_get_remove_roundtrip() {
        _reset();
        register(pane(1), PaneSessionMeta::default());
        register(pane(2), PaneSessionMeta::default());
        assert!(get(pane(1)).is_some());
        assert!(get(pane(3)).is_none());
        remove(pane(1));
        assert!(get(pane(1)).is_none());
        assert!(get(pane(2)).is_some());
        // 未注册项 remove 是空操作(不 panic、不误报事件)。
        remove(pane(99));
        _reset();
    }

    #[test]
    fn subscribe_observes_registered_and_removed_in_order() {
        _reset();
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let seen_clone = seen.clone();
        let _sub = subscribe(move |event| seen_clone.lock().unwrap().push(*event));
        register(pane(7), PaneSessionMeta::default());
        remove(pane(7));
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                RegistryEvent::Registered(pane(7)),
                RegistryEvent::Removed(pane(7))
            ]
        );
        _reset();
    }

    #[test]
    fn subscription_drop_stops_delivery() {
        _reset();
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let sub = subscribe(|_| {
            COUNT.fetch_add(1, Ordering::Relaxed);
        });
        register(pane(1), PaneSessionMeta::default());
        drop(sub);
        register(pane(2), PaneSessionMeta::default());
        assert_eq!(COUNT.load(Ordering::Relaxed), 1);
        _reset();
    }
}
