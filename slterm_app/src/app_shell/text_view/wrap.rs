//! Rope 的软换行显示行索引。

use std::ops::Range;

use ropey::{LineType, Rope};
use slterm_document::TextStateId;

use super::{
    metrics::DisplayMap,
    source::{RopeTextSource, VisibleLine},
};

const WRAP_CHECKPOINT_STRIDE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DisplayPosition {
    pub(super) row: usize,
    pub(super) column: usize,
}

/// 共享折行边界的光标亲和性；不进入持久 ViewState。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CaretAffinity {
    BeforeWrap,
    #[default]
    AfterWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AffinedByte {
    pub(super) byte: usize,
    pub(super) affinity: CaretAffinity,
}

#[derive(Clone)]
struct WrapSegment {
    index: usize,
    bytes: Range<usize>,
    display_start: usize,
}

#[derive(Clone, Copy)]
struct WrapCheckpoint {
    segment: usize,
    byte: usize,
    display_start: usize,
}

struct WrappedLine {
    bytes: Range<usize>,
    display_start: usize,
    segments: usize,
    checkpoints: Vec<WrapCheckpoint>,
}

/// 全文只保存逻辑行与稀疏段检查点；单线查询最多续扫一个检查点窗口。
pub(super) struct SoftWrapIndex {
    text: Rope,
    text_state: TextStateId,
    wrap_columns: usize,
    map: DisplayMap,
    lines: Vec<WrappedLine>,
    display_rows: usize,
    #[cfg(test)]
    scan_steps: std::cell::Cell<usize>,
}

impl SoftWrapIndex {
    pub(super) fn new(text: &Rope, text_state: TextStateId, wrap_columns: usize) -> Self {
        let map = DisplayMap::new(4);
        let source = RopeTextSource::new(text, map);
        let mut lines = Vec::with_capacity(text.len_lines(LineType::LF));
        let mut display_rows = 0;
        for index in 0..text.len_lines(LineType::LF) {
            let bytes = source
                .line_byte_range(index)
                .expect("索引范围来自 Rope 真实逻辑行");
            let mut checkpoints = Vec::new();
            let mut segments = 0;
            map.visit_wrap_segments_from(
                text.slice(bytes.clone()),
                wrap_columns,
                0,
                0,
                |range, display_start| {
                    if segments % WRAP_CHECKPOINT_STRIDE == 0 {
                        checkpoints.push(WrapCheckpoint {
                            segment: segments,
                            byte: range.start,
                            display_start,
                        });
                    }
                    segments += 1;
                    true
                },
            );
            lines.push(WrappedLine {
                bytes,
                display_start: display_rows,
                segments,
                checkpoints,
            });
            display_rows = display_rows
                .checked_add(segments)
                .expect("显示行数不得耗尽");
        }
        Self {
            text: text.clone(),
            text_state,
            wrap_columns,
            map,
            lines,
            display_rows,
            #[cfg(test)]
            scan_steps: std::cell::Cell::new(0),
        }
    }

    pub(super) fn matches(&self, text_state: TextStateId, wrap_columns: usize) -> bool {
        self.text_state == text_state && self.wrap_columns == wrap_columns
    }

    pub(super) const fn display_row_count(&self) -> usize {
        self.display_rows
    }

    /// 显示行 → 逻辑行；行超出当前索引时返回 `None`。
    pub(super) fn logical_line_at_display_row(&self, row: usize) -> Option<usize> {
        if row >= self.display_rows {
            return None;
        }
        self.lines
            .partition_point(|line| line.display_start <= row)
            .checked_sub(1)
    }

    #[cfg(test)]
    fn record_scan(&self) {
        self.scan_steps.set(self.scan_steps.get() + 1);
    }

    #[cfg(not(test))]
    fn record_scan(&self) {}

    #[cfg(test)]
    pub(super) fn take_scan_steps_for_test(&self) -> usize {
        self.scan_steps.take()
    }

    fn segment_at(&self, logical: usize, target: usize) -> Option<WrapSegment> {
        let line = self.lines.get(logical)?;
        if target >= line.segments {
            return None;
        }
        let checkpoint = line.checkpoints[target / WRAP_CHECKPOINT_STRIDE];
        let content = self.text.slice(line.bytes.clone());
        let mut current = checkpoint.segment;
        let mut found = None;
        self.map.visit_wrap_segments_from(
            content,
            self.wrap_columns,
            checkpoint.byte,
            checkpoint.display_start,
            |bytes, display_start| {
                self.record_scan();
                if current == target {
                    found = Some(WrapSegment {
                        index: current,
                        bytes,
                        display_start,
                    });
                    return false;
                }
                current += 1;
                true
            },
        );
        found
    }

