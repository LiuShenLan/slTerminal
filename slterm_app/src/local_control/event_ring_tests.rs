//! Runtime 事件环：有界保留、续传三元组与缺口检测。

use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use slterm_runtime_protocol::{EventsSubscribeParams, RuntimeEvent, RuntimeEventKind};

use super::events::{CLIENT_QUEUE_CAPACITY, EVENT_RING_CAPACITY, EventHub, EventRing, ResumeError};

fn event(kind: RuntimeEventKind, resource: &str) -> RuntimeEvent {
    RuntimeEvent {
        revision: 0,
        kind,
        resource: resource.to_owned(),
        generation: 1,
    }
}

#[test]
fn event_ring_assigns_monotonic_revisions_and_resumes_without_gap() {
    let mut ring = EventRing::new("store-a", "stream-a");
    assert_eq!(ring.latest_revision(), 0);
    for index in 0..3 {
        let revision = ring.publish(event(
            RuntimeEventKind::PanelActivated,
            &format!("panel-{index}"),
        ));
        assert_eq!(revision, index + 1);
    }
    let resumed = ring.resume("store-a", "stream-a", 1).unwrap();
    assert_eq!(resumed.len(), 2);
    assert_eq!(resumed[0].revision, 2);
    assert_eq!(resumed[1].kind, RuntimeEventKind::PanelActivated);

    // 已见最新事件时返回空增量，而不是错误。
    assert!(ring.resume("store-a", "stream-a", 3).unwrap().is_empty());
}

#[test]
fn event_ring_keeps_only_the_bounded_tail_and_requests_resync_on_gap() {
    let mut ring = EventRing::new("store-a", "stream-a");
    for index in 0..(EVENT_RING_CAPACITY + 5) {
        ring.publish(event(
            RuntimeEventKind::TerminalOutput,
            &format!("terminal-{index}"),
        ));
    }
    assert_eq!(ring.len(), EVENT_RING_CAPACITY);
    assert_eq!(
        ring.resume("store-a", "stream-a", 1),
        Err(ResumeError::ResyncRequired),
        "被淘汰的历史必须要求重新快照"
    );
    let resumed = ring
        .resume("store-a", "stream-a", EVENT_RING_CAPACITY as u64)
        .unwrap();
    assert_eq!(resumed.len(), 5);
    assert_eq!(resumed[0].revision, EVENT_RING_CAPACITY as u64 + 1);
}

#[test]
fn event_ring_rejects_stale_store_or_stream_epoch() {
    let mut ring = EventRing::new("store-a", "stream-a");
    ring.publish(event(RuntimeEventKind::ProjectActivated, "project-a"));
    assert_eq!(
        ring.resume("store-b", "stream-a", 0),
        Err(ResumeError::ResyncRequired)
    );
    assert_eq!(
        ring.resume("store-a", "stream-b", 0),
        Err(ResumeError::ResyncRequired)
    );
    // 同时匹配三元组后立即得到增量。
    assert_eq!(ring.resume("store-a", "stream-a", 0).unwrap().len(), 1);
}

fn subscribe_params(
    store_epoch: &str,
    stream_epoch: &str,
    since_revision: u64,
) -> EventsSubscribeParams {
    EventsSubscribeParams {
        store_epoch: store_epoch.to_owned(),
        stream_epoch: stream_epoch.to_owned(),
        since_revision,
    }
}

#[test]
fn event_hub_publishes_incrementally_and_rejects_resync() {
    let hub = EventHub::start("store-a", "stream-a").unwrap();
    let subscription = hub
        .subscribe(subscribe_params("store-a", "stream-a", 0))
        .expect("三元组匹配必须订阅成功");
    assert_eq!(subscription.confirmed.revision, 0);
    assert_eq!(subscription.confirmed.stream_epoch, "stream-a");

    assert!(hub.publish(event(RuntimeEventKind::ProjectActivated, "project-a")));
    let received = subscription
        .events
        .recv_timeout(Duration::from_secs(2))
        .expect("发布事件必须送达订阅端");
    assert_eq!(received.revision, 1);
    assert_eq!(received.kind, RuntimeEventKind::ProjectActivated);

    assert!(
        hub.subscribe(subscribe_params("store-b", "stream-a", 0))
            .is_err()
    );
    assert!(
        hub.subscribe(subscribe_params("store-a", "stream-a", 99))
            .is_err()
    );
    hub.shutdown().unwrap();
}

#[test]
fn lagging_subscription_is_removed_without_blocking_publishers() {
    // 直接覆盖投递语义：队列满即移除该订阅，发布端不等待。
    let (sender, receiver) = std::sync::mpsc::sync_channel(CLIENT_QUEUE_CAPACITY);
    let mut subscribers = vec![sender];
    for index in 0..(CLIENT_QUEUE_CAPACITY + 2) {
        super::events::deliver_event(
            &mut subscribers,
            event(
                RuntimeEventKind::TerminalOutput,
                &format!("terminal-{index}"),
            ),
        );
    }
    assert!(subscribers.is_empty(), "滞后订阅必须被移除");

    let mut received = 0;
    loop {
        match receiver.try_recv() {
            Ok(_) => received += 1,
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => panic!("队列内事件应全部可读"),
        }
    }
    assert_eq!(received, CLIENT_QUEUE_CAPACITY, "只在队列容量内保留事件");
}

#[test]
fn event_hub_backlog_over_client_capacity_requires_resync() {
    let hub = EventHub::start("store-a", "stream-a").unwrap();
    // 命令通道有界：满时短暂重试，确保全部发布都进入 hub。
    for index in 0..(CLIENT_QUEUE_CAPACITY + 1) {
        while !hub.publish(event(
            RuntimeEventKind::TerminalOutput,
            &format!("terminal-{index}"),
        )) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    // 等待 hub 处理完全部发布后再订阅，确保增量大于客户端队列。
    std::thread::sleep(Duration::from_millis(300));
    let result = hub.subscribe(subscribe_params("store-a", "stream-a", 0));
    assert!(result.is_err(), "积压超过客户端队列必须要求 resync");
    hub.shutdown().unwrap();
}
