//! Markdown 文本 → [`FormattedText`]:把 pulldown-cmark 事件流折叠为
//! `super` 定义的扁平块列表。解析永不失败——任何输入都产出尽力而为的文档,
//! 阅读器没有错误路径要渲染。
//!
//! 本文件只拥有事件折叠。AST 在 `super`,屏幕布局在数学适配层。

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::{
    CodeBlockText, CustomWeight, FormattedImage, FormattedIndentTextInline, FormattedTable,
    FormattedTaskList, FormattedText, FormattedTextFragment, FormattedTextHeader,
    FormattedTextInline, FormattedTextLine, FormattedTextStyles, Hyperlink, MathMode, MathSource,
    OrderedFormattedIndentTextInline, TableAlignment, TextRef,
};

/// 解析 Markdown 文档。GFM 表格/任务列表/删除线恒开;YAML front-matter
/// 块识别后跳过;`ENABLE_MATH` 给出 math_flow/math_text。
pub(crate) fn parse_markdown(text: &str) -> FormattedText {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_MATH);

    let mut fold = Fold::new(Arc::from(text));
    let mut cursor = 0usize;
    for quoted_math in quoted_display_math_blocks(text) {
        fold_markdown_segment(
            &mut fold,
            &text[cursor..quoted_math.outer.start],
            cursor,
            options,
        );
        fold.flush_inline_as_paragraph();
        fold.out.push(FormattedTextLine::Quote {
            depth: quoted_math.depth,
            line: Box::new(FormattedTextLine::DisplayMath(MathSource {
                source: TextRef::generated(quoted_math.source),
                mode: MathMode::Display,
            })),
        });
        cursor = quoted_math.outer.end;
    }
    fold_markdown_segment(&mut fold, &text[cursor..], cursor, options);
    fold.finish()
}

fn fold_markdown_segment(fold: &mut Fold, text: &str, source_offset: usize, options: Options) {
    let parser_input = normalize_standalone_display_math(text);
    for (event, range) in Parser::new_ext(parser_input.as_ref(), options).into_offset_iter() {
        fold.event(
            event,
            range.start + source_offset..range.end + source_offset,
        );
    }
}

#[derive(Debug)]
struct QuotedDisplayMath {
    outer: Range<usize>,
    depth: usize,
    source: Box<str>,
}

/// 只提取引用块里显式围栏的数学。pulldown-cmark 在裸引用行处结束数学
/// 段落,含视觉空行的引用公式否则会把定界符当文本暴露。美元围栏之外的裸
/// TeX 命令仍是普通 Markdown 文本。
fn quoted_display_math_blocks(text: &str) -> Vec<QuotedDisplayMath> {
    let bytes = text.as_bytes();
    let mut blocks = Vec::new();
    let mut open: Option<(usize, usize, usize)> = None;
    let mut code_fence: Option<(usize, u8, usize)> = None;
    let mut in_metadata = false;
    let mut line_start = 0usize;

    while line_start < bytes.len() {
        let newline = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |relative| line_start + relative);
        let line_end = if newline < bytes.len() {
            newline + 1
        } else {
            newline
        };
        let content_end = if newline > line_start && bytes[newline - 1] == b'\r' {
            newline - 1
        } else {
            newline
        };
        let line = &text[line_start..content_end];
        let (quote_depth, quote_content) = quote_line_content(line);
        let trimmed = quote_content.trim();

        if line_start == 0 && quote_depth == 0 && markdown_block_content(line) == Some("---") {
            in_metadata = true;
        } else if in_metadata {
            if quote_depth == 0 && matches!(markdown_block_content(line), Some("---" | "...")) {
                in_metadata = false;
            }
        } else if let Some((outer_start, content_start, depth)) = open {
            if quote_depth != depth {
                // 引用层级已经中断,不能跨到后续引用块寻找闭合符号。
                open = None;
            } else if trimmed == "$$" {
                let source = extract_quoted_math_source(&text[content_start..line_start], depth);
                blocks.push(QuotedDisplayMath {
                    outer: outer_start..line_end,
                    depth,
                    source: source.into_boxed_str(),
                });
                open = None;
            }
        } else if let Some((depth, marker, minimum)) = code_fence {
            if quote_depth != depth
                || markdown_block_content(quote_content)
                    .is_some_and(|content| closes_code_fence(content, marker, minimum))
            {
                code_fence = None;
            }
        } else if quote_depth > 0
            && let Some(content) = markdown_block_content(quote_content)
        {
            if let Some((marker, minimum)) = opens_code_fence(content) {
                code_fence = Some((quote_depth, marker, minimum));
            } else if content == "$$" {
                open = Some((line_start, line_end, quote_depth));
            }
        }

        if newline == bytes.len() {
            break;
        }
        line_start = newline + 1;
    }

    blocks
}

