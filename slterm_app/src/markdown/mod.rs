//! 自含 Markdown 文档模型(08 篇 M9.5,管线 B)。
//!
//! 本模块只拥有解析与解析结果——不渲染、不持窗口、不接触终端。阅读器
//! (数学适配见 [`math_run`])消费这里的值。
//!
//! 形态是**扁平的块序列**(一个段落一条、一个围栏代码块一条……)而不是
//! 嵌套树:虚拟滚动要二分「像素 Y 落在哪个块」并只布局可视切片,扁平列表
//! 天然满足。保留的嵌套(列表层级、引用深度)以数据字段承载,不建树。
//!
//! 数学片段(`FormattedTextLine::DisplayMath` / `FragmentContent::Math`)是
//! 引擎的唯二入口:markdown 与终端定位公式的方式不同,但都不允许独立
//! 规范化/解析/排版(08 篇边界 6)。

use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

pub(crate) mod math_run;
pub(crate) mod parser;

#[allow(unused_imports)] // 消费点 = 07 篇阅读器接线;本步由用例锁定语义。
pub(crate) use math_run::{MathRun, fit_math_run, measure_math};
#[allow(unused_imports)] // 消费点 = 07 篇阅读器接线;本步由用例锁定语义。
pub(crate) use parser::parse_markdown;

/// 片段携带的字重。解析器只会产出 `Bold`;完整刻度用于渲染层映射字重,
/// 而非布尔标志。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum CustomWeight {
    Thin,
    ExtraLight,
    Light,
    Medium,
    Semibold,
    Bold,
    ExtraBold,
    Black,
}

impl CustomWeight {
    /// 加粗或更重。
    pub(crate) fn is_at_least_bold(&self) -> bool {
        matches!(
            self,
            CustomWeight::Bold | CustomWeight::ExtraBold | CustomWeight::Black
        )
    }
}

/// 解析后的 Markdown 文档:扁平的块级行序列。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct FormattedText {
    /// Markdown 源码只保留这一份共享字节;片段用范围引用它,避免逐层复制。
    pub source: Arc<str>,
    pub lines: VecDeque<FormattedTextLine>,
}

impl FormattedText {
    pub(crate) fn new(lines: impl Into<VecDeque<FormattedTextLine>>) -> Self {
        Self {
            source: Arc::from(""),
            lines: lines.into(),
        }
    }

    /// 文档原文,去掉 Markdown 标记。
    pub(crate) fn raw_text(&self) -> String {
        self.lines.iter().map(|line| line.raw_text()).collect()
    }
}

/// UTF-8 源码中的半开字节范围。解析入口保证大小可用 `u32` 表示。
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) struct SourceRange {
    pub start: u32,
    pub end: u32,
}

impl SourceRange {
    pub(crate) fn new(range: Range<usize>) -> Option<Self> {
        Some(Self {
            start: range.start.try_into().ok()?,
            end: range.end.try_into().ok()?,
        })
    }

    pub(crate) fn as_usize(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// 绝大多数内容引用 Markdown 源码;只有实体解码、软换行等合成文本单独分配。
#[derive(Clone, Debug)]
pub(crate) enum TextRef {
    Source {
        source: Arc<str>,
        range: SourceRange,
    },
    Generated(Box<str>),
}

impl Default for TextRef {
    fn default() -> Self {
        Self::Generated(Box::default())
    }
}

impl PartialEq for TextRef {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for TextRef {}

impl TextRef {
    pub(crate) fn source(source: Arc<str>, range: Range<usize>) -> Option<Self> {
        let range = SourceRange::new(range)?;
        source.get(range.as_usize())?;
        Some(Self::Source { source, range })
    }

    pub(crate) fn generated(text: impl Into<Box<str>>) -> Self {
        Self::Generated(text.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Source { source, range } => &source[range.as_usize()],
            Self::Generated(text) => text,
        }
    }

    /// 只合并同一源码中的连续范围;跨 Markdown 标记的内容保持独立,
    /// 避免错误扩大范围。
    fn append(&mut self, next: Self) -> Result<(), Self> {
        match (self, next) {
            (
                Self::Source { source, range },
                Self::Source {
                    source: next_source,
                    range: next_range,
                },
            ) if Arc::ptr_eq(source, &next_source) && range.end == next_range.start => {
                range.end = next_range.end;
                Ok(())
            }
            (slot @ Self::Generated(_), Self::Generated(next)) => {
                let mut combined = String::from(slot.as_str());
                combined.push_str(&next);
                *slot = Self::Generated(combined.into_boxed_str());
                Ok(())
            }
            // 实体解码或反斜杠转义会在同样式文本中留下源码间隙;此时只合成
            // 这一小段,既保持 fragment 合并语义,也不扩大 range 到标记字节。
            (slot, next) => {
                let mut combined = String::from(slot.as_str());
                combined.push_str(next.as_str());
                *slot = Self::Generated(combined.into_boxed_str());
                Ok(())
            }
        }
    }
}

/// 数学片段的显示模式。两条管线共用;终端覆盖层只消费 `Display` 的块级形态。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum MathMode {
    Inline,
    Display,
}

/// 一条数学源码 + 模式。源码经传输层归一后进入 `compile_formula` 单点。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MathSource {
    pub source: TextRef,
    pub mode: MathMode,
}

impl MathSource {
    pub(crate) fn as_str(&self) -> &str {
        self.source.as_str()
    }
}

/// 文档的一个块级元素。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum FormattedTextLine {
    Heading(FormattedTextHeader),
    Line(FormattedTextInline),
    OrderedList(OrderedFormattedIndentTextInline),
    UnorderedList(FormattedIndentTextInline),
    CodeBlock(CodeBlockText),
    TaskList(FormattedTaskList),
    LineBreak,
    HorizontalRule,
    Image(FormattedImage),
    Table(FormattedTable),
    DisplayMath(MathSource),
    /// `>` 引用内的块。引用包裹内层块而不是给每个变体分叉,
    /// 扁平列表(及虚拟滚动的块索引)得以保留。
    Quote {
        depth: usize,
        line: Box<FormattedTextLine>,
    },
}

