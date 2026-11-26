//! Code for running a reader/writer on another thread while driving it through `polling`.

use std::collections::VecDeque;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::{io, thread};

use piper::{Writer, pipe};
use polling::os::iocp::{CompletionPacket, PollerIocpExt};
use polling::{Event, PollMode, Poller};

use crate::event_loop::READ_BUFFER_SIZE;
use crate::thread::spawn_named;

const OUTPUT_CHUNK_LIMIT: usize = 64;
const OUTPUT_BYTE_LIMIT: usize = 4 * 1024 * 1024;

struct OutputChunk {
    bytes: Vec<u8>,
    consumed: usize,
    watermark: crate::tty::OutputWatermark,
}

#[derive(Default)]
struct OutputState {
    chunks: VecDeque<OutputChunk>,
    bytes: usize,
    issued: u64,
    closed: bool,
}

/// 读取线程与解析 worker 间的双上限字节块队列。
struct OutputQueue {
    state: Mutex<OutputState>,
    changed: Condvar,
    max_chunks: usize,
    max_bytes: usize,
}

impl OutputQueue {
    fn new() -> Self {
        Self::with_limits(OUTPUT_CHUNK_LIMIT, OUTPUT_BYTE_LIMIT)
    }

    fn with_limits(max_chunks: usize, max_bytes: usize) -> Self {
        Self {
            state: Mutex::new(OutputState::default()),
            changed: Condvar::new(),
            max_chunks,
            max_bytes,
        }
    }

    fn push_locked(&self, state: &mut OutputState, bytes: Vec<u8>) -> Result<(), Vec<u8>> {
        if bytes.is_empty() {
            return Ok(());
        }
        if state.chunks.len() >= self.max_chunks || state.bytes + bytes.len() > self.max_bytes {
            return Err(bytes);
        }
        state.bytes += bytes.len();
        state.issued = state.issued.checked_add(1).expect("终端输出水位不得回绕");
        state.chunks.push_back(OutputChunk {
            bytes,
            consumed: 0,
            watermark: crate::tty::OutputWatermark::new(state.issued),
        });
        Ok(())
    }

    #[cfg(test)]
    fn try_push(&self, bytes: Vec<u8>) -> Result<(), Vec<u8>> {
        let mut state = self.state.lock().unwrap();
        self.push_locked(&mut state, bytes)?;
        self.changed.notify_all();
        Ok(())
    }

    fn push(&self, mut bytes: Vec<u8>) {
        let mut state = self.state.lock().unwrap();
        loop {
            match self.push_locked(&mut state, bytes) {
                Ok(()) => {
                    self.changed.notify_all();
                    return;
                }
                Err(returned) => bytes = returned,
            }
            state = self.changed.wait(state).unwrap();
        }
    }

    fn read_locked(
        &self,
        state: &mut OutputState,
        output: &mut [u8],
        watermark: crate::tty::OutputWatermark,
    ) -> usize {
        let mut written = 0;
        while written < output.len() {
            let Some(chunk) = state.chunks.front_mut() else {
                break;
            };
            if chunk.watermark > watermark {
                break;
            }
            let available = &chunk.bytes[chunk.consumed..];
            let count = available.len().min(output.len() - written);
            output[written..written + count].copy_from_slice(&available[..count]);
            chunk.consumed += count;
            written += count;
            let finished = chunk.consumed == chunk.bytes.len();
            if finished {
                state.chunks.pop_front();
            }
            state.bytes -= count;
        }
        written
    }

    fn try_read(&self, output: &mut [u8]) -> usize {
        let mut state = self.state.lock().unwrap();
        let read = self.read_locked(&mut state, output, crate::tty::OutputWatermark::MAX);
        if read > 0 {
            self.changed.notify_all();
        }
        read
    }

    fn try_read_before(&self, output: &mut [u8], watermark: crate::tty::OutputWatermark) -> usize {
        let mut state = self.state.lock().unwrap();
        let read = self.read_locked(&mut state, output, watermark);
        if read > 0 {
            self.changed.notify_all();
        }
        read
    }

