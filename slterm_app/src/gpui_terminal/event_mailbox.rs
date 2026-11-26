//! 事件信箱:入队前合并重绘唤醒(Wakeup),语义事件一条不丢。
//!
//! 因果链:PTY 输出爆发期每字节都可能触发 Wakeup,若全量入队,UI 线程会被
//! 重绘请求淹没;Wakeup 语义本就是「有输出待渲染」的布尔事实,合并到一条不
//! 丢信息。其余事件(Exit/Title/CommandStart…)携带语义,必须全保留。
//! 唤醒位与最新帧由同一把锁原子交换：接收端取帧时清除标记，UI 处理期间
//! 到达的新输出仍能再入队一次重绘，哪怕落在非活动 tab 上。

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::channel::mpsc;
use futures::{SinkExt as _, Stream};
use slterm_terminal::event::Event;
use slterm_terminal::render::TerminalFrame;

/// 两个最大事件批次的容量；worker 满载时等待，不丢弃语义事件。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 最新帧与待唤醒标记必须由同一把锁保护：接收端取帧时一并清除标记，
/// 不会留下「帧已更新但唤醒已被旧标记吃掉」的窗口。
#[derive(Clone, Default)]
struct WakeState(Arc<Mutex<WakeStateInner>>);

#[derive(Default)]
struct WakeStateInner {
    wake_pending: bool,
    frame: Option<Arc<TerminalFrame>>,
}

impl WakeState {
    /// 保存最新帧；返回 true 表示需要向通道投递一条 Wakeup。
    fn enqueue_frame(&self, frame: Arc<TerminalFrame>) -> bool {
        let mut state = self.0.lock().unwrap();
        state.frame = Some(frame);
        !std::mem::replace(&mut state.wake_pending, true)
    }

    /// 无帧刷新；返回 true 表示需要向通道投递一条 Wakeup。
    fn enqueue_wakeup(&self) -> bool {
        let mut state = self.0.lock().unwrap();
        !std::mem::replace(&mut state.wake_pending, true)
    }

    /// 取最新帧并同时清除唤醒标记，允许后续新帧重新入队。
    fn take_wakeup(&self) -> Option<Arc<TerminalFrame>> {
        let mut state = self.0.lock().unwrap();
        state.wake_pending = false;
        state.frame.take()
    }

    /// 通道断连时撤销本次唤醒；接收端已不存在，只保持状态自洽。
    fn cancel_wakeup(&self) {
        let mut state = self.0.lock().unwrap();
        state.wake_pending = false;
        state.frame = None;
    }
}

#[derive(Clone)]
pub(super) struct EventSender {
    sender: mpsc::Sender<Event>,
    wake: WakeState,
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
    wake: WakeState,
}

pub(super) fn channel() -> (EventSender, EventReceiver) {
    let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
    let wake = WakeState::default();
    (
        EventSender {
            sender,
            wake: wake.clone(),
            native_prompt: NativePromptState::default(),
        },
        EventReceiver { receiver, wake },
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
                if !self.wake.enqueue_frame(frame) {
                    return;
                }
                Event::Wakeup
            }
            Event::Wakeup => {
                if !self.wake.enqueue_wakeup() {
                    return;
                }
                Event::Wakeup
            }
            event => event,
        };
        let wake = matches!(event, Event::Wakeup);
        if futures::executor::block_on(self.sender.clone().send(event)).is_err() && wake {
            self.wake.cancel_wakeup();
        }
    }
}

impl EventReceiver {
    fn latest_event(&self, event: Event) -> Event {
        if matches!(event, Event::Wakeup) {
            return match self.wake.take_wakeup() {
                Some(frame) => Event::Frame(frame),
                None => Event::Wakeup,
            };
        }
        event
    }

    /// 旧两阶段接口的兼容占位：取帧时已原子清除唤醒位。
    #[cfg(test)]
    fn acknowledge(&self, _event: &Event) {}

    pub(super) fn try_recv(&mut self) -> Result<Event, mpsc::TryRecvError> {
        let event = self.receiver.try_recv()?;
        Ok(self.latest_event(event))
    }
}

impl Stream for EventReceiver {
    type Item = Event;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Event>> {
        match Pin::new(&mut self.receiver).poll_next(cx) {
            Poll::Ready(Some(event)) => Poll::Ready(Some(self.latest_event(event))),
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

    /// 取帧与清唤醒位之间的新帧不得被已存在的唤醒标记吞掉。
    #[test]
    fn frame_written_between_take_and_ack_is_not_lost() {
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
        sender.send(Event::Wakeup);
        let queued = receiver.receiver.try_recv().expect("唤醒事件必须已入队");
        let event = receiver.latest_event(queued);

        // 旧实现正是在这两步之间漏帧：新帧写入时旧唤醒位仍为真，
        // 不再投递新 Wakeup，随后 acknowledge 把唤醒位清零。
        sender.send(frame(41));
        receiver.acknowledge(&event);

        let Event::Frame(latest) = receiver.try_recv().expect("新帧必须仍可被消费") else {
            panic!("新帧不得被吞掉");
        };
        assert_eq!(latest.revision(), 41);
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