impl FormattedTextLine {
    pub(crate) fn raw_text(&self) -> String {
        let join = |inline: &FormattedTextInline| -> String {
            inline.iter().map(FormattedTextFragment::text).collect()
        };
        let mut text = match self {
            Self::CodeBlock(text) => text.code.clone(),
            Self::Heading(header) => join(&header.text),
            Self::Line(line) => join(line),
            Self::TaskList(line) => join(&line.text),
            Self::OrderedList(list) => join(&list.indented_text.text),
            Self::UnorderedList(list) => join(&list.text),
            Self::LineBreak | Self::HorizontalRule => "\n".to_string(),
            Self::Image(image) => format!("{}\n", image.alt_text),
            Self::Table(table) => table.to_plain_text(),
            Self::DisplayMath(math) => math.as_str().to_owned(),
            Self::Quote { line, .. } => line.raw_text(),
        };
        // 每个 `FormattedTextLine` 单元代表一整行。
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    }

    fn inline_fragments(&self) -> Option<&FormattedTextInline> {
        match &self {
            FormattedTextLine::Heading(header) => Some(&header.text),
            FormattedTextLine::Line(texts) => Some(texts),
            FormattedTextLine::OrderedList(texts) => Some(&texts.indented_text.text),
            FormattedTextLine::UnorderedList(texts) => Some(&texts.text),
            FormattedTextLine::TaskList(list) => Some(&list.text),
            FormattedTextLine::Quote { line, .. } => line.inline_fragments(),
            FormattedTextLine::CodeBlock(_)
            | FormattedTextLine::LineBreak
            | FormattedTextLine::HorizontalRule
            | FormattedTextLine::Image(_)
            | FormattedTextLine::Table(_)
            | FormattedTextLine::DisplayMath(_) => None,
        }
    }

