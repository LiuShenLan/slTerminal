//! 设置运行值与唯一写队列：项目 Settings 与无项目浮层共享同一状态。
//!
//! 运行值与磁盘值分开：写失败保留生效值、待写集合与失败标记，由后续
//! 修改或显式重试继续；关闭任一订阅视图不取消全局待写。Hooks 草稿不走
//! 本队列（草稿归 `ai_hook::draft_host`）。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use slterm_settings::json::Value;
use slterm_settings::{RawSettings, RuntimeSettings, keys};
use slterm_workspace::{
    InvalidViewState, ViewKey, ViewKeyKind, ViewState, ViewStateError, ViewStateRegistry,
    ViewStateSchema,
};

use crate::persistence::workspace_writer::WorkspaceWriterClient;

/// 启动读取设置时需要用户可见的降级事实。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsLoadWarning {
    /// 主文件损坏，已从备份或默认值恢复；原文件已隔离保留。
    Corrupted,
    /// 文件由更高版本写入；当前使用默认值且写通道拒绝覆盖。
    UnsupportedVersion(u64),
    /// 权限／IO 读取失败；当前运行使用默认值。
    ReadFailed,
}

/// 启动读取状态；独立全局避免第二次读盘丢失损坏／高版本事实。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SettingsLoadState {
    pub(crate) warning: Option<SettingsLoadWarning>,
}

impl gpui::Global for SettingsLoadState {}

/// 设置页的封闭页面集合；Hooks 页只经有 CLI 编辑能力的宿主提供。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SettingsPageId {
    Appearance,
    Scrolling,
    Interaction,
    Session,
    System,
    AgentHooks,
    Hooks,
}

impl SettingsPageId {
    pub(crate) const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Scrolling,
        Self::Interaction,
        Self::Session,
        Self::System,
        Self::AgentHooks,
        Self::Hooks,
    ];

    /// 页面标题只经 typed i18n 查询，不拼接英文枚举名。
    pub(crate) const fn message(self) -> crate::i18n::Message {
        use crate::i18n::Message;
        match self {
            Self::Appearance => Message::SettingsSidebarAppearance,
            Self::Scrolling => Message::SettingsSidebarScrolling,
            Self::Interaction => Message::SettingsSidebarInteraction,
            Self::Session => Message::SettingsSidebarSession,
            Self::System => Message::SettingsSidebarSystem,
            Self::AgentHooks => Message::SettingsSidebarAgents,
            Self::Hooks => Message::SettingsSidebarHooks,
        }
    }
}

/// 设置页仅持久化选中页与声明式页内参数；搜索、录制、临时错误、
/// 普通输入草稿以及 Hooks 层级／GUI 模式都是瞬态，不进入本 DTO。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettingsViewState {
    selected_page: SettingsPageId,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    declared_page_params: BTreeMap<String, String>,
}

impl SettingsViewState {
    pub(crate) fn new(selected_page: SettingsPageId) -> Self {
        Self {
            selected_page,
            declared_page_params: BTreeMap::new(),
        }
    }

    pub(crate) fn selected_page(&self) -> SettingsPageId {
        self.selected_page
    }

    pub(crate) fn declared_page_params(&self) -> &BTreeMap<String, String> {
        &self.declared_page_params
    }

    pub(crate) fn with_page_param(mut self, key: &str, value: &str) -> Self {
        self.declared_page_params
            .insert(key.to_owned(), value.to_owned());
        self
    }

    /// 失效选中页回退到宿主第一个可用页并报告回退；不得创建空白页。
    pub(crate) fn restore(
        selected_page: SettingsPageId,
        declared_page_params: BTreeMap<String, String>,
        available: &[SettingsPageId],
    ) -> (Self, bool) {
        if available.contains(&selected_page) {
            return (
                Self {
                    selected_page,
                    declared_page_params,
                },
                false,
            );
        }
        let fallback = available
            .first()
            .copied()
            .unwrap_or(SettingsPageId::Appearance);
        (
            Self {
                selected_page: fallback,
                declared_page_params,
            },
            true,
        )
    }

    pub(crate) fn to_view_state(
        &self,
        view_key: &ViewKey,
        registry: &ViewStateRegistry,
    ) -> Result<ViewState, ViewStateError> {
        let value = serde_json::to_value(self).map_err(|_| ViewStateError::Encode)?;
        ViewState::new(view_key, value, registry)
    }
}