fn markdown_block_content(line: &str) -> Option<&str> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    (indent <= 3).then(|| &line[indent..])
}

fn quote_line_content(mut line: &str) -> (usize, &str) {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return (0, line);
    }
    line = &line[indent..];
    let mut depth = 0usize;
    while let Some(rest) = line.strip_prefix('>') {
        depth += 1;
        line = rest.strip_prefix([' ', '\t']).unwrap_or(rest);
    }
    (depth, line)
}

fn extract_quoted_math_source(source: &str, depth: usize) -> String {
    let mut output = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let (line_depth, content) = quote_line_content(line);
        if line_depth >= depth {
            output.push_str(content.trim_start());
        }
        output.push('\n');
    }
    output.trim().to_owned()
}

/// CommonMark 段落在空行处结束,而真实世界的 `$$` 块常含空行。只把成对
/// 独立数学围栏内的行尾替换为空格,写入临时字节缓冲——pulldown-cmark 的
/// 偏移仍然有效,折叠出的文档依旧指向未改动的 UTF-8 源码。围栏代码与
/// YAML 元数据保留原行。
fn normalize_standalone_display_math(text: &str) -> Cow<'_, str> {
    let source = text.as_bytes();
    let mut normalized: Option<Vec<u8>> = None;
    let mut math_content_start = None;
    let mut code_fence: Option<(u8, usize)> = None;
    let mut in_metadata = false;
    let mut line_start = 0usize;

    while line_start < source.len() {
        let newline = source[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(source.len(), |relative| line_start + relative);
        let content_end = if newline > line_start && source[newline - 1] == b'\r' {
            newline - 1
        } else {
            newline
        };
        let line = &text[line_start..content_end];
        let indent = line.bytes().take_while(|byte| *byte == b' ').count();
        let block = (indent <= 3).then(|| &line[indent..]);

        if line_start == 0 && block == Some("---") {
            in_metadata = true;
        } else if in_metadata {
            if matches!(block, Some("---" | "...")) {
                in_metadata = false;
            }
        } else if let Some(content_start) = math_content_start {
            if block == Some("$$") {
                let output = normalized.get_or_insert_with(|| source.to_vec());
                for byte in &mut output[content_start..line_start] {
                    if matches!(*byte, b'\r' | b'\n') {
                        *byte = b' ';
                    }
                }
                math_content_start = None;
            }
        } else if let Some((marker, minimum)) = code_fence {
            if block.is_some_and(|content| closes_code_fence(content, marker, minimum)) {
                code_fence = None;
            }
        } else if let Some(content) = block {
            if let Some(fence) = opens_code_fence(content) {
                code_fence = Some(fence);
            } else if content == "$$" {
                math_content_start = Some(content_end);
            }
        }

        if newline == source.len() {
            break;
        }
        line_start = newline + 1;
    }

    normalized.map_or(Cow::Borrowed(text), |bytes| {
        Cow::Owned(String::from_utf8(bytes).expect("ASCII newline replacement preserves UTF-8"))
    })
}

fn opens_code_fence(line: &str) -> Option<(u8, usize)> {
    let marker = *line.as_bytes().first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = line.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some((marker, length))
}

fn closes_code_fence(line: &str, marker: u8, minimum: usize) -> bool {
    let length = line.bytes().take_while(|byte| *byte == marker).count();
    length >= minimum && line[length..].trim().is_empty()
}

