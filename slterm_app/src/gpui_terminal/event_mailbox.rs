//! 事件信箱:入队前合并重绘唤醒(Wakeup),语义事件一条不丢。
//!
//! 因果链:PTY 输出爆发期每字节都可能触发 Wakeup,若全量入队,UI 线程会被
//! 重绘请求淹没;Wakeup 语义本就是「有输出待渲染」的布尔事实,合并到一条不
//! 丢信息。其余事件(Exit/Title/CommandStart…)携带语义,必须全保留。
//! 唤醒位在处理**前**清零(见 `acknowledge`):UI 处理期间到达的新输出必须
//! 能再入队一次重绘,哪怕落在非活动 tab 上。

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::channel::mpsc;
use futures::{SinkExt as _, Stream};
use slterm_terminal::event::Event;
use slterm_terminal::render::TerminalFrame;

/// 两个最大事件批次的容量；worker 满载时等待，不丢弃语义事件。
const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone, Default)]
struct FrameState(Arc<Mutex<Option<Arc<TerminalFrame>>>>);

#[derive(Clone)]
pub(super) struct EventSender {
    sender: mpsc::Sender<Event>,
    wake_pending: Arc<AtomicBool>,
    frame: FrameState,
    pub(super) native_prompt: NativePromptState,
}

#[derive(Clone, Default)]
pub(super) struct NativePromptState(Arc<Mutex<NativePrompt>>);

/// PTY 接收与用户输入共用的边界;队列消费只唤醒视图,不重新给旧标记盖章。
#[derive(Clone, Copy, Default)]
pub(super) struct NativePrompt {
    pub seen: bool,
    pub input_epoch: u64,
    pub pending: bool,
}

pub(crate) struct EventReceiver {
    receiver: mpsc::Receiver<Event>,
    wake_pending: Arc<AtomicBool>,
    frame: FrameState,
}

pub(super) fn channel() -> (EventSender, EventReceiver) {
    let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
    let wake_pending = Arc::new(AtomicBool::new(false));
    let frame = FrameState::default();
    (
        EventSender {
            sender,
            wake_pending: wake_pending.clone(),
            frame: frame.clone(),
            native_prompt: NativePromptState::default(),
        },
        EventReceiver {
            receiver,
            wake_pending,
            frame,
        },
    )
}

/// 编辑 cmd 输入行不使已收到的原生提示符失效;提交与未知控制序列保持保守。
pub(super) fn preserves_native_prompt(mut bytes: &[u8]) -> bool {
    let editing_control = |value| {
        matches!(
            value,
            1 | 2 | 5 | 6 | 8 | 9 | 11 | 12 | 14 | 16 | 18 | 21 | 23 | 25 | 27 | 127
        )
    };
    while let Some((&byte, rest)) = bytes.split_first() {
        if byte != 0x1b {
            if byte.is_ascii_control() && (byte == 27 || !editing_control(u16::from(byte))) {
                return false;
            }
            bytes = rest;
            continue;
        }
        let Some(sequence) = bytes.strip_prefix(b"\x1b[") else {
            return false;
        };
        let Some(end) = sequence
            .iter()
            .position(|byte| (0x40..=0x7e).contains(byte))
        else {
            return false;
        };
        let parameters = &sequence[..end];
        match sequence[end] {
            b'A'..=b'D' | b'H' | b'F' if parameters.is_empty() => {}
            b'~' if matches!(parameters, b"1" | b"2" | b"3" | b"4" | b"5" | b"6") => {}
            b'_' => {
                let Ok(parameters) = std::str::from_utf8(parameters) else {
                    return false;
                };
                let mut fields = parameters.split(';');
                let mut values = [0u16; 6];
                for value in &mut values {
                    let Some(parsed) = fields.next().and_then(|field| field.parse().ok()) else {
                        return false;
                    };
                    *value = parsed;
                }
                if fields.next().is_some() || values[3] > 1 {
                    return false;
                }
                // Vk;Sc;Uc;Kd;Cs;Rc:Enter 即使 Uc 为零也是边界。
                if values[0] == 13
                    || (values[2] < 32 && values[2] != 0 && !editing_control(values[2]))
                {
                    return false;
                }
            }
            _ => return false,
        }
        bytes = &sequence[end + 1..];
    }
    true
}

