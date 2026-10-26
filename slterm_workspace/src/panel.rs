use std::fmt;

/// 仅标识当前进程内存活面板的身份，不进入持久布局或恢复材料。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PanelId {
    instance_epoch: u64,
    sequence: u64,
}

impl PanelId {
    /// 创建该面板的进程实例纪元。
    pub const fn instance_epoch(self) -> u64 {
        self.instance_epoch
    }

    /// 实例内单调递增且不复用的序列。
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// 面板序列已耗尽，禁止回绕后复用旧身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelIdExhausted;

/// 单前台所有者使用的 PanelId 分配器；实例纪元由 application 提供。
///
/// 分配器不可复制，避免同一纪元复制出重复序列：
/// ```compile_fail
/// use slterm_workspace::PanelIdAllocator;
/// let allocator = PanelIdAllocator::new(1);
/// let _duplicate = allocator.clone();
/// ```
#[derive(Debug)]
pub struct PanelIdAllocator {
    instance_epoch: u64,
    next_sequence: Option<u64>,
}

impl PanelIdAllocator {
    pub fn new(instance_epoch: u64) -> Self {
        Self {
            instance_epoch,
            next_sequence: Some(1),
        }
    }

    /// 分配新身份；关闭面板不回收序列。
    pub fn allocate(&mut self) -> Result<PanelId, PanelIdExhausted> {
        let sequence = self.next_sequence.ok_or(PanelIdExhausted)?;
        self.next_sequence = sequence.checked_add(1);
        Ok(PanelId {
            instance_epoch: self.instance_epoch,
            sequence,
        })
    }
}

impl fmt::Display for PanelIdExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("面板 ID 序列已耗尽")
    }
}

impl std::error::Error for PanelIdExhausted {}

#[cfg(test)]
mod panel_tests {
    use super::*;

    /// 面板身份由实例纪元与实例内单调序列共同组成。
    #[test]
    fn panel_ids_combine_instance_epoch_and_monotonic_sequence() {
        let mut allocator = PanelIdAllocator::new(42);
        let first = allocator.allocate().unwrap();
        let second = allocator.allocate().unwrap();
        assert_eq!(first.instance_epoch(), 42);
        assert_eq!(second.instance_epoch(), 42);
        assert_eq!(first.sequence(), 1);
        assert_eq!(second.sequence(), 2);
        assert_ne!(first, second);

        let mut next_instance = PanelIdAllocator::new(43);
        let next_instance_first = next_instance.allocate().unwrap();
        assert_eq!(next_instance_first.sequence(), 1);
        assert_ne!(first, next_instance_first);
    }

    /// 序列到顶后允许最后一个值，随后永久拒绝而不是回绕复用。
    #[test]
    fn panel_id_allocator_never_wraps_or_reuses_sequences() {
        let mut allocator = PanelIdAllocator::new(7);
        let first = allocator.allocate().unwrap();
        allocator.next_sequence = Some(u64::MAX);
        let last = allocator.allocate().unwrap();
        assert_eq!(last.sequence(), u64::MAX);
        assert_ne!(first, last);
        assert_eq!(allocator.allocate(), Err(PanelIdExhausted));
        assert_eq!(allocator.allocate(), Err(PanelIdExhausted));
    }

    /// 耗尽错误实现标准接口并提供稳定说明。
    #[test]
    fn panel_id_exhaustion_describes_the_failure() {
        fn require_error<T: std::error::Error>() {}
        require_error::<PanelIdExhausted>();
        assert_eq!(PanelIdExhausted.to_string(), "面板 ID 序列已耗尽");
    }
}