    fn location_with_affinity(
        &self,
        byte: usize,
        affinity: CaretAffinity,
    ) -> Option<(usize, WrapSegment)> {
        if byte > self.text.len() {
            return None;
        }
        let logical = self.text.byte_to_line_idx(byte, LineType::LF);
        let line = self.lines.get(logical)?;
        let local = byte.checked_sub(line.bytes.start)?;
        if local > line.bytes.len() {
            return None;
        }
        let checkpoint_index = line
            .checkpoints
            .partition_point(|checkpoint| checkpoint.byte <= local)
            .checked_sub(1)?;
        let checkpoint = line.checkpoints[checkpoint_index];
        if affinity == CaretAffinity::BeforeWrap
            && local == checkpoint.byte
            && checkpoint.segment > 0
        {
            let segment = self.segment_at(logical, checkpoint.segment - 1)?;
            return Some((logical, segment));
        }
        let content = self.text.slice(line.bytes.clone());
        let mut index = checkpoint.segment;
        let mut previous = None;
        let mut found = None;
        self.map.visit_wrap_segments_from(
            content,
            self.wrap_columns,
            checkpoint.byte,
            checkpoint.display_start,
            |bytes, display_start| {
                self.record_scan();
                let segment = WrapSegment {
                    index,
                    bytes,
                    display_start,
                };
                if affinity == CaretAffinity::BeforeWrap
                    && local == segment.bytes.start
                    && previous.is_some()
                {
                    found = previous.take();
                    return false;
                }
                let last = index + 1 == line.segments;
                if local < segment.bytes.end || (last && local == segment.bytes.end) {
                    found = Some(segment);
                    return false;
                }
                previous = Some(segment);
                index += 1;
                true
            },
        );
        found.map(|segment| (logical, segment))
    }

    #[cfg(test)]
    pub(super) fn position(&self, byte: usize) -> Option<DisplayPosition> {
        self.position_with_affinity(byte, CaretAffinity::AfterWrap)
    }

    pub(super) fn position_with_affinity(
        &self,
        byte: usize,
        affinity: CaretAffinity,
    ) -> Option<DisplayPosition> {
        let (logical, segment) = self.location_with_affinity(byte, affinity)?;
        let line = &self.lines[logical];
        let content = self
            .text
            .slice(line.bytes.clone())
            .slice(segment.bytes.clone());
        let local = byte - line.bytes.start - segment.bytes.start;
        let column = self
            .map
            .column_at_rope_from(content, local, segment.display_start)?
            - segment.display_start;
        Some(DisplayPosition {
            row: line.display_start + segment.index,
            column,
        })
    }

    fn line_for_row(&self, row: usize) -> Option<usize> {
        if row >= self.display_rows {
            return None;
        }
        let index = self
            .lines
            .partition_point(|line| line.display_start <= row)
            .checked_sub(1)?;
        let line = &self.lines[index];
        (row < line.display_start + line.segments).then_some(index)
    }

    pub(super) fn byte_at(&self, position: DisplayPosition) -> Option<usize> {
        Some(self.byte_at_with_affinity(position)?.byte)
    }

    pub(super) fn byte_at_with_affinity(&self, position: DisplayPosition) -> Option<AffinedByte> {
        let logical = self.line_for_row(position.row)?;
        let line = &self.lines[logical];
        let segment = self.segment_at(logical, position.row - line.display_start)?;
        let content = self
            .text
            .slice(line.bytes.clone())
            .slice(segment.bytes.clone())
            .to_string();
        let target = segment.display_start.saturating_add(position.column);
        let local = self
            .map
            .byte_at_column_from(&content, target, segment.display_start)?;
        let affinity = if local == segment.bytes.len() && segment.index + 1 < line.segments {
            CaretAffinity::BeforeWrap
        } else {
            CaretAffinity::AfterWrap
        };
        Some(AffinedByte {
            byte: line.bytes.start + segment.bytes.start + local,
            affinity,
        })
    }

    #[cfg(test)]
    pub(super) fn line_boundary(&self, byte: usize, end: bool) -> Option<usize> {
        Some(self.line_boundary_with_affinity(byte, end)?.byte)
    }

    #[cfg(test)]
    pub(super) fn line_boundary_with_affinity(
        &self,
        byte: usize,
        end: bool,
    ) -> Option<AffinedByte> {
        self.line_boundary_at(byte, end, CaretAffinity::AfterWrap)
    }

    pub(super) fn line_boundary_at(
        &self,
        byte: usize,
        end: bool,
        affinity: CaretAffinity,
    ) -> Option<AffinedByte> {
        let (logical, segment) = self.location_with_affinity(byte, affinity)?;
        let line = &self.lines[logical];
        Some(if end {
            AffinedByte {
                byte: line.bytes.start + segment.bytes.end,
                affinity: if segment.index + 1 < line.segments {
                    CaretAffinity::BeforeWrap
                } else {
                    CaretAffinity::AfterWrap
                },
            }
        } else {
            AffinedByte {
                byte: line.bytes.start + segment.bytes.start,
                affinity: CaretAffinity::AfterWrap,
            }
        })
    }