    /// 本行超链接,范围是拼接后内联文本的字符区间——阅读器据此映射点击。
    pub(crate) fn hyperlinks(&self) -> Vec<(Range<usize>, String)> {
        let mut hyperlinks = Vec::new();
        if let Some(inline_fragments) = self.inline_fragments() {
            let mut char_count = 0;
            for fragment in inline_fragments {
                let range_start = char_count;
                char_count += fragment.text().chars().count();
                if let Some(Hyperlink::Url(url)) = &fragment.styles.hyperlink {
                    hyperlinks.push((range_start..char_count, url.clone()));
                }
            }
        }
        hyperlinks
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormattedTextHeader {
    pub heading_size: usize,
    pub text: FormattedTextInline,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormattedTaskList {
    pub complete: bool,
    pub indent_level: usize,
    pub text: FormattedTextInline,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormattedIndentTextInline {
    pub indent_level: usize,
    pub text: FormattedTextInline,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct CodeBlockText {
    pub lang: String,
    pub code: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OrderedFormattedIndentTextInline {
    /// 列表项编号;源文档未指定或非法时为 `None`。
    pub number: Option<usize>,
    pub indented_text: FormattedIndentTextInline,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormattedImage {
    pub alt_text: String,
    pub source: String,
    /// CommonMark 图片 title,如 `![alt](src "title")`;空 title 归一为 `None`。
    pub title: Option<String>,
}

/// 表格列对齐。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Hash)]
pub(crate) enum TableAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// 带表头、对齐与行的表格。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormattedTable {
    pub headers: Vec<FormattedTextInline>,
    pub alignments: Vec<TableAlignment>,
    pub rows: Vec<Vec<FormattedTextInline>>,
}

impl FormattedTable {
    /// 序列化为 GFM 管道表格(原文提取用)。
    pub(crate) fn to_plain_text(&self) -> String {
        fn inline_to_text(inline: &FormattedTextInline) -> String {
            inline.iter().map(FormattedTextFragment::text).collect()
        }

        let mut lines = Vec::new();
        let headers: Vec<String> = self.headers.iter().map(inline_to_text).collect();
        lines.push(format!("| {} |", headers.join(" | ")));
        let separator: Vec<String> = self
            .alignments
            .iter()
            .map(|alignment| match alignment {
                TableAlignment::Left => "---".to_string(),
                TableAlignment::Center => ":---:".to_string(),
                TableAlignment::Right => "---:".to_string(),
            })
            .collect();
        lines.push(format!("| {} |", separator.join(" | ")));
        for row in &self.rows {
            let cells: Vec<String> = row.iter().map(inline_to_text).collect();
            lines.push(format!("| {} |", cells.join(" | ")));
        }
        lines.join("\n")
    }
}

pub(crate) type FormattedTextInline = Vec<FormattedTextFragment>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum FragmentContent {
    Text(TextRef),
    Math(MathSource),
}

impl Default for FragmentContent {
    fn default() -> Self {
        Self::Text(TextRef::default())
    }
}

/// 格式文本片段:内容 + 样式。
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub(crate) struct FormattedTextFragment {
    pub content: FragmentContent,
    pub styles: FormattedTextStyles,
}

/// 可点击链接目标。留成枚举,未来「在应用内打开」的动作目标
/// 可与普通 URL 并列加入。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Hyperlink {
    Url(String),
}

/// 无内容的格式样式。
#[derive(Clone, Default, Eq, PartialEq)]
pub(crate) struct FormattedTextStyles {
    pub weight: Option<CustomWeight>,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inline_code: bool,
    pub hyperlink: Option<Hyperlink>,
}

impl FormattedTextFragment {
    pub(crate) fn plain_text(text: impl Into<String>) -> Self {
        Self::from_generated(text, Default::default())
    }

    pub(crate) fn bold(text: impl Into<String>) -> Self {
        Self {
            content: FragmentContent::Text(TextRef::generated(text.into().into_boxed_str())),
            styles: FormattedTextStyles {
                weight: Some(CustomWeight::Bold),
                ..Default::default()
            },
        }
    }

    pub(crate) fn italic(text: impl Into<String>) -> Self {
        Self {
            content: FragmentContent::Text(TextRef::generated(text.into().into_boxed_str())),
            styles: FormattedTextStyles {
                italic: true,
                ..Default::default()
            },
        }
    }

    pub(crate) fn hyperlink(tag: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            content: FragmentContent::Text(TextRef::generated(tag.into().into_boxed_str())),
            styles: FormattedTextStyles {
                hyperlink: Some(Hyperlink::Url(url.into())),
                ..Default::default()
            },
        }
    }

    pub(crate) fn inline_code(text: impl Into<String>) -> Self {
        Self {
            content: FragmentContent::Text(TextRef::generated(text.into().into_boxed_str())),
            styles: FormattedTextStyles {
                inline_code: true,
                ..Default::default()
            },
        }
    }

    pub(crate) fn from_text(text: TextRef, styles: FormattedTextStyles) -> Self {
        Self {
            content: FragmentContent::Text(text),
            styles,
        }
    }

    pub(crate) fn math(source: MathSource, styles: FormattedTextStyles) -> Self {
        Self {
            content: FragmentContent::Math(source),
            styles,
        }
    }

    pub(crate) fn text(&self) -> &str {
        match &self.content {
            FragmentContent::Text(text) => text.as_str(),
            FragmentContent::Math(math) => math.as_str(),
        }
    }

    pub(crate) fn is_math(&self) -> bool {
        matches!(self.content, FragmentContent::Math(_))
    }

    fn append_text(&mut self, text: TextRef) -> Result<(), TextRef> {
        match &mut self.content {
            FragmentContent::Text(current) => current.append(text),
            FragmentContent::Math(_) => Err(text),
        }
    }

    fn from_generated(text: impl Into<String>, styles: FormattedTextStyles) -> Self {
        let text = text.into().into_boxed_str();
        Self {
            content: FragmentContent::Text(TextRef::generated(text)),
            styles,
        }
    }
}

impl fmt::Debug for FormattedTextStyles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 可读性:只显示启用的样式。
        let mut parts: Vec<String> = Vec::new();
        if let Some(weight) = self.weight {
            parts.push(format!("{weight:?}"));
        }
        if self.italic {
            parts.push("Italic".into());
        }
        if self.underline {
            parts.push("Underline".into());
        }
        if self.strikethrough {
            parts.push("Strikethrough".into());
        }
        if self.inline_code {
            parts.push("InlineCode".into());
        }
        if let Some(link) = &self.hyperlink {
            parts.push(format!("Hyperlink({link:?})"));
        }
        if parts.is_empty() {
            f.write_str("PlainText")
        } else {
            f.write_str(&parts.join(" | "))
        }
    }
}