    fn watermark(&self) -> crate::tty::OutputWatermark {
        crate::tty::OutputWatermark::new(self.state.lock().unwrap().issued)
    }

    fn read_wait(&self, output: &mut [u8]) -> usize {
        let mut state = self.state.lock().unwrap();
        loop {
            let read = self.read_locked(&mut state, output, crate::tty::OutputWatermark::MAX);
            if read > 0 {
                self.changed.notify_all();
                return read;
            }
            if state.closed {
                return 0;
            }
            state = self.changed.wait(state).unwrap();
        }
    }

    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.changed.notify_all();
    }

    fn is_closed(&self) -> bool {
        self.state.lock().unwrap().closed
    }

    fn is_empty(&self) -> bool {
        self.state.lock().unwrap().chunks.is_empty()
    }
}

struct Registration {
    interest: Mutex<Option<Interest>>,
    end: PipeEnd,
}

#[derive(Copy, Clone)]
enum PipeEnd {
    Reader,
    Writer,
}

struct Interest {
    /// The event to send about completion.
    event: Event,

    /// The poller to send the event to.
    poller: Arc<Poller>,

    /// The mode that we are in.
    mode: PollMode,
}

/// Poll a reader in another thread.
pub struct UnblockedReader<R> {
    /// The event to send about completion.
    interest: Arc<Registration>,

    /// 读取线程发布的有界输出块。
    queue: Arc<OutputQueue>,

    /// Is this the first time registering?
    first_register: bool,

    /// We logically own the reader, but we don't actually use it.
    _reader: PhantomData<R>,
}

impl<R: Read + Send + 'static> UnblockedReader<R> {
    /// Spawn a reader thread that forwards bounded byte chunks without parsing.
    pub fn new(mut source: R) -> Self {
        let queue = Arc::new(OutputQueue::new());
        let interest = Arc::new(Registration {
            interest: Mutex::<Option<Interest>>::new(None),
            end: PipeEnd::Reader,
        });

        let producer_queue = Arc::clone(&queue);
        let producer_interest = Arc::clone(&interest);
        spawn_named("slterm-tty-reader-thread", move || {
            let waker = Waker::from(producer_interest);
            let mut buf = vec![0; READ_BUFFER_SIZE];
            loop {
                match source.read(&mut buf) {
                    Ok(0) => {
                        crate::pty_trace("conout reader thread: EOF");
                        break;
                    }
                    Ok(read) => {
                        // 队列满时只阻塞专用读取线程，PTY 管道自然形成背压。
                        producer_queue.push(buf[..read].to_vec());
                        waker.wake_by_ref();
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                        crate::pty_trace("conout reader thread: broken pipe");
                        break;
                    }
                    Err(error) => {
                        log::error!("error reading from PTY: {error}");
                        crate::pty_trace("conout reader thread: read error");
                        break;
                    }
                }
            }
            producer_queue.close();
            // 关闭也必须唤醒等待方，否则 teardown 的 drain 会永久滞留。
            waker.wake_by_ref();
        });

        Self {
            interest,
            queue,
            first_register: true,
            _reader: PhantomData,
        }
    }

    /// Register interest in the reader.
    pub fn register(&mut self, poller: &Arc<Poller>, event: Event, mode: PollMode) {
        let mut interest = self.interest.interest.lock().unwrap();
        *interest = Some(Interest {
            event,
            poller: poller.clone(),
            mode,
        });

        // Send the event to start off with if we have any data.
        if (!self.queue.is_empty() && event.readable) || self.first_register {
            self.first_register = false;
            poller.post(CompletionPacket::new(event)).ok();
        }
    }

    /// Deregister interest in the reader.
    pub fn deregister(&self) {
        let mut interest = self.interest.interest.lock().unwrap();
        *interest = None;
    }

    /// Try to read from the queued chunks.
    pub fn try_read(&mut self, buf: &mut [u8]) -> usize {
        let read = self.queue.try_read(buf);
        if read > 0 {
            // `pty_read` 可在达到单次解析预算后提前停止；Level 模式必须再投
            // 一次 readable，保证静默源留下的队列尾部也会继续被消费。
            Waker::from(self.interest.clone()).wake_by_ref();
        }
        read
    }

    pub fn output_watermark(&self) -> crate::tty::OutputWatermark {
        self.queue.watermark()
    }

    /// 读取线程已结束（EOF、BrokenPipe 或读取错误）并关闭输出队列。
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    pub fn read_before(
        &mut self,
        watermark: crate::tty::OutputWatermark,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        Ok(self.queue.try_read_before(buf, watermark))
    }

    /// Hand the receiving end to a detached thread that keeps consuming until
    /// the source hits EOF. Called on teardown, right before the pseudoconsole
    /// is closed: `ClosePseudoConsole` blocks until the console host has
    /// flushed its remaining output, and once the terminal stops draining, a
    /// full pipe parks the reader thread forever — the close call deadlocks
    /// and the process lingers after its window is gone.
    pub fn drain_detached(&mut self) {
        let queue = Arc::clone(&self.queue);
        spawn_named("slterm-tty-drain-thread", move || {
            let mut sink = [0u8; 8192];
            while queue.read_wait(&mut sink) > 0 {}
            debug_assert!(queue.is_closed());
        });
    }
}

