#[cfg(feature = "test-support")]
impl<T, U> EventLoop<T, U>
where
    T: tty::EventedPty + event::OnResize + Send + 'static,
    U: EventListener + Send + 'static,
{
    /// 测试缝：用真实 EventLoop 与通道构造句柄，但让工作线程在回收 PTY 后
    /// panic，供跨 crate 验证 `join` 的错误传播。生产构建不编译此夹具。
    pub fn spawn_panicking_for_test(self) -> EventLoopHandle<T, U> {
        let tx = self.channel();
        let join = thread::spawn_named(
            "PTY reader test panic",
            move || -> (EventLoop<T, U>, State) {
                drop(self);
                panic!("测试注入的 PTY reader panic");
            },
        );
        EventLoopHandle {
            join: Some(join),
            tx,
        }
    }
}