/// 仍在收集首行文本的列表项。列表项结束时(或嵌套块打断时)才落成
/// Ordered/Unordered/Task 块。
struct PendingItem {
    /// 外层列表有序时是该项编号。
    number: Option<u64>,
    /// GFM `TaskListMarker` 在项打开后立即置位。
    task: Option<bool>,
    /// 外层列表嵌套深度(0 = 顶层)。
    depth: usize,
}

/// `Start(Table)` 与 `End(Table)` 之间的表格折叠态。
struct TableFold {
    alignments: Vec<TableAlignment>,
    headers: Vec<FormattedTextInline>,
    rows: Vec<Vec<FormattedTextInline>>,
    current_row: Vec<FormattedTextInline>,
    in_head: bool,
}

/// 事件流折叠状态。每个文档一份。
struct Fold {
    source: Arc<str>,
    out: Vec<FormattedTextLine>,
    /// 当前段落/标题/列表项/表格单元格正在收集的内联片段。
    inline: Vec<FormattedTextFragment>,
    // 内联样式状态:强调可嵌套,因此用深度而不是布尔。
    strong: usize,
    emphasis: usize,
    strike: usize,
    /// 打开中的链接目标栈;最内层生效。
    links: Vec<String>,
    /// 打开中的列表栈:`Some(next_number)` = 有序,`None` = 无序。
    /// 深度 = 栈内下标。
    list_stack: Vec<Option<u64>>,
    pending_item: Option<PendingItem>,
    /// 当前产出块外层的 `>` 引用深度。
    quote_depth: usize,
    heading: Option<usize>,
    code: Option<CodeBlockText>,
    table: Option<TableFold>,
    /// `Start(Image)` 到结束之间收集图片 alt 文本。
    image: Option<FormattedImage>,
    /// 位于 YAML front-matter 内:所有文本丢弃。
    in_metadata: bool,
}

impl Fold {
    fn new(source: Arc<str>) -> Self {
        Self {
            source,
            out: Vec::new(),
            inline: Vec::new(),
            strong: 0,
            emphasis: 0,
            strike: 0,
            links: Vec::new(),
            list_stack: Vec::new(),
            pending_item: None,
            quote_depth: 0,
            heading: None,
            code: None,
            table: None,
            image: None,
            in_metadata: false,
        }
    }

    fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if self.in_metadata {
                    return;
                }
                if let Some(code) = &mut self.code {
                    code.code.push_str(&text);
                } else if let Some(image) = &mut self.image {
                    image.alt_text.push_str(&text);
                } else {
                    self.push_source_text(range, &text, false);
                }
            }
            Event::Code(text) => {
                if let Some(image) = &mut self.image {
                    image.alt_text.push_str(&text);
                } else {
                    self.push_source_text(range, &text, true);
                }
            }
            // 阅读器把 HTML 按作者原文当字面文本渲染。
            Event::Html(html) | Event::InlineHtml(html) => {
                self.push_source_text(range, &html, false)
            }
            Event::SoftBreak => self.push_generated_text(" ", false),
            // 硬换行留在段落块内:布局层把内嵌 '\n' 变成强制视觉换行。
            Event::HardBreak => self.push_generated_text("\n", false),
            Event::Rule => {
                self.flush_inline_as_paragraph();
                self.push_block(FormattedTextLine::HorizontalRule);
            }
            Event::TaskListMarker(complete) => {
                if let Some(item) = &mut self.pending_item {
                    item.task = Some(complete);
                }
            }
            Event::FootnoteReference(name) => {
                self.push_generated_text(&format!("[^{name}]"), false);
            }
            Event::InlineMath(math) => {
                let source = self.source_ref(range, &math);
                let styles = self.current_styles(false);
                self.inline.push(FormattedTextFragment::math(
                    MathSource {
                        source,
                        mode: MathMode::Inline,
                    },
                    styles,
                ));
            }
            Event::DisplayMath(math) => {
                let source = self.display_math_source_ref(range, &math);
                self.flush_inline_as_paragraph();
                self.push_block(FormattedTextLine::DisplayMath(MathSource {
                    source,
                    mode: MathMode::Display,
                }));
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => {}
            Tag::Heading { level, .. } => {
                self.flush_inline_as_paragraph();
                self.heading = Some(level as usize);
            }
            Tag::BlockQuote(_) => {
                self.flush_inline_as_paragraph();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                // 列表项内打开代码块会结束该项首行文本。
                self.flush_pending_item();
                self.flush_inline_as_paragraph();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                let lang = if lang.is_empty() {
                    "text".to_owned()
                } else {
                    lang
                };
                self.code = Some(CodeBlockText {
                    lang,
                    code: String::new(),
                });
            }
            Tag::List(start) => {
                // 项内的子列表打开会结束该项文本。
                self.flush_pending_item();
                self.flush_inline_as_paragraph();
                self.list_stack.push(start);
            }
            Tag::Item => {
                let depth = self.list_stack.len().saturating_sub(1);
                let number = self.list_stack.last_mut().and_then(|slot| {
                    let number = *slot;
                    if let Some(n) = slot {
                        *n += 1;
                    }
                    number
                });
                self.pending_item = Some(PendingItem {
                    number,
                    task: None,
                    depth,
                });
            }
            Tag::Table(alignments) => {
                self.flush_inline_as_paragraph();
                self.table = Some(TableFold {
                    alignments: alignments
                        .into_iter()
                        .map(|alignment| match alignment {
                            Alignment::Center => TableAlignment::Center,
                            Alignment::Right => TableAlignment::Right,
                            Alignment::None | Alignment::Left => TableAlignment::Left,
                        })
                        .collect(),
                    headers: Vec::new(),
                    rows: Vec::new(),
                    current_row: Vec::new(),
                    in_head: false,
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                }
            }
            Tag::TableRow | Tag::TableCell => {}
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } => self.links.push(dest_url.into_string()),
            Tag::Image {
                dest_url, title, ..
            } => {
                self.image = Some(FormattedImage {
                    alt_text: String::new(),
                    source: dest_url.into_string(),
                    title: (!title.is_empty()).then(|| title.into_string()),
                });
            }
            Tag::MetadataBlock(_) => self.in_metadata = true,
            // 脚注正文按出现位置内联渲染(目标文档罕见);不设专用块。
            Tag::FootnoteDefinition(_) => {}
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                // 松散列表项的首段就是项文本。
                if self.pending_item.is_some() {
                    self.flush_pending_item();
                } else {
                    self.flush_inline_as_paragraph();
                }
            }
            TagEnd::Heading(_) => {
                let heading_size = self.heading.take().unwrap_or(1);
                let text = std::mem::take(&mut self.inline);
                self.push_block(FormattedTextLine::Heading(FormattedTextHeader {
                    heading_size,
                    text,
                }));
            }
            TagEnd::BlockQuote(_) => {
                self.flush_inline_as_paragraph();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some(mut code) = self.code.take() {
                    // 结尾换行是围栏语法,不是内容。
                    if code.code.ends_with('\n') {
                        code.code.pop();
                    }
                    self.push_block(FormattedTextLine::CodeBlock(code));
                }
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
            }
            TagEnd::Item => self.flush_pending_item(),
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    let mut alignments = table.alignments;
                    alignments.resize(table.headers.len().max(1), TableAlignment::Left);
                    self.push_block(FormattedTextLine::Table(FormattedTable {
                        headers: table.headers,
                        alignments,
                        rows: table.rows,
                    }));
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.current_row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.inline);
                if let Some(table) = &mut self.table {
                    if table.in_head {
                        table.headers.push(cell);
                    } else {
                        table.current_row.push(cell);
                    }
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => {
                self.links.pop();
            }
            TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    // 本阅读器把图片当块级元素;段落中段出现会切段——
                    // 对目标文件可接受。
                    self.flush_inline_as_paragraph();
                    self.push_block(FormattedTextLine::Image(image));
                }
            }
            TagEnd::MetadataBlock(_) => self.in_metadata = false,
            TagEnd::HtmlBlock => self.flush_inline_as_paragraph(),
            _ => {}
        }
    }

    /// 在活动样式下追加文本;样式相同的相邻片段合并。
    fn push_source_text(&mut self, range: Range<usize>, text: &str, inline_code: bool) {
        let text = self.source_ref(range, text);
        self.push_text_ref(text, inline_code);
    }

    fn push_generated_text(&mut self, text: &str, inline_code: bool) {
        self.push_text_ref(
            TextRef::generated(text.to_owned().into_boxed_str()),
            inline_code,
        );
    }

    fn push_text_ref(&mut self, text: TextRef, inline_code: bool) {
        if text.as_str().is_empty() || self.in_metadata {
            return;
        }
        let styles = self.current_styles(inline_code);
        let text = if let Some(last) = self.inline.last_mut() {
            if last.styles == styles {
                match last.append_text(text) {
                    Ok(()) => return,
                    Err(text) => text,
                }
            } else {
                text
            }
        } else {
            text
        };
        self.inline
            .push(FormattedTextFragment::from_text(text, styles));
    }

    fn current_styles(&self, inline_code: bool) -> FormattedTextStyles {
        FormattedTextStyles {
            weight: (self.strong > 0).then_some(CustomWeight::Bold),
            italic: self.emphasis > 0,
            underline: false,
            strikethrough: self.strike > 0,
            inline_code,
            hyperlink: self.links.last().map(|url| Hyperlink::Url(url.clone())),
        }
    }

    fn source_ref(&self, outer: Range<usize>, rendered: &str) -> TextRef {
        let source_range = self.source.get(outer.clone()).and_then(|outer_text| {
            outer_text.find(rendered).map(|offset| {
                let start = outer.start + offset;
                start..start + rendered.len()
            })
        });
        source_range
            .and_then(|range| TextRef::source(self.source.clone(), range))
            .unwrap_or_else(|| TextRef::generated(rendered.to_owned().into_boxed_str()))
    }

    fn display_math_source_ref(&self, outer: Range<usize>, rendered: &str) -> TextRef {
        if let Some(range) = self.source.get(outer.clone()).and_then(|outer_text| {
            outer_text.find(rendered).map(|offset| {
                let start = outer.start + offset;
                start..start + rendered.len()
            })
        }) {
            return TextRef::source(self.source.clone(), range)
                .unwrap_or_else(|| TextRef::generated(rendered.to_owned().into_boxed_str()));
        }

        let Some(outer_text) = self.source.get(outer.clone()) else {
            return TextRef::generated(rendered.to_owned().into_boxed_str());
        };
        let leading = outer_text.len() - outer_text.trim_start().len();
        let trimmed = outer_text.trim();
        if !trimmed.starts_with("$$") || !trimmed.ends_with("$$") || trimmed.len() < 4 {
            return TextRef::generated(rendered.to_owned().into_boxed_str());
        }
        let inner = &trimmed[2..trimmed.len() - 2];
        let inner_leading = inner.len() - inner.trim_start().len();
        let inner_trimmed = inner.trim();
        let start = outer.start + leading + 2 + inner_leading;
        TextRef::source(self.source.clone(), start..start + inner_trimmed.len())
            .unwrap_or_else(|| TextRef::generated(rendered.to_owned().into_boxed_str()))
    }

    /// 把待定内联串落成普通段落块。
    fn flush_inline_as_paragraph(&mut self) {
        if self.inline.is_empty() {
            return;
        }
        let inline = std::mem::take(&mut self.inline);
        self.push_block(FormattedTextLine::Line(inline));
    }

    /// 把打开中的列表项落成其列表块。
    fn flush_pending_item(&mut self) {
        let Some(item) = self.pending_item.take() else {
            return;
        };
        let text = std::mem::take(&mut self.inline);
        let line = match item.task {
            Some(complete) => FormattedTextLine::TaskList(FormattedTaskList {
                complete,
                indent_level: item.depth,
                text,
            }),
            None => {
                let indented_text = FormattedIndentTextInline {
                    indent_level: item.depth,
                    text,
                };
                match item.number {
                    Some(number) => {
                        FormattedTextLine::OrderedList(OrderedFormattedIndentTextInline {
                            number: Some(number as usize),
                            indented_text,
                        })
                    }
                    None => FormattedTextLine::UnorderedList(indented_text),
                }
            }
        };
        self.push_block(line);
    }

    /// 落一个完成块;活动引用深度包裹它。
    fn push_block(&mut self, line: FormattedTextLine) {
        let line = if self.quote_depth > 0 {
            FormattedTextLine::Quote {
                depth: self.quote_depth,
                line: Box::new(line),
            }
        } else {
            line
        };
        self.out.push(line);
    }

    fn finish(mut self) -> FormattedText {
        self.flush_inline_as_paragraph();
        FormattedText {
            source: self.source,
            lines: self.out.into(),
        }
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;
    use crate::markdown::{FragmentContent, MathMode, TextRef};

    fn parse(text: &str) -> Vec<FormattedTextLine> {
        parse_markdown(text).lines.into_iter().collect()
    }

    fn plain(text: &str) -> FormattedTextFragment {
        FormattedTextFragment::plain_text(text)
    }

    #[test]
    fn math_events_keep_mode_and_utf8_source_ranges() {
        let document = parse_markdown("中文 $x^2$\n\n$$\\frac{1}{2}$$");
        let FormattedTextLine::Line(inline) = &document.lines[0] else {
            panic!("expected paragraph");
        };
        let FragmentContent::Math(inline_math) = &inline[1].content else {
            panic!("expected inline math fragment: {:?}", inline[1]);
        };
        assert_eq!(inline_math.mode, MathMode::Inline);
        assert_eq!(inline_math.source.as_str(), "x^2");

        let FormattedTextLine::DisplayMath(display_math) = &document.lines[1] else {
            panic!("expected display math block: {:?}", document.lines[1]);
        };
        assert_eq!(display_math.mode, MathMode::Display);
        assert_eq!(display_math.source.as_str(), r"\frac{1}{2}");

        let TextRef::Source { source, range } = &inline_math.source else {
            panic!("math source should point into the document");
        };
        assert!(std::sync::Arc::ptr_eq(source, &document.source));
        assert_eq!(&document.source[range.as_usize()], "x^2");
    }

    #[test]
    fn standalone_display_math_crosses_blank_lines_without_copying_its_source() {
        let source = concat!(
            "#### 辅助角公式\n\n",
            "$$\n",
            r"\sin x + \cos x = \sqrt{2} \\",
            "\n",
            "\n",
            r"这里使用了和角公式：\\",
            "\n",
            "\\sin(a + b) = \\sin a \\cos b + \\cos a \\sin b\n",
            "$$\n",
        );
        let document = parse_markdown(source);
        let FormattedTextLine::DisplayMath(math) = &document.lines[1] else {
            panic!("expected one display-math block: {:?}", document.lines);
        };

        assert!(math.source.as_str().contains("\n\n这里使用了和角公式"));
        let TextRef::Source { source, .. } = &math.source else {
            panic!("display math must retain an original source range");
        };
        assert!(Arc::ptr_eq(source, &document.source));
    }

    #[test]
    fn standalone_math_normalization_skips_fenced_code_and_yaml_metadata() {
        let source = concat!(
            "---\nexample: |\n  $$\n  metadata\n  $$\n---\n\n",
            "```text\n$$\ncode\n\nblock\n$$\n```\n",
        );
        assert!(matches!(
            normalize_standalone_display_math(source),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn quoted_display_math_accepts_indentation_and_blank_quote_rows() {
        for source in [
            concat!(
                ">   $$\n",
                ">   \\lim_{x \\to x_0} f(x) = A \\iff f(x) = A + \\alpha(x)\n",
                ">   $$\n",
            ),
            concat!(
                "> $$\n",
                "> sin(2a)=2sin(a)cos(a) -> sin(a)cos(a)\\\\\n",
                ">\n",
                "> cos(2a)=cos^2(a)-sin^2(a)\\\\\n",
                "> $$\n",
            ),
        ] {
            let document = parse_markdown(source);
            let Some(FormattedTextLine::Quote { line, .. }) = document.lines.front() else {
                panic!("expected quoted formula: {:?}", document.lines);
            };
            assert!(
                matches!(line.as_ref(), FormattedTextLine::DisplayMath(_)),
                "quoted math rendered as text: {:?}",
                document.lines
            );
        }
    }

    #[test]
    fn bare_tex_commands_are_plain_text_until_dollar_fenced() {
        let document = parse_markdown(concat!(
            "a -> b\n\n",
            "\\lim_{x \\to 0} f(x)\n\n",
            "\\frac{1}{2}\n\n",
            "\\sqrt{x^2}\n",
            "\\(x^2\\) and \\[y^2\\]\n",
        ));
        for line in &document.lines {
            let FormattedTextLine::Line(inline) = line else {
                panic!("bare TeX unexpectedly became a block: {line:?}");
            };
            assert!(inline.iter().all(|fragment| !fragment.is_math()));
        }
        assert!(document.raw_text().contains("a -> b"));

        let fenced = parse_markdown(r"plain $x^2$ and $$\frac{1}{2}$$");
        assert!(fenced.lines.iter().any(|line| match line {
            FormattedTextLine::Line(inline) => inline.iter().any(FormattedTextFragment::is_math),
            FormattedTextLine::DisplayMath(_) => true,
            _ => false,
        }));
    }

    #[test]
    fn quoted_code_fences_never_become_display_math() {
        let source = concat!(
            "> ```tex\n",
            "> $$\n",
            "> \\frac{1}{2}\n",
            "> $$\n",
            "> ```\n",
        );
        assert!(quoted_display_math_blocks(source).is_empty());

        let document = parse_markdown(source);
        let Some(FormattedTextLine::Quote { line, .. }) = document.lines.front() else {
            panic!("expected quoted code block: {:?}", document.lines);
        };
        assert!(matches!(line.as_ref(), FormattedTextLine::CodeBlock(_)));
    }

    #[test]
    fn escaped_dollars_and_code_do_not_become_math() {
        let document = parse_markdown(r"escaped \$x$ and `$code$`");
        let FormattedTextLine::Line(inline) = &document.lines[0] else {
            panic!()
        };
        assert!(inline.iter().all(|fragment| !fragment.is_math()));
        assert_eq!(document.raw_text().trim_end(), "escaped $x$ and $code$");
    }

    #[test]
    fn heading_paragraph_and_inline_styles() {
        let lines = parse("# Title\n\nplain **bold** *it* `code` ~~gone~~");
        assert_eq!(
            lines[0],
            FormattedTextLine::Heading(FormattedTextHeader {
                heading_size: 1,
                text: vec![plain("Title")],
            })
        );
        let FormattedTextLine::Line(inline) = &lines[1] else {
            panic!("expected paragraph, got {:?}", lines[1]);
        };
        assert_eq!(inline[0], plain("plain "));
        assert_eq!(inline[1], FormattedTextFragment::bold("bold"));
        assert_eq!(inline[2], plain(" "));
        assert_eq!(inline[3], FormattedTextFragment::italic("it"));
        assert_eq!(inline[4], plain(" "));
        assert_eq!(inline[5], FormattedTextFragment::inline_code("code"));
        assert!(inline[7].styles.strikethrough);
        assert_eq!(inline[7].text(), "gone");
    }

    #[test]
    fn links_carry_their_target() {
        let lines = parse("see [docs](https://example.com) now");
        let FormattedTextLine::Line(inline) = &lines[0] else {
            panic!()
        };
        assert_eq!(
            inline[1],
            FormattedTextFragment::hyperlink("docs", "https://example.com")
        );
        assert_eq!(
            lines[0].hyperlinks(),
            vec![(4..8, "https://example.com".to_owned())]
        );
    }

    #[test]
    fn fenced_code_block_keeps_lang_and_content() {
        let lines = parse("```rust\nfn main() {}\nlet x = 1;\n```\n");
        assert_eq!(
            lines[0],
            FormattedTextLine::CodeBlock(CodeBlockText {
                lang: "rust".into(),
                code: "fn main() {}\nlet x = 1;".into(),
            })
        );
    }

    #[test]
    fn unlabelled_code_block_defaults_to_text() {
        let lines = parse("```\nhi\n```\n");
        let FormattedTextLine::CodeBlock(block) = &lines[0] else {
            panic!()
        };
        assert_eq!(block.lang, "text");
    }

    #[test]
    fn nested_lists_carry_indent_levels() {
        let lines = parse("- a\n  - b\n- c\n");
        let levels: Vec<(usize, String)> = lines
            .iter()
            .filter_map(|line| match line {
                FormattedTextLine::UnorderedList(list) => {
                    Some((list.indent_level, line.raw_text().trim().to_owned()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            levels,
            vec![(0, "a".into()), (1, "b".into()), (0, "c".into())]
        );
    }

    #[test]
    fn ordered_lists_number_their_items() {
        let lines = parse("3. three\n4. four\n");
        let numbers: Vec<Option<usize>> = lines
            .iter()
            .filter_map(|line| match line {
                FormattedTextLine::OrderedList(item) => Some(item.number),
                _ => None,
            })
            .collect();
        assert_eq!(numbers, vec![Some(3), Some(4)]);
    }

    #[test]
    fn task_lists_keep_completion_state() {
        let lines = parse("- [x] done\n- [ ] todo\n");
        let states: Vec<bool> = lines
            .iter()
            .filter_map(|line| match line {
                FormattedTextLine::TaskList(task) => Some(task.complete),
                _ => None,
            })
            .collect();
        assert_eq!(states, vec![true, false]);
    }

    #[test]
    fn block_quotes_wrap_inner_blocks_with_depth() {
        let lines = parse("> quoted\n>> deeper\n");
        let FormattedTextLine::Quote { depth: 1, line } = &lines[0] else {
            panic!("expected depth-1 quote, got {:?}", lines[0]);
        };
        assert!(matches!(line.as_ref(), FormattedTextLine::Line(_)));
        assert!(matches!(
            &lines[1],
            FormattedTextLine::Quote { depth: 2, .. }
        ));
    }

    #[test]
    fn tables_capture_alignment_and_cells() {
        let lines = parse("| a | b |\n|:-:|--:|\n| 1 | 2 |\n");
        let FormattedTextLine::Table(table) = &lines[0] else {
            panic!()
        };
        assert_eq!(
            table.alignments,
            vec![TableAlignment::Center, TableAlignment::Right]
        );
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows, vec![vec![vec![plain("1")], vec![plain("2")]]]);
    }

    #[test]
    fn rule_and_breaks() {
        let lines = parse("above\n\n---\n\nbelow");
        assert!(matches!(lines[1], FormattedTextLine::HorizontalRule));
        // 硬换行留在同一段落里,成为内嵌换行。
        let lines = parse("one  \ntwo");
        let FormattedTextLine::Line(inline) = &lines[0] else {
            panic!()
        };
        let text: String = inline.iter().map(FormattedTextFragment::text).collect();
        assert_eq!(text, "one\ntwo");
    }

    #[test]
    fn images_become_blocks_with_alt_text() {
        let lines = parse("![logo](img/logo.png \"The Logo\")\n");
        assert_eq!(
            lines[0],
            FormattedTextLine::Image(FormattedImage {
                alt_text: "logo".into(),
                source: "img/logo.png".into(),
                title: Some("The Logo".into()),
            })
        );
    }

    #[test]
    fn yaml_front_matter_is_skipped() {
        let lines = parse("---\ntitle: hidden\n---\n\n# Visible\n");
        assert_eq!(lines.len(), 1);
        assert!(matches!(&lines[0], FormattedTextLine::Heading(h) if h.heading_size == 1));
    }

    #[test]
    fn code_block_inside_list_item_flushes_the_item_first() {
        let lines = parse("- item\n\n  ```sh\n  ls\n  ```\n");
        assert!(matches!(&lines[0], FormattedTextLine::UnorderedList(_)));
        assert!(matches!(&lines[1], FormattedTextLine::CodeBlock(_)));
    }

    #[test]
    fn adjacent_same_style_text_merges_into_one_fragment() {
        // "a & b" 到达时是三个 Text 事件(中间是实体),折叠后是一个片段。
        let lines = parse("a &amp; b");
        let FormattedTextLine::Line(inline) = &lines[0] else {
            panic!()
        };
        assert_eq!(inline.len(), 1);
        assert_eq!(inline[0].text(), "a & b");
    }
}