impl<R: Read + Send + 'static> Read for UnblockedReader<R> {
    /// `Ok(0)` 只表示队列暂空，不代表 EOF；消费方不得据此判定流关闭。
    /// EOF 由读取线程关闭队列表达，显式 teardown 的 drain 据此结束。
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Ok(self.try_read(buf))
    }
}

/// Poll a writer in another thread.
pub struct UnblockedWriter<W> {
    /// The interest to send about completion.
    interest: Arc<Registration>,

    /// The pipe that we are writing to.
    pipe: Writer,

    /// We logically own the writer, but we don't actually use it.
    _reader: PhantomData<W>,
}

impl<W: Write + Send + 'static> UnblockedWriter<W> {
    /// Spawn a new unblocked writer.
    pub fn new(mut sink: W, pipe_capacity: usize) -> Self {
        // Create a new pipe.
        let (mut reader, writer) = pipe(pipe_capacity);
        let interest = Arc::new(Registration {
            interest: Mutex::<Option<Interest>>::new(None),
            end: PipeEnd::Writer,
        });

        // Spawn the writer thread.
        spawn_named("slterm-tty-writer-thread", move || {
            let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
            let mut context = Context::from_waker(&waker);

            loop {
                // Write from the pipe into the writer.
                match reader.poll_drain(&mut context, &mut sink) {
                    Poll::Ready(Ok(0)) => {
                        // Either the pipe is closed or the writer is full.
                        // In any case, we are done.
                        return;
                    }

                    Poll::Ready(Ok(_)) => {
                        // Keep writing.
                        continue;
                    }

                    Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::Interrupted => {
                        // We were interrupted; continue.
                        continue;
                    }

                    Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::BrokenPipe => return,

                    Poll::Ready(Err(e)) => {
                        log::error!("error writing to pipe: {}", e);
                        return;
                    }

                    Poll::Pending => {
                        // We are now waiting on the other end to advance. Park the
                        // thread until they do.
                        thread::park();
                    }
                }
            }
        });

        Self {
            interest,
            pipe: writer,
            _reader: PhantomData,
        }
    }

    /// Register interest in the writer.
    pub fn register(&self, poller: &Arc<Poller>, event: Event, mode: PollMode) {
        let mut interest = self.interest.interest.lock().unwrap();
        *interest = Some(Interest {
            event,
            poller: poller.clone(),
            mode,
        });

        // Send the event to start off with if we have room for data.
        if !self.pipe.is_full() && event.writable {
            poller.post(CompletionPacket::new(event)).ok();
        }
    }

    /// Deregister interest in the writer.
    pub fn deregister(&self) {
        let mut interest = self.interest.interest.lock().unwrap();
        *interest = None;
    }

    /// Try to write to the writer.
    pub fn try_write(&mut self, buf: &[u8]) -> usize {
        let waker = Waker::from(self.interest.clone());

        match self
            .pipe
            .poll_fill_bytes(&mut Context::from_waker(&waker), buf)
        {
            Poll::Pending => 0,
            Poll::Ready(n) => n,
        }
    }
}

