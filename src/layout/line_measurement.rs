use crate::common::HashMap;
use crate::core::geometry::TextRange;
use crate::core::int_range::IntRange;
use crate::core::layout_model::{AutoSpaceDecisionInfo, Cluster, LineEndReason};
use crate::core::text_model::{TextWrapMode, WhiteSpaceMode};
use super::line_optimization::PushInAllocation;
use super::progressive_break_decisions::{ShrinkOpportunity, ProgressiveBreakOpportunity, ProgressiveBreakTier};
use crate::common::HashSet;
use super::white_space::{LineWhiteSpaceAction, LineWhiteSpaceAdjustment, WhiteSpaceAction, WhiteSpaceKind, WhiteSpaceUnit};

/// 段落共享的候选测量索引。普通区间用前缀差，位置相关区间只访问其中的 Tab。
#[derive(Clone, Debug, PartialEq)]
pub struct LineMeasurement {
    pub mode: WhiteSpaceMode,
    pub wrap: TextWrapMode,
    pub block_indent: f32,
    pub first_line_indent: f32,
    pub interval: f32,
    natural: Vec<f32>,
    adjusted: Vec<f32>,
    spaces: Vec<bool>,
    hidden: Vec<bool>,
    hard: Vec<bool>,
    run_start: Vec<usize>,
    run_end: Vec<usize>,
    tabs: Vec<(usize, f32)>,
    leading_gap: Vec<f32>,
    trailing_gap: Vec<f32>,
    next_visible: Vec<usize>,
    previous_visible: Vec<usize>,
    tab_prefix: Vec<usize>,
    technical_capacity: Vec<f32>,
    sino_count: Vec<i32>,
    cjk_count: Vec<i32>,
    gap_count: Vec<i32>,
    source_units: Vec<i32>,
    source_starts: Vec<crate::core::geometry::ScalarOffset>,
    source_ends: Vec<crate::core::geometry::ScalarOffset>,
}

#[derive(Clone, Copy, Debug)]
pub struct MeasuredLine {
    pub natural: f32,
    pub adjusted: f32,
}

