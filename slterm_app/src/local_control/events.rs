//! Runtime 事件环：有界历史、单调 revision 与续传三元组校验。
//!
//! `EventHub` 独占事件环与订阅表；发布与订阅都经有界命令通道进入，
//! 滞后客户端只被移除自己的订阅，不反压发布者。

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use slterm_runtime_protocol::{
    DomainErrorCode, EventsSubscribeParams, EventsSubscribedResponse, JsonRpcError,
    JsonRpcErrorData, RuntimeEvent,
};

/// 全局事件环保留最近 1,024 个轻量事件。
pub(crate) const EVENT_RING_CAPACITY: usize = 1024;
/// 每客户端待发送队列 64 项；滞后客户端只终止自身订阅。
pub(crate) const CLIENT_QUEUE_CAPACITY: usize = 64;

/// 续传不可用：客户端必须重新快照后订阅。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResumeError {
    ResyncRequired,
}

/// 事件命令通道与订阅回执的有界等待。
const HUB_COMMAND_CAPACITY: usize = 64;
const HUB_POLL: Duration = Duration::from_millis(100);
const HUB_REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// 一次事件订阅：确认信息与容量 64 的事件接收端。
pub(crate) struct Subscription {
    pub(crate) confirmed: EventsSubscribedResponse,
    pub(crate) events: Receiver<RuntimeEvent>,
}

/// 订阅注册命令；环与订阅表只由 hub 线程访问。
enum HubCommand {
    Publish(RuntimeEvent),
    Subscribe {
        params: EventsSubscribeParams,
        response: SyncSender<Result<Subscription, ResumeError>>,
    },
}

/// 全局事件环；只由 runtime 事件所有者单写，订阅方只读快照。
pub(crate) struct EventRing {
    store_epoch: String,
    stream_epoch: String,
    events: VecDeque<RuntimeEvent>,
    next_revision: u64,
}

impl EventRing {
    pub(crate) fn new(store_epoch: impl Into<String>, stream_epoch: impl Into<String>) -> Self {
        Self {
            store_epoch: store_epoch.into(),
            stream_epoch: stream_epoch.into(),
            events: VecDeque::new(),
            next_revision: 0,
        }
    }

    /// 发布分配单调 revision；满环丢弃最旧事件，缺口只能以重新快照补偿。
    pub(crate) fn publish(&mut self, mut event: RuntimeEvent) -> u64 {
        let revision = self.next_revision + 1;
        self.next_revision = revision;
        event.revision = revision;
        if self.events.len() == EVENT_RING_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(event);
        revision
    }

    pub(crate) fn latest_revision(&self) -> u64 {
        self.next_revision
    }

    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }

    /// 续传必须同时匹配 store_epoch、stream_epoch 与起点 revision。
    ///
    /// 起点早于环首前一个 revision、或超过当前末尾，都要求重新快照。
    pub(crate) fn resume(
        &self,
        store_epoch: &str,
        stream_epoch: &str,
        since_revision: u64,
    ) -> Result<Vec<RuntimeEvent>, ResumeError> {
        if store_epoch != self.store_epoch
            || stream_epoch != self.stream_epoch
            || since_revision > self.next_revision
        {
            return Err(ResumeError::ResyncRequired);
        }
        let oldest = self
            .events
            .front()
            .map(|event| event.revision)
            .unwrap_or(self.next_revision + 1);
        if since_revision.saturating_add(1) < oldest {
            return Err(ResumeError::ResyncRequired);
        }
        Ok(self
            .events
            .iter()
            .filter(|event| event.revision > since_revision)
            .cloned()
            .collect())
    }
}