impl NativePromptState {
    pub(super) fn snapshot(&self) -> NativePrompt {
        *self.0.lock().unwrap()
    }

    pub(super) fn observe_input(&self, input_epoch: u64, preserves_prompt: bool) {
        let mut prompt = self.0.lock().unwrap();
        prompt.input_epoch = input_epoch;
        prompt.pending &= preserves_prompt;
    }

    pub(super) fn observe_prompt(&self) {
        let mut prompt = self.0.lock().unwrap();
        prompt.seen = true;
        prompt.pending = true;
    }
}

impl EventSender {
    pub(super) fn send(&self, event: Event) {
        // 原生提示符旁路观察(core cmd_prompt 注入的 SetUserVar 标记);
        // 事件本体仍入队,旁路只是盖章。
        if matches!(&event, Event::UserVar { name, value }
            if name == "slterm_cmd_prompt" && value == "1")
        {
            self.native_prompt.observe_prompt();
        }
        let event = match event {
            Event::Frame(frame) => {
                *self.frame.0.lock().unwrap() = Some(frame);
                Event::Wakeup
            }
            event => event,
        };
        let wake = matches!(event, Event::Wakeup);
        if wake && self.wake_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        if futures::executor::block_on(self.sender.clone().send(event)).is_err() && wake {
            self.wake_pending.store(false, Ordering::Release);
            self.frame.0.lock().unwrap().take();
        }
    }
}

impl EventReceiver {
    fn latest_event(&self, event: Event) -> Event {
        if matches!(event, Event::Wakeup)
            && let Some(frame) = self.frame.0.lock().unwrap().take()
        {
            return Event::Frame(frame);
        }
        event
    }

    fn acknowledge(&self, event: &Event) {
        if matches!(event, Event::Wakeup | Event::Frame(_)) {
            // 处理前清零:UI 工作期间到达的输出必须仍能排进下一次重绘。
            self.wake_pending.store(false, Ordering::Release);
        }
    }

    pub(super) fn try_recv(&mut self) -> Result<Event, mpsc::TryRecvError> {
        let event = self.receiver.try_recv()?;
        let event = self.latest_event(event);
        self.acknowledge(&event);
        Ok(event)
    }
}

impl Stream for EventReceiver {
    type Item = Event;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Event>> {
        match Pin::new(&mut self.receiver).poll_next(cx) {
            Poll::Ready(Some(event)) => {
                let event = self.latest_event(event);
                self.acknowledge(&event);
                Poll::Ready(Some(event))
            }
            result => result,
        }
    }
}

#[cfg(test)]
mod mailbox_tests {
    use super::*;

    #[test]
    fn line_editor_controls_keep_the_prompt_but_submissions_and_interrupts_end_it() {
        let prompt = NativePromptState::default();
        prompt.observe_prompt();
        for (epoch, bytes) in [b"\x15".as_slice(), b"\x17", b"\x1b[85;22;21;1;8;1_"]
            .into_iter()
            .enumerate()
        {
            assert!(preserves_native_prompt(bytes));
            prompt.observe_input(epoch as u64, preserves_native_prompt(bytes));
            assert!(prompt.snapshot().pending);
        }
        for bytes in [b"\r".as_slice(), b"\x03", b"\x1b[13;28;0;1;0;1_"] {
            assert!(!preserves_native_prompt(bytes));
        }
        prompt.observe_input(5, preserves_native_prompt(b"\r"));
        assert!(!prompt.snapshot().pending);
    }