    /// 按目标显示段直接切片；不得从逻辑行首段扫描到可见窗口。
    pub(super) fn visible_lines(&self, rows: Range<usize>) -> Vec<VisibleLine> {
        if rows.start >= rows.end || rows.start >= self.display_rows {
            return Vec::new();
        }
        let end = rows.end.min(self.display_rows);
        let Some(mut logical) = self.line_for_row(rows.start) else {
            return Vec::new();
        };
        let mut visible = Vec::new();
        while logical < self.lines.len() {
            let line = &self.lines[logical];
            if line.display_start >= end {
                break;
            }
            let first = rows
                .start
                .saturating_sub(line.display_start)
                .min(line.segments);
            let last = end.saturating_sub(line.display_start).min(line.segments);
            for segment_index in first..last {
                let Some(segment) = self.segment_at(logical, segment_index) else {
                    continue;
                };
                let content = self
                    .text
                    .slice(line.bytes.clone())
                    .slice(segment.bytes.clone())
                    .to_string();
                let bytes =
                    line.bytes.start + segment.bytes.start..line.bytes.start + segment.bytes.end;
                let width =
                    self.map.width_from(&content, segment.display_start) - segment.display_start;
                visible.push(VisibleLine::new(
                    logical,
                    bytes,
                    segment.display_start,
                    content,
                    width,
                ));
            }
            logical += 1;
        }
        visible
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;
    use slterm_document::TextStateId;

    use super::super::metrics::DisplayMap;
    use super::{CaretAffinity, DisplayPosition, SoftWrapIndex};

    #[test]
    fn soft_wrap_queries_use_bounded_checkpoint_windows() {
        let text = Rope::from_str(&"abcd".repeat(300));
        let state = TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
        let index = SoftWrapIndex::new(&text, state, 3);
        let last = index.display_row_count() - 1;

        assert!(index.position(1_199).is_some());
        assert!(index.take_scan_steps_for_test() <= 64);
        assert_eq!(index.visible_lines(last..last + 1).len(), 1);
        assert!(index.take_scan_steps_for_test() <= 64);
    }

    #[test]
    fn soft_wrap_checkpoints_match_full_scan_across_unicode_chunks() {
        let text = Rope::from_str(&"e\u{301}🇨🇳👩\u{200d}💻X".repeat(220));
        assert!(text.chunks().count() > 1, "夹具必须跨 Rope chunk");
        let state = TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
        let index = SoftWrapIndex::new(&text, state, 7);
        let map = DisplayMap::new(4);
        let expected = map.wrap_segments(text.slice(..), 7);
        let actual = index.visible_lines(0..index.display_row_count());
        assert_eq!(actual.len(), expected.len());
        for (line, (range, display_start)) in actual.iter().zip(expected.iter()) {
            assert_eq!(line.byte_range(), *range);
            assert_eq!(line.display_start(), *display_start);
        }
        for row in [0, 63, 64, 65, expected.len() - 1] {
            let (range, display_start) = expected[row].clone();
            let width = map.width_from(&text.slice(range.clone()).to_string(), display_start)
                - display_start;
            let target = index
                .byte_at_with_affinity(DisplayPosition { row, column: width })
                .unwrap();
            assert_eq!(
                index.position_with_affinity(target.byte, target.affinity),
                Some(DisplayPosition { row, column: width })
            );
        }
    }

    #[test]
    fn soft_wrap_caret_affinity_roundtrips_nonfinal_segment_end() {
        let text = Rope::from_str("abcdef");
        let state = TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
        let index = SoftWrapIndex::new(&text, state, 3);

        let target = index
            .byte_at_with_affinity(DisplayPosition { row: 0, column: 3 })
            .unwrap();
        assert_eq!(target.byte, 3);
        assert_eq!(target.affinity, CaretAffinity::BeforeWrap);
        assert_eq!(
            index.position_with_affinity(target.byte, target.affinity),
            Some(DisplayPosition { row: 0, column: 3 })
        );
        assert_eq!(
            index.position(target.byte),
            Some(DisplayPosition { row: 1, column: 0 })
        );
        assert_eq!(index.line_boundary_with_affinity(1, true), Some(target));
    }

    #[test]
    fn soft_wrap_index_maps_unicode_display_rows_and_bytes() {
        let text = Rope::from_str("A中B😀C\nx\n");
        let state = TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
        let index = SoftWrapIndex::new(&text, state, 3);

        assert_eq!(index.display_row_count(), 5);
        assert!(index.matches(state, 3));
        assert!(!index.matches(state, 4));
        assert_eq!(
            index.position(4),
            Some(DisplayPosition { row: 1, column: 0 })
        );
        assert_eq!(
            index.position(9),
            Some(DisplayPosition { row: 2, column: 0 })
        );
        assert_eq!(
            index.byte_at(DisplayPosition { row: 1, column: 1 }),
            Some(5)
        );
        assert_eq!(
            index.byte_at(DisplayPosition { row: 3, column: 1 }),
            Some(12)
        );
        assert_eq!(
            index.byte_at(DisplayPosition { row: 4, column: 0 }),
            Some(13)
        );
        assert_eq!(index.line_boundary(4, false), Some(4));
        assert_eq!(index.line_boundary(4, true), Some(9));
        let lines = index.visible_lines(0..5);
        assert_eq!(
            lines.iter().map(|line| line.text()).collect::<Vec<_>>(),
            ["A中", "B😀", "C", "x", ""]
        );
        assert_eq!(lines[1].byte_range(), 4..9);
        assert_eq!(lines[1].display_start(), 3);
    }
}