/// 事件环与订阅表的唯一所有者线程句柄。
pub(crate) struct EventHub {
    commands: SyncSender<HubCommand>,
    shutdown: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl EventHub {
    pub(crate) fn start(
        store_epoch: impl Into<String>,
        stream_epoch: impl Into<String>,
    ) -> io::Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(HUB_COMMAND_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_thread = Arc::clone(&shutdown);
        let store_epoch = store_epoch.into();
        let stream_epoch = stream_epoch.into();
        let thread = std::thread::Builder::new()
            .name("slterm-runtime-events".to_owned())
            .spawn(move || {
                event_hub_loop(receiver, store_epoch, stream_epoch, shutdown_for_thread)
            })?;
        Ok(Self {
            commands,
            shutdown,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// 非阻塞发布；命令队列满时丢弃并返回 false，不反压状态更新。
    pub(crate) fn publish(&self, event: RuntimeEvent) -> bool {
        self.commands.try_send(HubCommand::Publish(event)).is_ok()
    }

    /// 注册订阅；三元组不匹配、历史缺口或积压超过客户端队列都要求 resync。
    pub(crate) fn subscribe(
        &self,
        params: EventsSubscribeParams,
    ) -> Result<Subscription, JsonRpcError> {
        let (response, receipt) = mpsc::sync_channel(1);
        self.commands
            .try_send(HubCommand::Subscribe { params, response })
            .map_err(|_| hub_unavailable_error())?;
        match receipt.recv_timeout(HUB_REPLY_TIMEOUT) {
            Ok(Ok(subscription)) => Ok(subscription),
            Ok(Err(ResumeError::ResyncRequired)) => Err(control_error(
                DomainErrorCode::ResyncRequired,
                -32007,
                "resync required",
            )),
            Err(_) => Err(hub_unavailable_error()),
        }
    }

    /// 停止 hub 线程并 join；重复调用安全。
    pub(crate) fn shutdown(&self) -> io::Result<()> {
        self.shutdown.store(true, Ordering::SeqCst);
        let thread = self.thread.lock().expect("event hub lock poisoned").take();
        match thread {
            Some(thread) => thread
                .join()
                .map_err(|_| io::Error::other("event hub terminated unexpectedly")),
            None => Ok(()),
        }
    }
}

impl Drop for EventHub {
    fn drop(&mut self) {
        // 正常退出必须先显式 shutdown；Drop 只尽力回收。
        let _ = self.shutdown();
    }
}

fn event_hub_loop(
    receiver: Receiver<HubCommand>,
    store_epoch: String,
    stream_epoch: String,
    shutdown: Arc<AtomicBool>,
) {
    let mut ring = EventRing::new(store_epoch, stream_epoch);
    let mut subscribers: Vec<SyncSender<RuntimeEvent>> = Vec::new();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match receiver.recv_timeout(HUB_POLL) {
            Ok(HubCommand::Publish(event)) => {
                let revision = ring.publish(event.clone());
                deliver_event(&mut subscribers, RuntimeEvent { revision, ..event });
            }
            Ok(HubCommand::Subscribe { params, response }) => {
                let result = subscribe_ring(&mut ring, &mut subscribers, &params);
                let _ = response.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// 向全部订阅投递事件；队列满或接收端断开只移除该订阅，其他订阅继续。
pub(super) fn deliver_event(subscribers: &mut Vec<SyncSender<RuntimeEvent>>, event: RuntimeEvent) {
    subscribers.retain(|sender| match sender.try_send(event.clone()) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
    });
}

fn subscribe_ring(
    ring: &mut EventRing,
    subscribers: &mut Vec<SyncSender<RuntimeEvent>>,
    params: &EventsSubscribeParams,
) -> Result<Subscription, ResumeError> {
    let backlog = ring.resume(
        &params.store_epoch,
        &params.stream_epoch,
        params.since_revision,
    )?;
    if backlog.len() > CLIENT_QUEUE_CAPACITY {
        return Err(ResumeError::ResyncRequired);
    }
    let (sender, events) = sync_channel(CLIENT_QUEUE_CAPACITY);
    for event in backlog {
        if sender.try_send(event).is_err() {
            return Err(ResumeError::ResyncRequired);
        }
    }
    subscribers.push(sender);
    Ok(Subscription {
        confirmed: EventsSubscribedResponse {
            stream_epoch: params.stream_epoch.clone(),
            revision: ring.latest_revision(),
        },
        events,
    })
}

fn hub_unavailable_error() -> JsonRpcError {
    control_error(DomainErrorCode::NotReady, -32001, "event hub unavailable")
}

fn control_error(domain: DomainErrorCode, numeric: i32, message: &'static str) -> JsonRpcError {
    JsonRpcError::new(numeric, message, Some(JsonRpcErrorData { code: domain }))
}
