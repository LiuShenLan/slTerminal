//! 设置运行值与唯一写队列：项目 Settings 与无项目浮层共享同一状态。
//!
//! 运行值与磁盘值分开：写失败保留生效值、待写集合与失败标记，由后续
//! 修改或显式重试继续；关闭任一订阅视图不取消全局待写。Hooks 草稿不走
//! 本队列（草稿归 `ai_hook::draft_host`）。

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::time::{Duration, Instant};

use slterm_settings::json::Value;
use slterm_settings::{RawSettings, RuntimeSettings, keys};

/// 自动保存延迟：停止编辑满 2 秒后落盘。
pub(crate) const AUTO_SAVE_DELAY: Duration = Duration::from_secs(2);

/// 全局设置运行值与唯一待写队列；由 AppRoot 持有，视图只持订阅身份。
pub(crate) struct SettingsStore {
    values: RawSettings,
    pending: BTreeMap<String, Value>,
    last_edit: Option<Instant>,
    persist_failed: bool,
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