/// Settings 面板与无项目浮层各自按完整 ViewKey 解码同一 DTO。
pub(crate) fn settings_view_state_schemas() -> Vec<ViewStateSchema> {
    [ViewKeyKind::Settings, ViewKeyKind::AppSettingsOverlay]
        .map(|kind| ViewStateSchema::new(kind, decode_settings_view_state))
        .into_iter()
        .collect()
}

/// Settings 面板的 ViewState 通道：同 ViewKey 关闭重开与重启恢复共用一份状态。
#[derive(Clone)]
pub(crate) struct SettingsViewStateStore {
    registry: Arc<ViewStateRegistry>,
    writer: Option<WorkspaceWriterClient>,
    restored: Rc<RefCell<Vec<(ViewKey, SettingsViewState)>>>,
}

impl SettingsViewStateStore {
    pub(crate) fn new(
        registry: Arc<ViewStateRegistry>,
        writer: Option<WorkspaceWriterClient>,
    ) -> Self {
        Self {
            registry,
            writer,
            restored: Rc::default(),
        }
    }

    pub(crate) fn restore(&self, key: ViewKey, state: SettingsViewState) {
        self.insert(key, state);
    }

    pub(crate) fn state(&self, key: &ViewKey) -> Option<SettingsViewState> {
        self.restored
            .borrow()
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, state)| state.clone())
    }

    fn insert(&self, key: ViewKey, state: SettingsViewState) {
        let mut restored = self.restored.borrow_mut();
        if let Some((_, existing)) = restored.iter_mut().find(|(existing, _)| *existing == key) {
            *existing = state;
        } else {
            restored.push((key, state));
        }
    }

    /// 显式切页才写盘；写失败保留内存状态供后续重试，不回滚当前选择。
    pub(crate) fn checkpoint(&self, key: &ViewKey, state: SettingsViewState) {
        self.insert(key.clone(), state.clone());
        let Ok(view_state) = state.to_view_state(key, &self.registry) else {
            log::warn!("设置页状态编码失败，保留内存状态");
            return;
        };
        if let Some(writer) = &self.writer
            && let Err(error) = writer.save_view_state(key, &view_state)
        {
            log::warn!("设置页状态写请求失败: {error}");
        }
    }
}

fn decode_settings_view_state(
    key: &ViewKey,
    text: &str,
) -> Result<serde_json::Value, InvalidViewState> {
    match key {
        ViewKey::Settings { .. } | ViewKey::AppSettingsOverlay => {}
        _ => return Err(InvalidViewState),
    }
    let state: SettingsViewState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
    serde_json::to_value(state).map_err(|_| InvalidViewState)
}

/// 自动保存延迟：停止编辑满 2 秒后落盘。
pub(crate) const AUTO_SAVE_DELAY: Duration = Duration::from_secs(2);

/// 全局设置运行值与唯一待写队列；由 AppRoot 持有，视图只持订阅身份。
/// 字段提交的联动清除规则：选择内置主题时清除自定义主题覆盖，
/// 否则 `resolve_theme` 仍优先加载旧的自定义主题。
pub(crate) fn theme_field_followups(key: &str) -> &'static [&'static str] {
    if key == "theme" {
        &["custom_theme"]
    } else {
        &[]
    }
}

pub(crate) struct SettingsStore {
    values: RawSettings,
    pending: BTreeMap<String, Value>,
    last_edit: Option<Instant>,
    persist_failed: bool,
    /// 后台写盘在途标记：同一时刻只允许一批，避免前台重复启动任务。
    in_flight: bool,
    subscribers: BTreeSet<u64>,
    next_subscriber: u64,
}

impl SettingsStore {
    pub(crate) fn new(values: RawSettings) -> Self {
        Self {
            values,
            pending: BTreeMap::new(),
            last_edit: None,
            persist_failed: false,
            in_flight: false,
            subscribers: BTreeSet::new(),
            next_subscriber: 0,
        }
    }

    /// 当前生效值：待写修改已经生效，不等待落盘。
    pub(crate) fn runtime(&self) -> RuntimeSettings {
        RuntimeSettings::from_raw(&self.values)
    }

    pub(crate) fn values(&self) -> &RawSettings {
        &self.values
    }

