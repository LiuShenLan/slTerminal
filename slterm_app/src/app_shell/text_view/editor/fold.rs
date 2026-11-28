//! 代码折叠：当前语言 fold query 产出的逻辑行区间与显示行过滤。
//!
//! 折叠只是视图瞬态：不写 Document、不进入 ViewState。正文变化立即清空
//! 折叠集合，等新语法树到达后按仍有效的起始行重新建立可折叠区域。

use std::collections::BTreeSet;
use std::ops::Range;

use ropey::{LineType, Rope};
use slterm_document::FoldRange;

use super::super::wrap::{CaretAffinity, SoftWrapIndex};

/// 可折叠区域按逻辑行闭区间表示；起始行始终保持可见。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FoldRegion {
    pub(super) start_line: usize,
    pub(super) end_line: usize,
}

/// 折叠集合与区域缓存；两者都只在运行期存在。
#[derive(Default)]
pub(super) struct FoldState {
    regions: Vec<FoldRegion>,
    folded: BTreeSet<usize>,
}

impl FoldState {
    pub(super) fn regions(&self) -> &[FoldRegion] {
        &self.regions
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn folded_starts(&self) -> &BTreeSet<usize> {
        &self.folded
    }

    pub(super) fn is_folded(&self, start_line: usize) -> bool {
        self.folded.contains(&start_line)
    }

    /// 新语法树到达时只保留仍存在的起始行；消失区域自动展开。
    pub(super) fn set_regions(&mut self, regions: Vec<FoldRegion>) {
        self.regions = regions;
        self.folded.retain(|start| {
            self.regions
                .iter()
                .any(|region| region.start_line == *start)
        });
    }

    /// 正文变化后旧行号与旧树同时失效，保守地展开全部区域。
    pub(super) fn clear_for_change(&mut self) {
        self.regions.clear();
        self.folded.clear();
    }

    /// 光标所在行优先折叠其起始行；否则选择包住该行的最内层区域。
    pub(super) fn toggle(&mut self, line: usize) -> Option<usize> {
        let region = self
            .regions
            .iter()
            .find(|region| region.start_line == line)
            .or_else(|| {
                self.regions
                    .iter()
                    .filter(|region| region.start_line < line && line <= region.end_line)
                    .max_by_key(|region| region.start_line)
            })?;
        let start = region.start_line;
        if self.folded.contains(&start) {
            self.folded.remove(&start);
        } else {
            self.folded.insert(start);
        }
        Some(start)
    }

    pub(super) fn unfold_line(&mut self, line: usize) -> bool {
        let starts = self
            .regions
            .iter()
            .filter(|region| region.start_line < line && line <= region.end_line)
            .map(|region| region.start_line)
            .collect::<Vec<_>>();
        let mut changed = false;
        for start in starts {
            changed |= self.folded.remove(&start);
        }
        changed
    }

    /// 折叠隐藏的原始行区间；软换行下换算为显示行，非换行下即逻辑行。
    pub(super) fn hidden_ranges(
        &self,
        wrap: bool,
        index: Option<&SoftWrapIndex>,
        text: &Rope,
        total_raw_rows: usize,
    ) -> Vec<Range<usize>> {
        let mut hidden = Vec::new();
        for start in &self.folded {
            let Some(region) = self
                .regions
                .iter()
                .find(|region| region.start_line == *start)
            else {
                continue;
            };
            let range = if wrap {
                let Some(index) = index else {
                    continue;
                };
                let first_hidden_line = start.saturating_add(1);
                let hidden_end_line = region.end_line.saturating_add(1);
                let Some(first_byte) = line_byte(text, first_hidden_line) else {
                    continue;
                };
                let Some(first_row) = index
                    .position_with_affinity(first_byte, CaretAffinity::AfterWrap)
                    .map(|position| position.row)
                else {
                    continue;
                };
                let end_row = line_byte(text, hidden_end_line)
                    .and_then(|byte| {
                        index
                            .position_with_affinity(byte, CaretAffinity::AfterWrap)
                            .map(|position| position.row)
                    })
                    .unwrap_or(total_raw_rows);
                first_row.min(total_raw_rows)..end_row.min(total_raw_rows)
            } else {
                start.saturating_add(1).min(total_raw_rows)
                    ..region.end_line.saturating_add(1).min(total_raw_rows)
            };
            if range.start < range.end {
                hidden.push(range);
            }
        }
        hidden.sort_by_key(|range| range.start);
        merge_ranges(hidden)
    }
}

fn line_byte(text: &Rope, line: usize) -> Option<usize> {
    (line < text.len_lines(LineType::LF)).then(|| text.line_to_byte_idx(line, LineType::LF))
}

pub(super) fn merge_ranges(ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// fold query 的字节区间折算为可折叠逻辑行区间；同起始行保留最大范围。
pub(super) fn regions_from_ranges(text: &Rope, ranges: &[FoldRange]) -> Vec<FoldRegion> {
    let lines = text.len_lines(LineType::LF);
    let mut regions = Vec::new();
    for range in ranges {
        if range.start_byte() >= range.end_byte() || range.end_byte() > text.len() {
            continue;
        }
        let start_line = text.byte_to_line_idx(range.start_byte(), LineType::LF);
        let end_line = text.byte_to_line_idx(range.end_byte(), LineType::LF);
        let end_line = if range.end_byte() == text.line_to_byte_idx(end_line, LineType::LF)
            && end_line > start_line
        {
            end_line - 1
        } else {
            end_line
        };
        if start_line < lines && end_line > start_line {
            regions.push(FoldRegion {
                start_line,
                end_line: end_line.min(lines.saturating_sub(1)),
            });
        }
    }
    regions.sort_by_key(|region| (region.start_line, usize::MAX - region.end_line));
    let mut normalized: Vec<FoldRegion> = Vec::new();
    for region in regions {
        match normalized.last_mut() {
            Some(last) if last.start_line == region.start_line => {
                last.end_line = last.end_line.max(region.end_line);
            }
            _ => normalized.push(region),
        }
    }
    normalized
}

pub(super) fn visible_row_count(total_raw_rows: usize, hidden: &[Range<usize>]) -> usize {
    total_raw_rows.saturating_sub(hidden.iter().map(Range::len).sum::<usize>())
}

/// 可见行 → 原始行；越过末尾时返回 None。
pub(super) fn raw_row(visible: usize, hidden: &[Range<usize>]) -> Option<usize> {
    let mut raw = visible;
    for range in hidden {
        if raw < range.start {
            break;
        }
        raw = raw.saturating_add(range.len());
    }
    Some(raw)
}

/// 原始行 → 可见行；落在折叠区间内返回 None。
pub(super) fn visible_row(raw: usize, hidden: &[Range<usize>]) -> Option<usize> {
    let mut hidden_before = 0;
    for range in hidden {
        if raw < range.start {
            break;
        }
        if raw < range.end {
            return None;
        }
        hidden_before += range.len();
    }
    raw.checked_sub(hidden_before)
}

pub(super) fn next_visible_raw(raw: usize, hidden: &[Range<usize>]) -> Option<usize> {
    let mut next = raw.saturating_add(1);
    for range in hidden {
        if next < range.start {
            break;
        }
        if next < range.end {
            next = range.end;
        }
    }
    Some(next)
}

pub(super) fn previous_visible_raw(raw: usize, hidden: &[Range<usize>]) -> Option<usize> {
    if raw == 0 {
        return None;
    }
    let mut previous = raw - 1;
    for range in hidden.iter().rev() {
        if previous < range.start {
            continue;
        }
        if previous < range.end {
            if range.start == 0 {
                return None;
            }
            previous = range.start - 1;
        }
    }
    Some(previous)
}