impl<W: Write + Send + 'static> Write for UnblockedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(self.try_write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        // Nothing to flush.
        Ok(())
    }
}

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

impl Wake for Registration {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let mut interest_lock = self.interest.lock().unwrap();
        if let Some(interest) = interest_lock.as_ref() {
            // Send the event to the poller.
            let send_event = match self.end {
                PipeEnd::Reader => interest.event.readable,
                PipeEnd::Writer => interest.event.writable,
            };

            if send_event {
                interest
                    .poller
                    .post(CompletionPacket::new(interest.event))
                    .ok();

                // Clear the event if we're in oneshot mode.
                if matches!(interest.mode, PollMode::Oneshot | PollMode::EdgeOneshot) {
                    *interest_lock = None;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use polling::Events;

    use super::*;

    #[test]
    fn output_queue_enforces_byte_limit_without_losing_order() {
        let queue = OutputQueue::with_limits(4, 5);
        queue.try_push(b"abc".to_vec()).unwrap();
        assert_eq!(queue.try_push(b"def".to_vec()), Err(b"def".to_vec()));
        queue.try_push(b"d".to_vec()).unwrap();

        let mut output = [0; 2];
        assert_eq!(queue.try_read(&mut output), 2);
        assert_eq!(&output, b"ab");
        queue.try_push(b"efg".to_vec()).unwrap();

        let mut rest = Vec::new();
        let mut buf = [0; 3];
        loop {
            let read = queue.try_read(&mut buf);
            if read == 0 {
                break;
            }
            rest.extend_from_slice(&buf[..read]);
        }
        assert_eq!(rest, b"cdefg");
    }

    /// EOF/读取错误关闭队列后，事件循环必须能观察到传输已关闭。
    #[test]
    fn reader_reports_output_closed_after_eof() {
        let mut reader = UnblockedReader::new(std::io::empty());
        let start = Instant::now();
        while !reader.is_closed() {
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "EOF 后读取端必须报告输出已关闭"
            );
            std::thread::yield_now();
        }
        let mut buf = [0u8; 8];
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn output_queue_enforces_chunk_limit_independently() {
        let queue = OutputQueue::with_limits(2, 1024);
        queue.try_push(b"a".to_vec()).unwrap();
        queue.try_push(b"b".to_vec()).unwrap();
        assert_eq!(queue.try_push(b"c".to_vec()), Err(b"c".to_vec()));

        let mut output = [0; 1];
        assert_eq!(queue.try_read(&mut output), 1);
        assert_eq!(&output, b"a");
        queue.try_push(b"c".to_vec()).unwrap();

        let mut rest = [0; 2];
        assert_eq!(queue.try_read(&mut rest), 2);
        assert_eq!(&rest, b"bc");
    }

    #[test]
    fn output_queue_watermark_leaves_later_output_for_the_new_geometry() {
        let queue = OutputQueue::with_limits(2, 1024);
        queue.try_push(b"old".to_vec()).unwrap();
        let watermark = queue.watermark();
        queue.try_push(b"new".to_vec()).unwrap();

        let mut output = [0; 6];
        assert_eq!(queue.try_read_before(&mut output, watermark), 3);
        assert_eq!(&output[..3], b"old");
        assert_eq!(queue.try_read(&mut output), 3);
        assert_eq!(&output[..3], b"new");
    }

    #[test]
    fn output_queue_push_waits_for_byte_space() {
        let queue = Arc::new(OutputQueue::with_limits(2, 4));
        queue.try_push(b"full".to_vec()).unwrap();
        let producer = Arc::clone(&queue);
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            producer.push(b"!".to_vec());
            done_tx.send(()).unwrap();
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());

        let mut output = [0; 1];
        assert_eq!(queue.try_read(&mut output), 1);
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let mut rest = [0; 4];
        assert_eq!(queue.try_read(&mut rest), 4);
        assert_eq!(&rest, b"ull!");
    }

    #[test]
    fn output_queue_close_wakes_a_waiting_drainer() {
        let queue = Arc::new(OutputQueue::with_limits(1, 4));
        let drainer = Arc::clone(&queue);
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let mut output = [0; 4];
            assert_eq!(drainer.read_wait(&mut output), 0);
            assert!(drainer.is_closed());
            done_tx.send(()).unwrap();
        });
        started_rx.recv().unwrap();
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        queue.close();
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }

    /// 「给一批就静默」的源：`recv` 阻塞住读线程，精确复刻 AI CLI 打完一轮
    /// 就不再输出的时序。源不静默的话总有下一次写入替我们投递事件，缺唤醒
    /// 也就显不出来——静默是这个 bug 的必要条件。
    struct ScriptedSource {
        rx: mpsc::Receiver<Vec<u8>>,
        pending: Vec<u8>,
    }

    impl Read for ScriptedSource {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pending.is_empty() {
                match self.rx.recv() {
                    Ok(chunk) => self.pending = chunk,
                    // 发送端关闭 = EOF，读线程正常收尾。
                    Err(_) => return Ok(0),
                }
            }
            let n = self.pending.len().min(buf.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            Ok(n)
        }
    }

    fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            if cond() {
                return true;
            }
            thread::sleep(Duration::from_millis(1));
        }
        cond()
    }

    fn readable_arrived(poller: &Poller, key: usize) -> bool {
        let mut events = Events::new();
        poller
            .wait(&mut events, Some(Duration::from_secs(2)))
            .unwrap();
        events
            .iter()
            .any(|event| event.key == key && event.readable)
    }

    /// 队列读空后仍补投 readable，消费方达到单次解析预算时不会遗留静默尾部。
    #[test]
    fn reposts_readable_after_draining_the_queue() {
        const KEY: usize = 7;

        let (tx, rx) = mpsc::channel();
        let poller = Arc::new(Poller::new().unwrap());
        let event = Event::readable(KEY);
        let mut reader = UnblockedReader::new(ScriptedSource {
            rx,
            pending: Vec::new(),
        });

        // Readiness 只表示可以再 poll；过期通知可能晚于对应数据，重试必须先等通知。
        // Retry only after a notification so a genuinely lost wake still fails.
        let read_chunk = |reader: &mut UnblockedReader<ScriptedSource>, buf: &mut [u8]| {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let read = reader.try_read(buf);
                if read > 0 {
                    return read;
                }
                assert!(Instant::now() < deadline, "readiness never produced data");
                assert!(readable_arrived(&poller, KEY), "pending read was not woken");
            }
        };

        // 首次 register 自带一次无条件投递；先消化掉，后面等到的事件就只可能
        // 来自 try_read 的补投。
        reader.register(&poller, event, PollMode::Level);
        assert!(readable_arrived(&poller, KEY), "首次 register 应当自投一次");

        tx.send(b"hello".to_vec()).unwrap();
        assert!(
            wait_until(Duration::from_secs(2), || !reader.queue.is_empty()),
            "读线程没有把数据搬进队列"
        );

        let mut buf = [0u8; 64];
        assert_eq!(read_chunk(&mut reader, &mut buf), 5);
        assert_eq!(&buf[..5], b"hello");
        assert!(
            reader.queue.is_empty(),
            "这一读须把队列读空，验证补投自收敛"
        );

        assert!(
            readable_arrived(&poller, KEY),
            "读空队列后没有补投 readable：源一静默，尾部可能永久滞留"
        );

        // 空读不再补投；后续生产仍直接经 Registration 唤醒。
        assert_eq!(reader.try_read(&mut buf), 0);
        tx.send(b"world".to_vec()).unwrap();
        assert!(
            readable_arrived(&poller, KEY),
            "重新注册的 waker 失效，后续输出会卡住"
        );
        assert_eq!(read_chunk(&mut reader, &mut buf), 5);
        assert_eq!(&buf[..5], b"world");
    }
}