    /// 重绘洪流不丢语义事件:80 会话 × 1000 Wakeup 合并到一条,
    /// 其间的命令边界/UserVar 一条不少、序不乱。(上游对应用例的
    /// AiHookEnvelope 变体本仓 core 已裁剪,以 UserVar 等价顶替。)
    #[test]
    fn eighty_sessions_keep_all_semantic_events_through_a_repaint_flood() {
        let mut sessions: Vec<_> = (0..80).map(|_| channel()).collect();
        for (id, (sender, _)) in sessions.iter().enumerate() {
            sender.send(Event::CommandStart);
            for _ in 0..1000 {
                sender.send(Event::Wakeup);
            }
            for edge in ["working", "attention", "done"] {
                sender.send(Event::UserVar {
                    name: "slterm_test_edge".into(),
                    value: format!("{edge}:{id}"),
                });
            }
            sender.send(Event::CommandDone { exit_code: Some(0) });
        }
        for (id, (sender, receiver)) in sessions.iter_mut().enumerate() {
            assert!(matches!(receiver.try_recv().unwrap(), Event::CommandStart));
            assert!(matches!(receiver.try_recv().unwrap(), Event::Wakeup));
            for edge in ["working", "attention", "done"] {
                let Event::UserVar { name, value } = receiver.try_recv().unwrap() else {
                    panic!("lost semantic event")
                };
                assert_eq!(name, "slterm_test_edge");
                assert_eq!(value, format!("{edge}:{id}"));
            }
            assert!(matches!(
                receiver.try_recv().unwrap(),
                Event::CommandDone { exit_code: Some(0) }
            ));
            assert!(receiver.try_recv().is_err());
            // 消费清零唤醒位后,新 Wakeup 必须能再次入队。
            sender.send(Event::Wakeup);
            assert!(matches!(receiver.try_recv().unwrap(), Event::Wakeup));
        }
    }

    #[test]
    fn event_transport_never_uses_an_unbounded_channel() {
        let source = include_str!("event_mailbox.rs");
        for forbidden in [
            concat!("mpsc::", "unbounded"),
            concat!("Unbounded", "Sender"),
            concat!("Unbounded", "Receiver"),
        ] {
            assert!(!source.contains(forbidden), "终端事件端点必须有界");
        }
    }

    #[test]
    fn frame_events_coalesce_to_latest_without_losing_semantic_events() {
        use crate::gpui_terminal::session::GridSize;
        use slterm_terminal::event::VoidListener;
        use slterm_terminal::render::TerminalFrame;
        use slterm_terminal::term::{Config, Term};

        fn frame(revision: u64) -> Event {
            let term = Term::new(
                Config::default(),
                &GridSize {
                    columns: 80,
                    screen_lines: 24,
                },
                VoidListener,
            );
            Event::Frame(Arc::new(TerminalFrame::capture(&term, revision)))
        }

        let (sender, mut receiver) = channel();
        sender.send(frame(1));
        sender.send(frame(2));
        sender.send(Event::CommandStart);

        let Event::Frame(latest) = receiver.try_recv().unwrap() else {
            panic!("帧事件必须保留为最新不可变帧");
        };
        assert_eq!(latest.revision(), 2);
        assert!(matches!(receiver.try_recv(), Ok(Event::CommandStart)));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn sender_clones_share_wakeup_state_but_retain_exit() {
        let (sender, mut receiver) = channel();
        sender.send(Event::Wakeup);
        sender.clone().send(Event::Wakeup);
        sender.send(Event::Exit);
        assert!(matches!(receiver.try_recv().unwrap(), Event::Wakeup));
        assert!(matches!(receiver.try_recv().unwrap(), Event::Exit));
        assert!(receiver.try_recv().is_err());
    }

    /// 原生提示符旁路:slterm_cmd_prompt=1 的 UserVar 盖章 pending,
    /// 事件本体照样入队。
    #[test]
    fn cmd_prompt_user_var_marks_native_prompt_and_still_flows() {
        let (sender, mut receiver) = channel();
        sender.send(Event::UserVar {
            name: "slterm_cmd_prompt".into(),
            value: "1".into(),
        });
        let prompt = sender.native_prompt.snapshot();
        assert!(prompt.seen && prompt.pending);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            Event::UserVar { .. }
        ));
    }
}