impl LineMeasurement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        natural: &[Cluster], adjusted: &[Cluster], units: &HashMap<TextRange, WhiteSpaceUnit>,
        decisions: &[AutoSpaceDecisionInfo], mode: WhiteSpaceMode, wrap: TextWrapMode,
        interval: f32, tab_minimums: &HashMap<i32, f32>, block_indent: f32, first_line_indent: f32,
    ) -> Self {
        let count = adjusted.len();
        let mut result = Self {
            mode, wrap, block_indent, first_line_indent, interval,
            natural: vec![0.0; count + 1], adjusted: vec![0.0; count + 1],
            spaces: vec![false; count], hidden: vec![false; count], hard: vec![false; count],
            run_start: vec![0; count + 1], run_end: vec![count; count + 1], tabs: Vec::new(),
            leading_gap: vec![0.0; count], trailing_gap: vec![0.0; count],
            next_visible: vec![count; count + 1], previous_visible: vec![count; count + 1],
            tab_prefix: vec![0; count + 1], technical_capacity: vec![0.0; count + 1],
            sino_count: vec![0; count + 1], cjk_count: vec![0; count + 1], gap_count: vec![0; count + 1], source_units: vec![0; count + 1],
            source_starts: adjusted.iter().map(|cluster| cluster.range.start()).collect(),
            source_ends: adjusted.iter().map(|cluster| cluster.range.end()).collect(),
        };
        let by_range: HashMap<_, _> = adjusted.iter().enumerate().map(|(i, c)| (c.range, i)).collect();
        for decision in decisions {
            if let Some(&i) = by_range.get(&decision.cluster_range) {
                match decision.side.as_str() {
                    "leading" => result.leading_gap[i] += (-decision.total_reduction).max(0.0),
                    "trailing" => result.trailing_gap[i] += (-decision.total_reduction).max(0.0),
                    "gap" => {
                        result.leading_gap[i] = adjusted[i].advance;
                        result.trailing_gap[i] = adjusted[i].advance;
                    }
                    _ => {}
                }
            }
        }
        for (i, cluster) in adjusted.iter().enumerate() {
            result.natural[i + 1] = result.natural[i] + natural[i].advance;
            result.adjusted[i + 1] = result.adjusted[i] + cluster.advance;
            if let Some(unit) = units.get(&cluster.range) {
                result.hidden[i] = matches!(unit.action, WhiteSpaceAction::Collapse | WhiteSpaceAction::Remove);
                result.spaces[i] = !result.hidden[i] && (unit.action == WhiteSpaceAction::ToSpace
                    || matches!(unit.kind, WhiteSpaceKind::Space | WhiteSpaceKind::Tab));
                result.hard[i] = !result.spaces[i] && !result.hidden[i];
                if unit.kind == WhiteSpaceKind::Tab && unit.action == WhiteSpaceAction::Preserve {
                    result.tabs.push((i, tab_minimums.get(&(i as i32)).copied().unwrap_or(0.0)));
                }
            }
            result.run_start[i + 1] = if result.spaces[i] || result.hidden[i] || result.hard[i] {
                result.run_start[i]
            } else { i + 1 };
            result.previous_visible[i + 1] = if result.hidden[i] || result.hard[i] {
                result.previous_visible[i]
            } else { i };
            result.tab_prefix[i + 1] = result.tabs.len();
        }
        for i in (0..count).rev() {
            result.run_end[i] = if result.spaces[i] || result.hidden[i] || result.hard[i] { result.run_end[i + 1] } else { i };
            result.next_visible[i] = if result.hidden[i] || result.hard[i] { result.next_visible[i + 1] } else { i };
        }
        result
    }

    pub fn has_tabs(&self, range: IntRange) -> bool {
        self.tab_prefix[range.last() as usize + 1] > self.tab_prefix[range.first() as usize]
    }

    /// 两端对齐只分配给行内有效内容，软断尾空白保留占位但不成为拉伸机会。
    pub fn alignment_range(&self, range: IntRange) -> IntRange {
        if range.is_empty() { return range; }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let (head, tail) = self.edges(start, end);
        let collapse = matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks);
        let first = if collapse { head } else { start };
        let last = if collapse || (self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap) {
            tail.max(first)
        } else { end };
        IntRange::new(first as i32, last as i32 - 1)
    }

    /// 技术断行成本与普通候选共用空白有效范围，前缀数据仅在规划入口构造一次。
    pub fn with_progressive(mut self, clusters: &[Cluster], opportunities: &HashMap<i32, ProgressiveBreakOpportunity>, cjk: &HashSet<i32>, sino: &HashSet<i32>) -> Self {
        for (i, cluster) in clusters.iter().enumerate() {
            self.technical_capacity[i + 1] = self.technical_capacity[i]
                + opportunities.get(&(i as i32 + 1)).filter(|opp| opp.tier == ProgressiveBreakTier::Whitespace)
                    .map_or(0.0, |opp| opp.preceding_whitespace_stretch_capacity);
            self.sino_count[i + 1] = self.sino_count[i] + i32::from(sino.contains(&(i as i32)));
            self.cjk_count[i + 1] = self.cjk_count[i] + i32::from(cjk.contains(&(i as i32)));
            self.gap_count[i + 1] = self.gap_count[i] + i32::from(cjk.contains(&(i as i32)) || sino.contains(&(i as i32)));
            self.source_units[i + 1] = self.source_units[i] + if cluster.text.chars().any(char::is_whitespace) { 0 } else { cluster.text.scalar_len().value() };
        }
        self
    }

    pub fn effective_gap_count(&self, range: IntRange) -> i32 {
        if range.is_empty() { return 0; }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let (head, tail) = self.edges(start, end);
        let collapse = matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks);
        let first = if collapse { head } else { start };
        let last = if collapse || (self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap) { tail.max(first) } else { end };
        self.gap_count[last] - self.gap_count[(first + 1).min(last)]
    }

    pub fn progressive_stats(&self, range: IntRange, span: Option<TextRange>) -> (f32, i32, i32, i32) {
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let (head, tail) = self.edges(start, end);
        let collapse = matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks);
        let first = if collapse { head } else { start };
        let last = if collapse || (self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap) { tail.max(first) } else { end };
        let gap_first = (first + 1).min(last);
        let capacity_end = last.saturating_sub(1).max(first);
        let units = span.map_or(0, |span| {
            let left = start.max(self.source_starts.partition_point(|offset| *offset < span.start()));
            let right = end.min(self.source_ends.partition_point(|offset| *offset <= span.end()));
            if left < right { self.source_units[right] - self.source_units[left] } else { 0 }
        });
        (self.technical_capacity[capacity_end] - self.technical_capacity[first],
            self.sino_count[last] - self.sino_count[gap_first],
            self.cjk_count[last] - self.cjk_count[gap_first], units)
    }

    pub fn shrink_is_effective(&self, range: IntRange, _reason: LineEndReason, index: i32) -> bool {
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let index = index as usize;
        if index < start || index >= end || self.hidden[index] || self.hard[index] { return false; }
        if (self.next_visible[start] == index && self.leading_gap[index] >= self.adjusted[index + 1] - self.adjusted[index])
            || (self.previous_visible[end] == index && self.trailing_gap[index] >= self.adjusted[index + 1] - self.adjusted[index]) {
            return false;
        }
        let (head, tail) = self.edges(start, end);
        if matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks) {
            return index >= head && index < tail;
        }
        // 软断尾空白全悬挂；硬断与段末只悬挂溢出，但二者可容纳的条件均为正文宽度不超过行限。
        // 收缩非负尾空白不改变此条件，因此筛选机会无需区分行末原因。
        !(self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap
            && index >= tail)
    }

    /// 软断点吸收隐藏 source；保留模式中的空白串仅在串尾断开。
    pub fn boundary_allowed(&self, end: i32) -> bool {
        let end = end as usize;
        if end == self.spaces.len() { return true; }
        if self.wrap == TextWrapMode::NoWrap || self.hidden[end] || self.hard[end] { return false; }
        !(self.mode == WhiteSpaceMode::Preserve && end > 0
            && (self.spaces[end - 1] || self.hidden[end - 1]) && self.spaces[end])
    }

    pub fn next_boundary(&self, end: i32, segment_end: i32) -> i32 {
        let mut end = end;
        while end < segment_end && !self.boundary_allowed(end) { end += 1; }
        end
    }

    fn indent(&self, start: usize) -> f32 {
        self.block_indent + if start == 0 { self.first_line_indent } else { 0.0 }
    }

    /// 按半个当前字体 0 字宽阈值最多额外跳一站。
    pub fn tab_advance(&self, pen: f32, minimum: f32) -> f32 {
        if self.interval <= 0.0 { return 0.0; }
        let mut distance = ((pen / self.interval).floor() + 1.0) * self.interval - pen;
        if distance < minimum { distance += self.interval; }
        distance
    }

    /// Tab 可吸收的拉伸预算上界，按当前行 Tab 数计算。
    pub fn justification_tab_budget(&self, range: IntRange) -> f32 {
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        self.tabs[self.tab_prefix[start]..self.tab_prefix[end]].iter()
            .map(|_| self.interval * 2.0).sum()
    }

    /// 用实际占位和拉伸分配试算最终 stop；分配索引仅在此选定行内建立。
    pub fn justified_width(&self, range: IntRange, clusters: &[Cluster], deltas: &[f32]) -> f32 {
        if range.is_empty() { return 0.0; }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let mut tab = self.tab_prefix[start];
        let mut pen = self.indent(start);
        for index in start..end {
            if tab < self.tab_prefix[end] && self.tabs[tab].0 == index {
                pen += self.tab_advance(pen, self.tabs[tab].1);
                tab += 1;
            } else { pen += clusters[index].advance + deltas[index - start]; }
        }
        pen - self.indent(start)
    }

    /// 最终行只扫描自身 cluster，Tab 网格不随对齐 inset 移动。
    pub fn resolve_final_tabs(&self, range: IntRange, clusters: &mut [Cluster]) {
        if range.is_empty() || !self.has_tabs(range) { return; }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let mut tab = self.tab_prefix[start];
        let mut pen = self.indent(start);
        for index in start..end {
            if tab < self.tab_prefix[end] && self.tabs[tab].0 == index {
                clusters[index].advance = self.tab_advance(pen, self.tabs[tab].1);
                tab += 1;
            }
            pen += clusters[index].advance;
        }
    }

    /// 最终占位仅扣除实际尾空白的悬挂部分；标点悬挂由行框另行扣除。
    pub fn final_hanging_advance(&self, range: IntRange, reason: LineEndReason, limit: f32, clusters: &[Cluster]) -> f32 {
        if range.is_empty() || self.mode != WhiteSpaceMode::Preserve || self.wrap != TextWrapMode::Wrap { return 0.0; }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let tail = self.run_start[end].max(start);
        let trailing: f32 = clusters[tail..end].iter().map(|cluster| cluster.advance).sum();
        if reason == LineEndReason::AutoWrap { trailing } else {
            let width: f32 = clusters[start..end].iter().map(|cluster| cluster.advance).sum();
            (width - limit).max(0.0).min(trailing)
        }
    }

    /// 从最终占位生成完整尾悬挂记录，包含 Tab 重解析后新增的悬挂部分。
    pub fn final_hanging_adjustments(&self, range: IntRange, reason: LineEndReason, limit: f32, clusters: &[Cluster]) -> Vec<LineWhiteSpaceAdjustment> {
        let mut hanging = self.final_hanging_advance(range, reason, limit, clusters);
        let mut records = Vec::new();
        if hanging <= 0.0 || range.is_empty() { return records; }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        for index in (self.run_start[end].max(start)..end).rev() {
            if !self.spaces[index] || hanging <= 0.0 { continue; }
            let occupied_advance = clusters[index].advance;
            let hanging_advance = hanging.min(occupied_advance);
            hanging -= hanging_advance;
            records.push(LineWhiteSpaceAdjustment { cluster_index: index as i32,
                action: if reason == LineEndReason::AutoWrap { LineWhiteSpaceAction::HangEnd } else { LineWhiteSpaceAction::ConditionallyHangEnd },
                occupied_advance, hanging_advance });
        }
        records
    }

    fn width(&self, start: usize, end: usize, natural: bool, allocations: &[PushInAllocation]) -> f32 {
        let prefix = if natural { &self.natural } else { &self.adjusted };
        let mut cursor = start;
        let first = self.next_visible[start];
        let mut width = if !natural && first < end { -self.leading_gap[first] } else { 0.0 };
        let mut allocation_cursor = 0;
        let first_tab = self.tab_prefix[start];
        for &(tab, minimum) in &self.tabs[first_tab..] {
            if tab >= end { break; }
            width += prefix[tab] - prefix[cursor];
            while allocation_cursor < allocations.len() && allocations[allocation_cursor].cluster_index < tab as i32 {
                let allocation = allocations[allocation_cursor];
                if allocation.cluster_index >= cursor as i32 { width -= allocation.shrink; }
                allocation_cursor += 1;
            }
            width += self.tab_advance(self.indent(start) + width, minimum);
            cursor = tab + 1;
        }
        width += prefix[end] - prefix[cursor];
        for allocation in &allocations[allocation_cursor..] {
            if allocation.cluster_index >= cursor as i32 && allocation.cluster_index < end as i32 { width -= allocation.shrink; }
        }
        width
    }

    fn edges(&self, start: usize, end: usize) -> (usize, usize) {
        (self.run_end[start].min(end), self.run_start[end].max(start))
    }

    /// 每个 tier 内用同一个容量比例；跨 Tab 时斜率归零，并记录下一次跳站事件。
    fn profile_width(&self, start: usize, end: usize, profile: &[(usize, f32, f32)], factor: f32) -> (f32, f32, f32) {
        let prefix_at = |index: usize| {
            let count = profile.partition_point(|entry| entry.0 < index);
            if count == 0 { (0.0, 0.0) } else { (profile[count - 1].1, profile[count - 1].2) }
        };
        let mut cursor = start;
        let first = self.next_visible[start];
        let mut width = if first < end { -self.leading_gap[first] } else { 0.0 };
        let mut slope = 0.0;
        let mut next_event = f32::INFINITY;
        let first_tab = self.tab_prefix[start];
        for &(tab, minimum) in &self.tabs[first_tab..] {
            if tab >= end { break; }
            let before = prefix_at(cursor);
            let after = prefix_at(tab);
            width += self.adjusted[tab] - self.adjusted[cursor] - (after.0 - before.0) - factor * (after.1 - before.1);
            slope -= after.1 - before.1;
            if self.interval > 0.0 {
                let pen = self.indent(start) + width;
                if slope < 0.0 {
                    let floor = (pen / self.interval).floor() * self.interval;
                    let minimum_threshold = floor + self.interval - minimum;
                    for threshold in [floor, if minimum_threshold >= floor { minimum_threshold } else { floor }] {
                        let distance = (pen - threshold) / -slope;
                        if distance > 0.0 { next_event = next_event.min(factor + distance); }
                        else if distance == 0.0 {
                            // 跨过整站点时取足以改变 pen 的最小比例步长，避免逐 ULP 空转。
                            let pen_step = (f32::from_bits(pen.abs().to_bits() + 1) - pen.abs()).max(f32::MIN_POSITIVE);
                            next_event = next_event.min(factor + pen_step / -slope);
                        }
                    }
                }
                width += self.tab_advance(pen, minimum);
                slope = 0.0;
            }
            cursor = tab + 1;
        }
        let before = prefix_at(cursor);
        let after = prefix_at(end);
        width += self.adjusted[end] - self.adjusted[cursor] - (after.0 - before.0) - factor * (after.1 - before.1);
        slope -= after.1 - before.1;
        (width, slope, next_event)
    }

    /// 跳站函数在阈值处不单调；依次访问事件区间，在区间内按线性斜率求解。
    pub fn compression(&self, range: IntRange, reason: LineEndReason, limit: f32, opportunities: &[ShrinkOpportunity]) -> Option<Vec<PushInAllocation>> {
        if self.measure(range, reason, limit, &[]).adjusted <= limit { return Some(Vec::new()); }
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let (head, tail) = self.edges(start, end);
        let collapse = matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks);
        let measured_start = if collapse { head } else { start };
        // 条件悬挂后可容纳当且仅当正文可容纳；求解正文后仍用完整行及真实原因复测。
        let measured_end = if collapse || (self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap) { tail.max(measured_start) } else { end };
        // 自动间距是否位于行末由完整有效行决定，正文后的保留空白会阻止其成为行边间距。
        let edge_end = if collapse { measured_end } else { end };
        let last = self.previous_visible[edge_end];
        let edge_gap = if last < edge_end { self.trailing_gap[last] } else { 0.0 };
        let mut tiers: Vec<_> = opportunities.iter().map(|opp| opp.tier).collect();
        tiers.sort_unstable();
        tiers.dedup();
        let mut fixed = vec![0.0; opportunities.len()];
        for tier in tiers {
            let mut entries: Vec<_> = opportunities.iter().enumerate().map(|(i, opp)| {
                (opp.cluster_index as usize, fixed[i], if opp.tier == tier { opp.capacity } else { 0.0 })
            }).collect();
            entries.sort_by_key(|entry| entry.0);
            let mut fixed_sum = 0.0;
            let mut slope_sum = 0.0;
            for entry in &mut entries { fixed_sum += entry.1; slope_sum += entry.2; entry.1 = fixed_sum; entry.2 = slope_sum; }
            let mut factor = 0.0f32;
            loop {
                let (width, slope, event) = self.profile_width(measured_start, measured_end, &entries, factor);
                let width = width - edge_gap;
                if width <= limit + 0.00001 {
                    let mut allocations: Vec<_> = opportunities.iter().enumerate().filter_map(|(i, opp)| {
                        let shrink = fixed[i] + if opp.tier == tier { factor * opp.capacity } else { 0.0 };
                        (shrink > 0.0).then_some(PushInAllocation { cluster_index: opp.cluster_index, shrink,
                            available_capacity: opp.capacity, channel: opp.channel })
                    }).collect();
                    allocations.sort_by_key(|allocation| allocation.cluster_index);
                    if self.measure(range, reason, limit, &allocations).adjusted <= limit { return Some(allocations); }
                }
                let next = event.min(1.0);
                if slope < 0.0 {
                    let root = factor + (width - limit) / -slope;
                    if root <= next && root > factor { factor = root; continue; }
                }
                if next >= 1.0 {
                    if factor < 1.0 { factor = 1.0; continue; }
                    break;
                }
                // 同时检查阈值本身与其右侧，保留严格小于阈值的 CSS 跳站条件。
                factor = if next > factor { next } else { f32::from_bits(factor.to_bits() + 1) };
                if factor > 1.0 { break; }
            }
            for (i, opp) in opportunities.iter().enumerate() { if opp.tier == tier { fixed[i] = opp.capacity; } }
        }
        None
    }

    pub fn measure(&self, range: IntRange, reason: LineEndReason, limit: f32, allocations: &[PushInAllocation]) -> MeasuredLine {
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let natural = self.width(start, end, true, &[]);
        let (head, tail) = self.edges(start, end);
        let collapse = matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks);
        let mut adjusted = if collapse {
            self.width(head, tail.max(head), false, allocations)
        } else { self.width(start, end, false, allocations) };
        let last = self.previous_visible[if collapse { tail.max(head) } else { end }];
        if last < end && last >= start {
            adjusted -= self.trailing_gap[last].min(adjusted.max(0.0));
        }
        if self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap {
            let trailing = self.width(start, end, false, allocations) - self.width(start, tail, false, allocations);
            adjusted -= if reason == LineEndReason::AutoWrap { trailing } else { (adjusted - limit).max(0.0).min(trailing) };
        }
        MeasuredLine { natural, adjusted: adjusted.max(0.0) }
    }

    /// 只在选定行上生成供下一阶段消费的逐空白记录。
    pub fn adjustments(&self, range: IntRange, reason: LineEndReason, limit: f32, allocations: &[PushInAllocation]) -> Vec<LineWhiteSpaceAdjustment> {
        let start = range.first() as usize;
        let end = range.last() as usize + 1;
        let (head, tail) = self.edges(start, end);
        let mut result = Vec::new();
        let mut cursor = start;
        let first = self.next_visible[start];
        let mut pen = self.indent(start) - if first < end { self.leading_gap[first] } else { 0.0 };
        let mut allocation_cursor = 0;
        for &(tab, minimum) in &self.tabs[self.tab_prefix[start]..] {
            if tab >= end { break; }
            pen += self.adjusted[tab] - self.adjusted[cursor];
            while allocation_cursor < allocations.len() && allocations[allocation_cursor].cluster_index < tab as i32 {
                if allocations[allocation_cursor].cluster_index >= cursor as i32 { pen -= allocations[allocation_cursor].shrink; }
                allocation_cursor += 1;
            }
            let advance = self.tab_advance(pen, minimum);
            result.push(LineWhiteSpaceAdjustment { cluster_index: tab as i32, action: LineWhiteSpaceAction::ResolveTab,
                occupied_advance: advance, hanging_advance: 0.0 });
            pen += advance;
            cursor = tab + 1;
        }
        let collapse = matches!(self.mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks);
        let mut hanging = if self.mode == WhiteSpaceMode::Preserve && self.wrap == TextWrapMode::Wrap {
            let total = self.width(start, end, false, allocations);
            let trailing = total - self.width(start, tail, false, allocations);
            if reason == LineEndReason::AutoWrap { trailing } else { (total - limit).max(0.0).min(trailing) }
        } else { 0.0 };
        let mut tab_record = result.len();
        let mut allocation_record = allocations.len();
        for i in (start..end).rev() {
            if collapse && (i < head || i >= tail) && self.spaces[i] {
                result.push(LineWhiteSpaceAdjustment { cluster_index: i as i32,
                    action: if i < head { LineWhiteSpaceAction::RemoveStart } else { LineWhiteSpaceAction::RemoveEnd },
                    occupied_advance: 0.0, hanging_advance: 0.0 });
            } else if i >= tail && self.spaces[i] && hanging > 0.0 {
                while tab_record > 0 && result[tab_record - 1].cluster_index > i as i32 { tab_record -= 1; }
                while allocation_record > 0 && allocations[allocation_record - 1].cluster_index > i as i32 { allocation_record -= 1; }
                let mut shrink = 0.0;
                while allocation_record > 0 && allocations[allocation_record - 1].cluster_index == i as i32 {
                    allocation_record -= 1;
                    shrink += allocations[allocation_record].shrink;
                }
                let occupied = if tab_record > 0 && result[tab_record - 1].cluster_index == i as i32 {
                    result[tab_record - 1].occupied_advance
                } else { self.adjusted[i + 1] - self.adjusted[i] - shrink };
                let amount = hanging.min(occupied);
                hanging -= amount;
                result.push(LineWhiteSpaceAdjustment { cluster_index: i as i32,
                    action: if reason == LineEndReason::AutoWrap { LineWhiteSpaceAction::HangEnd } else { LineWhiteSpaceAction::ConditionallyHangEnd },
                    occupied_advance: occupied, hanging_advance: amount });
            }
        }
        let first = self.next_visible[if collapse { head } else { start }];
        let last = self.previous_visible[if collapse { tail.max(head) } else { end }];
        if first < end && self.leading_gap[first] > 0.0 {
            result.push(LineWhiteSpaceAdjustment { cluster_index: first as i32,
                action: LineWhiteSpaceAction::RemoveAutoSpaceStart,
                occupied_advance: self.leading_gap[first], hanging_advance: 0.0 });
        }
        if last >= start && last < end && self.trailing_gap[last] > 0.0 {
            let amount = if first == last { (self.adjusted[last + 1] - self.adjusted[last] - self.leading_gap[last]).max(0.0).min(self.trailing_gap[last]) }
                else { self.trailing_gap[last] };
            if amount > 0.0 { result.push(LineWhiteSpaceAdjustment { cluster_index: last as i32,
                action: LineWhiteSpaceAction::RemoveAutoSpaceEnd, occupied_advance: amount, hanging_advance: 0.0 }); }
        }
        result
    }
}