    /// 自动保存字段：立即生效并进入待写；未知键拒绝（控件只来自注册表）。
    pub(crate) fn set_auto(&mut self, key: &'static str, value: Value, now: Instant) -> bool {
        if keys::lookup(key).is_none() {
            return false;
        }
        self.values = self.values.with_updates(&[(key, value.clone())]);
        self.pending.insert(key.to_owned(), value);
        self.last_edit = Some(now);
        true
    }

    /// 清除可选字段：写入 Null 表示回到缺省，立即生效并进入待写。
    pub(crate) fn clear_auto(&mut self, key: &'static str, now: Instant) -> bool {
        if keys::lookup(key).is_none() {
            return false;
        }
        self.values = self.values.with_updates(&[(key, Value::Null)]);
        self.pending.insert(key.to_owned(), Value::Null);
        self.last_edit = Some(now);
        true
    }

    /// 显式动作（导入、恢复默认、后台任务）：立即写入，不等去抖。
    pub(crate) fn save_explicit(
        &mut self,
        updates: &[(&str, Value)],
        now: Instant,
        write: impl FnOnce(&[(&str, Value)]) -> io::Result<()>,
    ) -> io::Result<()> {
        for (key, _) in updates {
            if keys::lookup(key).is_none() && !keys::is_external_segment(key) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "设置键不在白名单内",
                ));
            }
        }
        self.values = self.values.with_updates(updates);
        for (key, value) in updates {
            self.pending.insert((*key).to_owned(), value.clone());
        }
        self.flush(now, write)
    }

    /// 显式字段（授权开关）：只入队并立即到期，真实写盘仍走后台批量。
    pub(crate) fn queue_explicit(&mut self, updates: &[(&str, Value)], now: Instant) -> bool {
        if updates.iter().any(|(key, _)| keys::lookup(key).is_none()) {
            return false;
        }
        self.values = self.values.with_updates(updates);
        for (key, value) in updates {
            self.pending.insert((*key).to_owned(), value.clone());
        }
        // 立即到期等价于「满足去抖窗口」，仍由下一节拍的后台写盘接管。
        self.last_edit = Some(now.checked_sub(AUTO_SAVE_DELAY).unwrap_or(now));
        true
    }

    /// 后台写盘批次入口：到期且无在途时返回待写快照；不清空 pending。
    pub(crate) fn begin_save(&mut self, now: Instant) -> Option<Vec<(String, Value)>> {
        if self.in_flight || self.pending.is_empty() {
            return None;
        }
        let last_edit = self.last_edit?;
        if now.saturating_duration_since(last_edit) < AUTO_SAVE_DELAY {
            return None;
        }
        self.in_flight = true;
        Some(
            self.pending
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )
    }

    /// 任务注册失败时撤销在途标记，使同一批快照可在下一节拍重试。
    pub(crate) fn abort_save(&mut self) {
        self.in_flight = false;
    }

    /// 结算后台写盘：成功只清除与写入值一致的待写项，较新编辑保留；
    /// 失败保留全部并从 `now` 起退避一个延迟窗口。
    pub(crate) fn finish_save(
        &mut self,
        written: &[(String, Value)],
        result: io::Result<()>,
        now: Instant,
    ) {
        self.in_flight = false;
        match result {
            Ok(()) => {
                for (key, value) in written {
                    if self
                        .pending
                        .get(key)
                        .is_some_and(|current| current == value)
                    {
                        self.pending.remove(key);
                    }
                }
                self.persist_failed = false;
                if self.pending.is_empty() {
                    self.last_edit = None;
                }
            }
            Err(_) => {
                self.persist_failed = true;
                self.last_edit = Some(now);
            }
        }
    }

    /// 自动保存时机：距最后一次编辑满 [`AUTO_SAVE_DELAY`] 且有可写项。
    /// 返回是否实际执行了一次写入。
    pub(crate) fn tick(
        &mut self,
        now: Instant,
        write: impl FnOnce(&[(&str, Value)]) -> io::Result<()>,
    ) -> io::Result<bool> {
        let Some(last_edit) = self.last_edit else {
            return Ok(false);
        };
        if now.saturating_duration_since(last_edit) < AUTO_SAVE_DELAY {
            return Ok(false);
        }
        if self.pending.is_empty() {
            return Ok(false);
        }
        self.flush(now, write)?;
        Ok(true)
    }

    /// 写失败保留生效值与待写集合，并标记「本次运行生效、重启可能丢失」。
    fn flush(
        &mut self,
        _now: Instant,
        write: impl FnOnce(&[(&str, Value)]) -> io::Result<()>,
    ) -> io::Result<()> {
        let entries = self
            .pending
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone()))
            .collect::<Vec<_>>();
        match write(&entries) {
            Ok(()) => {
                self.pending.clear();
                self.persist_failed = false;
                self.last_edit = None;
                Ok(())
            }
            Err(error) => {
                self.persist_failed = true;
                Err(error)
            }
        }
    }

    pub(crate) fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub(crate) fn persist_failed(&self) -> bool {
        self.persist_failed
    }

    /// 视图订阅身份；关闭视图只退订，不影响全局待写集合。
    pub(crate) fn subscribe(&mut self) -> u64 {
        self.next_subscriber += 1;
        let id = self.next_subscriber;
        self.subscribers.insert(id);
        id
    }

    pub(crate) fn unsubscribe(&mut self, id: u64) {
        self.subscribers.remove(&id);
    }

    pub(crate) fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn store() -> SettingsStore {
        SettingsStore::new(RawSettings::default())
    }

    type RecordedWrites = Rc<RefCell<Vec<Vec<(String, Value)>>>>;
    type WriteProbe = (
        RecordedWrites,
        Box<dyn Fn(&[(&str, Value)]) -> io::Result<()>>,
    );

    fn record_writes() -> WriteProbe {
        let writes: RecordedWrites = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&writes);
        let write = move |entries: &[(&str, Value)]| {
            captured.borrow_mut().push(
                entries
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), value.clone()))
                    .collect(),
            );
            Ok(())
        };
        (writes, Box::new(write))
    }

    /// 字段提交的联动清除规则：选择内置主题时清除自定义主题覆盖。
    #[test]
    fn theme_field_followups_clear_only_for_builtin_theme_selection() {
        assert_eq!(theme_field_followups("theme"), &["custom_theme"]);
        assert!(theme_field_followups("font_size").is_empty());
    }

    /// 选择内置主题必须清除自定义主题覆盖，否则旧 custom_theme 继续生效。
    #[test]
    fn builtin_theme_selection_clears_custom_theme_override() {
        let mut store = SettingsStore::new(RawSettings::from_json_bytes(
            br#"{"appearance":{"theme":"Linear","custom_theme":"my-night"}}"#,
        ));
        assert_eq!(store.runtime().custom_theme.as_deref(), Some("my-night"));
        let now = Instant::now();
        assert!(store.set_auto("theme", Value::String("Nord".to_owned()), now));
        for key in theme_field_followups("theme") {
            assert!(store.clear_auto(key, now));
        }
        assert_eq!(store.runtime().custom_theme, None);
        assert_eq!(
            store.values().value("theme"),
            Some("Nord"),
            "内置主题选择必须生效"
        );
    }

    /// 后台写盘：在途不重复启动；写入期间的新编辑保留；失败退避后重试。
    #[test]
    fn settings_async_save_keeps_newer_edits_and_backs_off_on_failure() {
        let mut store = store();
        let t0 = Instant::now();
        assert!(store.set_auto("theme", Value::String("Nord".to_owned()), t0));
        assert!(
            store
                .begin_save(t0 + Duration::from_millis(1_999))
                .is_none(),
            "未满 2 秒不得开始写盘"
        );
        let batch = store
            .begin_save(t0 + AUTO_SAVE_DELAY)
            .expect("到期必须返回待写快照");
        assert_eq!(
            batch,
            vec![("theme".to_owned(), Value::String("Nord".to_owned()))]
        );
        assert!(
            store
                .begin_save(t0 + Duration::from_millis(2_100))
                .is_none(),
            "在途写盘不得重复启动"
        );

        // 写入在途期间的新编辑必须保留，且旧快照成功不得清掉新值。
        assert!(store.set_auto(
            "theme",
            Value::String("MintDark".to_owned()),
            t0 + Duration::from_millis(2_050)
        ));
        store.finish_save(&batch, Ok(()), t0 + Duration::from_millis(2_100));
        assert_eq!(store.pending_len(), 1, "较新编辑必须保留");
        let retried = store
            .begin_save(t0 + Duration::from_millis(4_100))
            .expect("新值到期后重试");
        assert_eq!(
            retried,
            vec![("theme".to_owned(), Value::String("MintDark".to_owned()))]
        );

        store.finish_save(
            &retried,
            Err(io::Error::other("disk full")),
            t0 + Duration::from_millis(4_100),
        );
        assert!(store.persist_failed());
        assert_eq!(store.pending_len(), 1, "失败保留待写");
        assert!(
            store
                .begin_save(t0 + Duration::from_millis(5_000))
                .is_none(),
            "失败后按同一延迟退避"
        );
        let recovered = store
            .begin_save(t0 + Duration::from_millis(6_200))
            .expect("退避到期必须重试");
        store.finish_save(&recovered, Ok(()), t0 + Duration::from_millis(6_200));
        assert!(!store.persist_failed());
        assert_eq!(store.pending_len(), 0);
    }

    /// 显式字段（授权开关）只入队并立即到期，未知键拒绝。
    #[test]
    fn settings_explicit_fields_queue_immediately_and_reject_unknown_keys() {
        let mut store = store();
        let t0 = Instant::now();
        assert!(!store.queue_explicit(&[("not_a_key", Value::Bool(true))], t0));
        assert!(
            store.queue_explicit(&[("slterm_ai_hooks", Value::Bool(true))], t0),
            "全局授权键必须可入队"
        );
        let batch = store.begin_save(t0).expect("显式字段必须立即到期");
        assert_eq!(
            batch,
            vec![("slterm_ai_hooks".to_owned(), Value::Bool(true))]
        );
        store.finish_save(&batch, Ok(()), t0 + Duration::from_millis(2_100));
        assert_eq!(store.pending_len(), 0);
    }

    /// 两个视图的修改进入同一队列；关闭一个视图不取消全局待写。
    #[test]
    fn settings_single_queue_across_views() {
        let mut store = store();
        let view_a = store.subscribe();
        let _view_b = store.subscribe();
        let t0 = Instant::now();
        assert!(store.set_auto("font_size", Value::Number(14.0), t0));
        assert!(store.set_auto("theme", Value::String("Nord".to_owned()), t0));
        assert_eq!(store.pending_len(), 2, "两个视图的修改必须共享同一队列");

        store.unsubscribe(view_a);
        assert_eq!(store.subscriber_count(), 1);
        assert_eq!(store.pending_len(), 2, "关闭视图不得取消全局待写");

        assert!(
            !store
                .tick(t0 + Duration::from_millis(1_999), |_| {
                    panic!("未满 2 秒不得自动保存")
                })
                .unwrap(),
            "未满 2 秒不得自动保存"
        );
        let (writes, write) = record_writes();
        assert!(
            store
                .tick(t0 + Duration::from_millis(2_000), write)
                .unwrap(),
            "满 2 秒后必须自动保存"
        );
        assert_eq!(writes.borrow()[0].len(), 2);
        assert_eq!(store.pending_len(), 0);
    }

    /// 两个视图读取同一生效值；写失败保留生效值与待写，重试成功才清除。
    #[test]
    fn settings_views_share_global_queue() {
        let mut store = store();
        let _a = store.subscribe();
        let _b = store.subscribe();
        let t0 = Instant::now();
        assert!(store.set_auto("font_size", Value::Number(16.0), t0));
        assert_eq!(store.runtime().font_size_px, Some(16.0), "运行值立即生效");

        let error = store
            .tick(t0 + AUTO_SAVE_DELAY, |_| Err(io::Error::other("disk full")))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(store.persist_failed(), "写失败必须可见");
        assert_eq!(store.pending_len(), 1, "写失败必须保留待写");
        assert_eq!(
            store.runtime().font_size_px,
            Some(16.0),
            "写失败保留本次运行生效值"
        );

        let (writes, write) = record_writes();
        assert!(store.tick(t0 + AUTO_SAVE_DELAY, write).unwrap());
        assert!(!store.persist_failed());
        assert_eq!(store.pending_len(), 0);
        assert_eq!(writes.borrow()[0][0].0, "font_size");
    }

    /// 显式动作立即写入；未知键零写入。
    #[test]
    fn settings_explicit_actions_write_immediately() {
        let mut store = store();
        let (writes, write) = record_writes();
        store
            .save_explicit(
                &[("theme", Value::String("Nord".to_owned()))],
                Instant::now(),
                write,
            )
            .unwrap();
        assert_eq!(writes.borrow().len(), 1);
        assert_eq!(store.pending_len(), 0);

        let error = store
            .save_explicit(
                &[("not_a_setting", Value::Bool(true))],
                Instant::now(),
                |_| panic!("未知键不得进入写通道"),
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    /// 选中页按完整 ViewKey 恢复；瞬态字段拒绝，失效页回退并报告。
    #[test]
    fn settings_page_view_state_restore() {
        use slterm_workspace::{ViewKey, ViewKeyKind, ViewState, ViewStateRegistry};
        use std::collections::BTreeSet;
        use std::str::FromStr as _;

        let registry = ViewStateRegistry::new(&settings_view_state_schemas()).unwrap();
        let project =
            slterm_workspace::ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let project_key = ViewKey::Settings {
            project_id: project,
        };
        let state =
            SettingsViewState::new(SettingsPageId::Session).with_page_param("focus", "terminal");
        let project_state = state.to_view_state(&project_key, &registry).unwrap();
        let overlay_state = state
            .to_view_state(&ViewKey::AppSettingsOverlay, &registry)
            .unwrap();
        assert_ne!(
            project_state.to_canonical_json().unwrap(),
            overlay_state.to_canonical_json().unwrap(),
            "项目 Settings 与浮层按完整 ViewKey 独立存取"
        );

        let persisted = project_state
            .state()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            persisted,
            BTreeSet::from([
                "declared_page_params".to_owned(),
                "selected_page".to_owned()
            ]),
            "搜索、录制与普通输入等瞬态不得进入持久状态"
        );

        let text = project_state.to_canonical_json().unwrap();
        let restored = ViewState::from_canonical_json(&text, &project_key, &registry).unwrap();
        assert_eq!(restored.kind(), ViewKeyKind::Settings);
        assert_eq!(restored.state(), project_state.state());

        let mut envelope: serde_json::Value = serde_json::from_str(&text).unwrap();
        envelope["state"]["search"] = serde_json::json!("transient");
        let bad = serde_json::to_string(&envelope).unwrap();
        assert!(
            ViewState::from_canonical_json(&bad, &project_key, &registry).is_err(),
            "未登记瞬态字段必须拒绝"
        );

        let (restored, fell_back) = SettingsViewState::restore(
            SettingsPageId::Hooks,
            BTreeMap::new(),
            &[SettingsPageId::Appearance, SettingsPageId::Session],
        );
        assert_eq!(restored.selected_page(), SettingsPageId::Appearance);
        assert!(fell_back, "失效选中页必须回退到首个可用页");
        let (restored, fell_back) = SettingsViewState::restore(
            SettingsPageId::Session,
            BTreeMap::new(),
            &[SettingsPageId::Appearance, SettingsPageId::Session],
        );
        assert_eq!(restored.selected_page(), SettingsPageId::Session);
        assert!(!fell_back);
    }

    /// 读侧分流：缺失无警示、单字段非法只回退自身、损坏隔离并标记。
    #[test]
    fn settings_damage_classification() {
        use crate::settings_store::{load_settings_at, save_settings_blocking_at};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let missing = load_settings_at(&path).unwrap();
        assert!(!missing.corrupted, "缺失文件不得标记损坏");
        assert_eq!(RuntimeSettings::from_raw(&missing.data).font_size_px, None);

        save_settings_blocking_at(
            &path,
            &[
                ("font_size", Value::Number(15.0)),
                ("theme", Value::String("Nord".to_owned())),
            ],
        )
        .unwrap();
        std::fs::write(
            &path,
            br#"{"appearance":{"font_size":"not-a-number","theme":"Nord"}}"#,
        )
        .unwrap();
        let single = load_settings_at(&path).unwrap();
        assert!(!single.corrupted, "单字段非法不是文件损坏");
        let runtime = RuntimeSettings::from_raw(&single.data);
        assert_eq!(runtime.font_size_px, None, "非法字段只回退自身");
        assert_eq!(
            runtime.theme,
            slterm_settings::ThemeName::Nord,
            "其他字段保留"
        );

        std::fs::write(&path, b"{broken").unwrap();
        let corrupt = load_settings_at(&path).unwrap();
        assert!(corrupt.corrupted, "损坏文件必须标记并隔离");
        assert!(
            std::fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().contains("corrupted")),
            "损坏原件必须保留为诊断材料"
        );
    }
}
