//! Comrak AST 的不可变文档投影；不依赖 GPUI 或外部 I/O。

use std::collections::HashMap;
use std::ops::Range;

use comrak::nodes::{AlertType, AstNode, NodeValue};
use comrak::{Arena, Options};
use unicode_normalization::UnicodeNormalization as _;

use crate::markdown_html::MarkdownHtmlFragment;
use crate::{LanguageId, LanguageRegistry};

/// 当前投影内的块身份；不得持久化到其他 revision。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(usize);

impl BlockId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownCodeBlock {
    language: LanguageId,
    source: String,
}

impl MarkdownCodeBlock {
    pub const fn language(&self) -> LanguageId {
        self.language
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownAlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownBlockKind {
    Paragraph,
    Heading(u8),
    List,
    Quote,
    Alert(MarkdownAlertKind),
    ThematicBreak,
    Code(MarkdownCodeBlock),
    Table,
    Footnote,
    Math,
    FrontMatter,
    Html(MarkdownHtmlFragment),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MarkdownStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub subscript: bool,
    pub superscript: bool,
    pub kbd: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownInlineKind {
    Text,
    Code,
    Math,
    Image,
    FootnoteReference,
    LineBreak,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownInline {
    kind: MarkdownInlineKind,
    text: String,
    style: MarkdownStyle,
    url: Option<String>,
    source_range: Range<usize>,
    /// 范围相对当前块的可见文本；图片与脚注引用为空范围。
    visible_range: Range<usize>,
}

impl MarkdownInline {
    pub const fn kind(&self) -> MarkdownInlineKind {
        self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn style(&self) -> &MarkdownStyle {
        &self.style
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn source_range(&self) -> Range<usize> {
        self.source_range.clone()
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.visible_range.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownStructure {
    List(MarkdownList),
    Table(MarkdownTable),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownList {
    items: Vec<MarkdownListItem>,
}

impl MarkdownList {
    pub fn items(&self) -> &[MarkdownListItem] {
        &self.items
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownListItem {
    ordinal: Option<usize>,
    checked: Option<bool>,
    depth: usize,
    inlines: Vec<MarkdownInline>,
    children: Vec<MarkdownListItem>,
}

impl MarkdownListItem {
    pub const fn ordinal(&self) -> Option<usize> {
        self.ordinal
    }

    pub const fn checked(&self) -> Option<bool> {
        self.checked
    }

    pub const fn depth(&self) -> usize {
        self.depth
    }

    pub fn inlines(&self) -> &[MarkdownInline] {
        &self.inlines
    }

    pub fn children(&self) -> &[MarkdownListItem] {
        &self.children
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownAlignment {
    None,
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTable {
    alignments: Vec<MarkdownAlignment>,
    rows: Vec<MarkdownTableRow>,
}

impl MarkdownTable {
    pub fn alignments(&self) -> &[MarkdownAlignment] {
        &self.alignments
    }

    pub fn rows(&self) -> &[MarkdownTableRow] {
        &self.rows
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTableRow {
    header: bool,
    cells: Vec<MarkdownTableCell>,
}

impl MarkdownTableRow {
    pub const fn header(&self) -> bool {
        self.header
    }

    pub fn cells(&self) -> &[MarkdownTableCell] {
        &self.cells
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTableCell {
    inlines: Vec<MarkdownInline>,
}

impl MarkdownTableCell {
    pub fn inlines(&self) -> &[MarkdownInline] {
        &self.inlines
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownBlock {
    id: BlockId,
    kind: MarkdownBlockKind,
    structure: Option<MarkdownStructure>,
    inlines: Vec<MarkdownInline>,
    source_range: Range<usize>,
    visible_range: Range<usize>,
}

impl MarkdownBlock {
    pub const fn id(&self) -> BlockId {
        self.id
    }

    pub const fn kind(&self) -> &MarkdownBlockKind {
        &self.kind
    }

    pub fn source_range(&self) -> Range<usize> {
        self.source_range.clone()
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.visible_range.clone()
    }

    pub fn inlines(&self) -> &[MarkdownInline] {
        &self.inlines
    }

    pub fn structure(&self) -> Option<&MarkdownStructure> {
        self.structure.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownHeading {
    text: String,
    depth: u8,
    block: BlockId,
    anchor: String,
}

impl MarkdownHeading {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn depth(&self) -> u8 {
        self.depth
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub fn anchor(&self) -> &str {
        &self.anchor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownAnchorKind {
    Heading,
    Footnote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownAnchor {
    id: String,
    block: BlockId,
    kind: MarkdownAnchorKind,
}

impl MarkdownAnchor {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn kind(&self) -> MarkdownAnchorKind {
        self.kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownLink {
    block: BlockId,
    url: String,
    fragment: Option<String>,
    target: Option<BlockId>,
}

impl MarkdownLink {
    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }

    pub const fn target(&self) -> Option<BlockId> {
        self.target
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownSearchMatch {
    range: Range<usize>,
    block: BlockId,
}

impl MarkdownSearchMatch {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }
}

/// 大纲、查找、链接和阅读锚点共享的一次性 AST 投影。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownDocument {
    blocks: Vec<MarkdownBlock>,
    headings: Vec<MarkdownHeading>,
    anchors: Vec<MarkdownAnchor>,
    links: Vec<MarkdownLink>,
    visible_text: String,
}

impl MarkdownDocument {
    pub fn parse(source: &str) -> Self {
        let arena = Arena::new();
        let mut options = markdown_options();
        let mut root = comrak::parse_document(&arena, source, &options);
        let invalid_front_matter = root.children().any(|node| match &node.data.borrow().value {
            NodeValue::FrontMatter(content) => !valid_front_matter(content),
            _ => false,
        });
        if invalid_front_matter {
            // front matter 只是可选解释；失败必须关闭该扩展并从原文重新解析。
            options.extension.front_matter_delimiter = None;
            root = comrak::parse_document(&arena, source, &options);
        }
        let mut projection = Projection::default();
        for node in root.children() {
            projection.block(source, node);
        }
        projection.finish()
    }

    pub fn blocks(&self) -> &[MarkdownBlock] {
        &self.blocks
    }

    pub fn headings(&self) -> &[MarkdownHeading] {
        &self.headings
    }

    /// 大纲只是标题表的只读视图，不重新遍历源码或 AST。
    pub fn outline(&self) -> &[MarkdownHeading] {
        self.headings()
    }

    pub fn anchors(&self) -> &[MarkdownAnchor] {
        &self.anchors
    }

    pub fn links(&self) -> &[MarkdownLink] {
        &self.links
    }

    pub fn visible_text(&self) -> &str {
        &self.visible_text
    }

    pub fn resolve_anchor(&self, anchor: &str) -> Option<BlockId> {
        let anchor = anchor.strip_prefix('#').unwrap_or(anchor);
        self.anchors
            .iter()
            .find(|candidate| candidate.id == anchor)
            .map(|anchor| anchor.block)
    }

    pub fn find_literal(&self, query: &str) -> Vec<MarkdownSearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        // 块按可见区间顺序排列：匹配扫描只前进块游标，避免每个匹配重扫全部块。
        let mut next_block = 0;
        self.visible_text
            .match_indices(query)
            .filter_map(|(start, matched)| {
                while self
                    .blocks
                    .get(next_block)
                    .is_some_and(|block| start >= block.visible_range.end)
                {
                    next_block += 1;
                }
                let block = self
                    .blocks
                    .get(next_block)
                    .filter(|block| block.visible_range.start <= start)?;
                Some(MarkdownSearchMatch {
                    range: start..start + matched.len(),
                    block: block.id,
                })
            })
            .collect()
    }
}

#[derive(Default)]
struct Projection {
    blocks: Vec<MarkdownBlock>,
    headings: Vec<MarkdownHeading>,
    anchors: Vec<MarkdownAnchor>,
    links: Vec<MarkdownLink>,
    visible_text: String,
    anchor_counts: HashMap<String, usize>,
}

impl Projection {
    fn block<'a>(&mut self, source: &str, node: &'a AstNode<'a>) {
        let id = BlockId(self.blocks.len());
        let source_range = source_range(source, node);
        let kind = block_kind(node);
        let structure = block_structure(source, node);
        let inline = if matches!(
            &kind,
            MarkdownBlockKind::Code(_)
                | MarkdownBlockKind::FrontMatter
                | MarkdownBlockKind::Html(_)
        ) {
            InlineProjection::default()
        } else {
            project_inlines(source, node)
        };
        let visible = match &kind {
            MarkdownBlockKind::Code(code) => code.source.clone(),
            MarkdownBlockKind::FrontMatter => String::new(),
            MarkdownBlockKind::Html(fragment) => fragment.visible_text(),
            _ => inline.visible.clone(),
        };
        if matches!(kind, MarkdownBlockKind::Heading(_)) {
            let base = anchor_base(&visible);
            let anchor = self.unique_anchor(base);
            self.anchors.push(MarkdownAnchor {
                id: anchor.clone(),
                block: id,
                kind: MarkdownAnchorKind::Heading,
            });
            self.headings.push(MarkdownHeading {
                text: visible.clone(),
                depth: heading_depth(node),
                block: id,
                anchor,
            });
        }
        if let NodeValue::FootnoteDefinition(definition) = &node.data.borrow().value {
            let anchor = format!("fn-{}", anchor_base(&definition.name));
            self.anchors.push(MarkdownAnchor {
                id: anchor,
                block: id,
                kind: MarkdownAnchorKind::Footnote,
            });
        }
        let mut html = InlineHtmlState::default();
        collect_links(node, id, &mut self.links, &mut html);
        if let MarkdownBlockKind::Html(fragment) = &kind {
            for url in fragment.links() {
                let fragment_name = url
                    .strip_prefix('#')
                    .filter(|fragment| !fragment.is_empty())
                    .map(str::to_owned);
                self.links.push(MarkdownLink {
                    block: id,
                    url: url.clone(),
                    fragment: fragment_name,
                    target: None,
                });
            }
        }
        if !visible.is_empty() && !self.visible_text.is_empty() {
            self.visible_text.push('\n');
        }
        let visible_start = self.visible_text.len();
        self.visible_text.push_str(&visible);
        let visible_range = visible_start..self.visible_text.len();
        self.blocks.push(MarkdownBlock {
            id,
            kind,
            inlines: if structure.is_some() {
                Vec::new()
            } else {
                inline.inlines
            },
            structure,
            source_range,
            visible_range,
        });
    }

    fn unique_anchor(&mut self, base: String) -> String {
        let count = self.anchor_counts.entry(base.clone()).or_insert(0);
        let anchor = if *count == 0 {
            base
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        anchor
    }

    fn finish(mut self) -> MarkdownDocument {
        for link in &mut self.links {
            link.target = link.fragment.as_deref().and_then(|fragment| {
                self.anchors
                    .iter()
                    .find(|anchor| anchor.id == fragment)
                    .map(|anchor| anchor.block)
            });
        }
        MarkdownDocument {
            blocks: self.blocks,
            headings: self.headings,
            anchors: self.anchors,
            links: self.links,
            visible_text: self.visible_text,
        }
    }
}

fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.alerts = true;
    options.extension.math_dollars = true;
    options.extension.front_matter_delimiter = Some("---".to_owned());
    options
}

fn valid_front_matter(content: &str) -> bool {
    const MAX_FRONT_MATTER_BYTES: usize = 64 * 1024;
    if content.len() > MAX_FRONT_MATTER_BYTES {
        return false;
    }
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let Some(opening_end) = content.find('\n') else {
        return false;
    };
    let Some(relative_end) = content[opening_end..].rfind("\n---") else {
        return false;
    };
    let yaml = &content[opening_end + 1..opening_end + relative_end];
    let mut validator = YamlValidator::default();
    yaml_rust2::parser::Parser::new_from_str(yaml)
        .load(&mut validator, true)
        .is_ok()
        && !validator.exceeded
}

#[derive(Default)]
struct YamlValidator {
    events: usize,
    depth: usize,
    exceeded: bool,
}

impl yaml_rust2::parser::EventReceiver for YamlValidator {
    fn on_event(&mut self, event: yaml_rust2::parser::Event) {
        const MAX_YAML_EVENTS: usize = 4096;
        const MAX_YAML_DEPTH: usize = 64;
        use yaml_rust2::parser::Event;
        self.events = self.events.saturating_add(1);
        match event {
            Event::MappingStart(..) | Event::SequenceStart(..) => {
                self.depth = self.depth.saturating_add(1);
            }
            Event::MappingEnd | Event::SequenceEnd => {
                self.depth = self.depth.saturating_sub(1);
            }
            _ => {}
        }
        // 只消费事件；别名不展开，标签不反序列化，正文不进入值对象。
        self.exceeded |= self.events > MAX_YAML_EVENTS || self.depth > MAX_YAML_DEPTH;
    }
}

fn block_kind<'a>(node: &'a AstNode<'a>) -> MarkdownBlockKind {
    match &node.data.borrow().value {
        NodeValue::Heading(heading) => MarkdownBlockKind::Heading(heading.level),
        NodeValue::Paragraph if display_math(node).is_some() => MarkdownBlockKind::Math,
        NodeValue::Paragraph => MarkdownBlockKind::Paragraph,
        NodeValue::List(_) | NodeValue::Item(_) | NodeValue::TaskItem(_) => MarkdownBlockKind::List,
        NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => MarkdownBlockKind::Quote,
        NodeValue::Alert(alert) => MarkdownBlockKind::Alert(match alert.alert_type {
            AlertType::Note => MarkdownAlertKind::Note,
            AlertType::Tip => MarkdownAlertKind::Tip,
            AlertType::Important => MarkdownAlertKind::Important,
            AlertType::Warning => MarkdownAlertKind::Warning,
            AlertType::Caution => MarkdownAlertKind::Caution,
        }),
        NodeValue::ThematicBreak => MarkdownBlockKind::ThematicBreak,
        NodeValue::CodeBlock(code) => {
            let alias = code.info.split_whitespace().next().unwrap_or_default();
            let language = LanguageRegistry::new()
                .for_markdown_alias(alias)
                .unwrap_or(LanguageId::PlainText);
            MarkdownBlockKind::Code(MarkdownCodeBlock {
                language,
                source: code.literal.clone(),
            })
        }
        NodeValue::Table(_) => MarkdownBlockKind::Table,
        NodeValue::FootnoteDefinition(_) => MarkdownBlockKind::Footnote,
        NodeValue::FrontMatter(_) => MarkdownBlockKind::FrontMatter,
        NodeValue::HtmlBlock(html) => {
            MarkdownBlockKind::Html(crate::markdown_html::project(&html.literal))
        }
        _ => MarkdownBlockKind::Paragraph,
    }
}

fn heading_depth(node: &AstNode<'_>) -> u8 {
    match &node.data.borrow().value {
        NodeValue::Heading(heading) => heading.level,
        _ => 0,
    }
}

fn display_math<'a>(node: &'a AstNode<'a>) -> Option<String> {
    let mut children = node.children();
    let child = children.next()?;
    if children.next().is_some() {
        return None;
    }
    match &child.data.borrow().value {
        NodeValue::Math(math) if math.display_math => Some(math.literal.clone()),
        _ => None,
    }
}

#[derive(Default)]
struct InlineHtmlState {
    suppressed: Vec<String>,
}

impl InlineHtmlState {
    /// 危险标签内的安全外观节点同样不可成为可见文本或链接事实。
    fn accepts(&mut self, tag: &crate::markdown_html::InlineHtml) -> bool {
        if tag.closing {
            if let Some(index) = self.suppressed.iter().rposition(|name| name == &tag.name) {
                self.suppressed.truncate(index);
            }
            false
        } else if tag.dangerous {
            self.suppressed.push(tag.name.clone());
            false
        } else {
            self.suppressed.is_empty()
        }
    }
}

#[derive(Default)]
struct InlineProjection {
    visible: String,
    inlines: Vec<MarkdownInline>,
}

#[derive(Default)]
struct RenderInlineState {
    html: InlineHtmlState,
    style: MarkdownStyle,
    html_styles: Vec<String>,
    html_links: Vec<String>,
    markdown_link: Option<String>,
}

impl RenderInlineState {
    fn current_url(&self) -> Option<String> {
        self.html_links
            .last()
            .cloned()
            .or_else(|| self.markdown_link.clone())
    }

    fn refresh_html_style(&mut self) {
        self.style.subscript = self.html_styles.iter().any(|name| name == "sub");
        self.style.superscript = self.html_styles.iter().any(|name| name == "sup");
        self.style.kbd = self.html_styles.iter().any(|name| name == "kbd");
    }
}

fn project_inlines<'a>(source: &str, node: &'a AstNode<'a>) -> InlineProjection {
    let mut projection = InlineProjection::default();
    let mut state = RenderInlineState::default();
    if let Some(math) = display_math(node) {
        push_inline(
            source,
            node,
            MarkdownInlineKind::Math,
            math.clone(),
            &state,
            &mut projection,
        );
        return projection;
    }
    project_inline_node(source, node, &mut state, &mut projection);
    projection
}

fn project_inline_node<'a>(
    source: &str,
    node: &'a AstNode<'a>,
    state: &mut RenderInlineState,
    projection: &mut InlineProjection,
) {
    if let NodeValue::HtmlInline(raw) = &node.data.borrow().value {
        let Some(tag) = crate::markdown_html::inline_tag(raw) else {
            return;
        };
        let accepted = state.html.accepts(&tag);
        if accepted && tag.line_break {
            push_inline(
                source,
                node,
                MarkdownInlineKind::LineBreak,
                "\n".to_owned(),
                state,
                projection,
            );
        }
        if tag.closing {
            if state.html.suppressed.is_empty() {
                if tag.name == "a" {
                    state.html_links.pop();
                }
                if let Some(index) = state.html_styles.iter().rposition(|name| name == &tag.name) {
                    state.html_styles.truncate(index);
                    state.refresh_html_style();
                }
            }
            return;
        }
        if !accepted {
            return;
        }
        if matches!(tag.name.as_str(), "sub" | "sup" | "kbd") {
            state.html_styles.push(tag.name);
            state.refresh_html_style();
        }
        if let Some(url) = tag.link {
            state.html_links.push(url);
        }
        return;
    }
    if !state.html.suppressed.is_empty() {
        return;
    }
    match &node.data.borrow().value {
        NodeValue::Text(text) => push_inline(
            source,
            node,
            MarkdownInlineKind::Text,
            text.to_string(),
            state,
            projection,
        ),
        NodeValue::Code(code) => push_inline(
            source,
            node,
            MarkdownInlineKind::Code,
            code.literal.clone(),
            state,
            projection,
        ),
        NodeValue::Math(math) => push_inline(
            source,
            node,
            MarkdownInlineKind::Math,
            math.literal.clone(),
            state,
            projection,
        ),
        NodeValue::SoftBreak | NodeValue::LineBreak => push_inline(
            source,
            node,
            MarkdownInlineKind::LineBreak,
            "\n".to_owned(),
            state,
            projection,
        ),
        NodeValue::Image(image) => {
            let mut alt = String::new();
            for child in node.children() {
                collect_visible_text(child, &mut alt, &mut InlineHtmlState::default());
            }
            let start = projection.visible.len();
            projection.inlines.push(MarkdownInline {
                kind: MarkdownInlineKind::Image,
                text: alt,
                style: state.style,
                url: Some(image.url.clone()),
                source_range: source_range(source, node),
                visible_range: start..start,
            });
            return;
        }
        NodeValue::FootnoteReference(reference) => {
            let start = projection.visible.len();
            projection.inlines.push(MarkdownInline {
                kind: MarkdownInlineKind::FootnoteReference,
                text: reference.name.clone(),
                style: state.style,
                url: Some(format!("#fn-{}", anchor_base(&reference.name))),
                source_range: source_range(source, node),
                visible_range: start..start,
            });
            return;
        }
        NodeValue::Link(link) => {
            let previous = state.markdown_link.replace(link.url.clone());
            for child in node.children() {
                project_inline_node(source, child, state, projection);
            }
            state.markdown_link = previous;
            return;
        }
        NodeValue::Emph | NodeValue::Strong | NodeValue::Strikethrough => {
            let field = match &node.data.borrow().value {
                NodeValue::Emph => &mut state.style.emphasis,
                NodeValue::Strong => &mut state.style.strong,
                NodeValue::Strikethrough => &mut state.style.strikethrough,
                _ => unreachable!(),
            };
            let previous = *field;
            *field = true;
            for child in node.children() {
                project_inline_node(source, child, state, projection);
            }
            match &node.data.borrow().value {
                NodeValue::Emph => state.style.emphasis = previous,
                NodeValue::Strong => state.style.strong = previous,
                NodeValue::Strikethrough => state.style.strikethrough = previous,
                _ => unreachable!(),
            }
            return;
        }
        _ => {}
    }
    for child in node.children() {
        project_inline_node(source, child, state, projection);
    }
}

fn block_structure<'a>(source: &str, node: &'a AstNode<'a>) -> Option<MarkdownStructure> {
    match &node.data.borrow().value {
        NodeValue::List(_) => Some(MarkdownStructure::List(build_list(source, node, 0))),
        NodeValue::Table(table) => {
            let alignments = table
                .alignments
                .iter()
                .map(|alignment| match alignment {
                    comrak::nodes::TableAlignment::None => MarkdownAlignment::None,
                    comrak::nodes::TableAlignment::Left => MarkdownAlignment::Left,
                    comrak::nodes::TableAlignment::Right => MarkdownAlignment::Right,
                    comrak::nodes::TableAlignment::Center => MarkdownAlignment::Center,
                })
                .collect();
            let rows = node
                .children()
                .filter_map(|row| {
                    let header = match &row.data.borrow().value {
                        NodeValue::TableRow(header) => *header,
                        _ => return None,
                    };
                    let cells = row
                        .children()
                        .map(|cell| MarkdownTableCell {
                            inlines: project_inlines(source, cell).inlines,
                        })
                        .collect();
                    Some(MarkdownTableRow { header, cells })
                })
                .collect();
            Some(MarkdownStructure::Table(MarkdownTable { alignments, rows }))
        }
        _ => None,
    }
}

fn build_list<'a>(source: &str, list: &'a AstNode<'a>, depth: usize) -> MarkdownList {
    let NodeValue::List(metadata) = &list.data.borrow().value else {
        return MarkdownList { items: Vec::new() };
    };
    let items = list
        .children()
        .enumerate()
        .map(|(index, item)| {
            let checked = match &item.data.borrow().value {
                NodeValue::TaskItem(task) => Some(task.symbol.is_some()),
                _ => None,
            };
            let ordinal = (metadata.list_type == comrak::nodes::ListType::Ordered)
                .then_some(metadata.start + index);
            let mut inlines = Vec::new();
            let mut children = Vec::new();
            let mut visible_offset = 0;
            for child in item.children() {
                if matches!(child.data.borrow().value, NodeValue::List(_)) {
                    children.extend(build_list(source, child, depth + 1).items);
                    continue;
                }
                let mut projected = project_inlines(source, child);
                for inline in &mut projected.inlines {
                    inline.visible_range.start += visible_offset;
                    inline.visible_range.end += visible_offset;
                }
                visible_offset += projected.visible.len();
                inlines.extend(projected.inlines);
            }
            MarkdownListItem {
                ordinal,
                checked,
                depth,
                inlines,
                children,
            }
        })
        .collect();
    MarkdownList { items }
}

fn push_inline(
    source: &str,
    node: &AstNode<'_>,
    kind: MarkdownInlineKind,
    text: String,
    state: &RenderInlineState,
    projection: &mut InlineProjection,
) {
    let start = projection.visible.len();
    projection.visible.push_str(&text);
    let end = projection.visible.len();
    projection.inlines.push(MarkdownInline {
        kind,
        text,
        style: state.style,
        url: state.current_url(),
        source_range: source_range(source, node),
        visible_range: start..end,
    });
}

fn collect_visible_text<'a>(
    node: &'a AstNode<'a>,
    output: &mut String,
    html: &mut InlineHtmlState,
) {
    if let NodeValue::HtmlInline(raw) = &node.data.borrow().value {
        if let Some(tag) = crate::markdown_html::inline_tag(raw)
            && html.accepts(&tag)
            && tag.line_break
        {
            output.push('\n');
        }
        return;
    }
    if !html.suppressed.is_empty() {
        return;
    }
    match &node.data.borrow().value {
        NodeValue::Text(text) => output.push_str(text),
        NodeValue::Code(code) => output.push_str(&code.literal),
        NodeValue::Math(math) => output.push_str(&math.literal),
        NodeValue::SoftBreak | NodeValue::LineBreak => output.push('\n'),
        // 图片 alt、URL、原始 HTML 和脚注标记不是普通可见正文查找事实。
        NodeValue::Image(_) | NodeValue::HtmlInline(_) | NodeValue::FootnoteReference(_) => {
            return;
        }
        _ => {}
    }
    for child in node.children() {
        collect_visible_text(child, output, html);
    }
}

fn collect_links<'a>(
    node: &'a AstNode<'a>,
    block: BlockId,
    links: &mut Vec<MarkdownLink>,
    html: &mut InlineHtmlState,
) {
    if let NodeValue::HtmlInline(raw) = &node.data.borrow().value {
        if let Some(tag) = crate::markdown_html::inline_tag(raw)
            && html.accepts(&tag)
            && let Some(url) = tag.link
        {
            let fragment = url
                .strip_prefix('#')
                .filter(|fragment| !fragment.is_empty())
                .map(str::to_owned);
            links.push(MarkdownLink {
                block,
                url,
                fragment,
                target: None,
            });
        }
    } else {
        if !html.suppressed.is_empty() {
            return;
        }
        if let NodeValue::Link(link) = &node.data.borrow().value {
            let fragment = link
                .url
                .strip_prefix('#')
                .filter(|fragment| !fragment.is_empty())
                .map(str::to_owned);
            links.push(MarkdownLink {
                block,
                url: link.url.clone(),
                fragment,
                target: None,
            });
        }
        if let NodeValue::FootnoteReference(reference) = &node.data.borrow().value {
            let fragment = format!("fn-{}", anchor_base(&reference.name));
            links.push(MarkdownLink {
                block,
                url: format!("#{fragment}"),
                fragment: Some(fragment),
                target: None,
            });
        }
    }
    for child in node.children() {
        collect_links(child, block, links, html);
    }
}

fn source_range(source: &str, node: &AstNode<'_>) -> Range<usize> {
    let position = node.data.borrow().sourcepos;
    let start = source_position(source, position.start.line, position.start.column, false);
    let end = source_position(source, position.end.line, position.end.column, true);
    match (start, end) {
        (Some(start), Some(end)) if start <= end => start..end,
        _ => 0..0,
    }
}

fn source_position(source: &str, line: usize, column: usize, end: bool) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let line_start = source
        .split_inclusive('\n')
        .take(line - 1)
        .map(str::len)
        .sum::<usize>();
    let offset = line_start + column - 1 + usize::from(end);
    (offset <= source.len() && source.is_char_boundary(offset)).then_some(offset)
}

fn anchor_base(text: &str) -> String {
    let mut anchor = String::new();
    let mut separator = false;
    for character in text.nfkc().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() || matches!(character, '_' | '-') {
            if separator && !anchor.is_empty() {
                anchor.push('-');
            }
            separator = false;
            anchor.push(character);
        } else if character.is_whitespace() {
            separator = true;
        }
    }
    if anchor.is_empty() {
        "section".to_owned()
    } else {
        anchor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LanguageId;

    #[test]
    fn front_matter_is_validated_and_invalid_yaml_restarts_as_markdown() {
        let valid = MarkdownDocument::parse("---\ntitle: Test\n---\n# Body\n");
        assert!(matches!(
            valid.blocks()[0].kind(),
            MarkdownBlockKind::FrontMatter
        ));
        assert!(valid.find_literal("title").is_empty());
        assert_eq!(valid.headings()[0].anchor(), "body");

        for source in [
            "---\ntitle: [broken\n---\n# Body\n".to_owned(),
            format!("---\n{}\n---\n# Body\n", "x".repeat(65_537)),
            format!(
                "---\n{}---\n# Body\n",
                (0..4100)
                    .map(|index| format!("key{index}: value\n"))
                    .collect::<String>()
            ),
            format!(
                "---\n{}---\n# Body\n",
                (0..80)
                    .map(|depth| format!("{}key:\n", "  ".repeat(depth)))
                    .collect::<String>()
            ),
        ] {
            let invalid = MarkdownDocument::parse(&source);
            assert!(
                invalid
                    .blocks()
                    .iter()
                    .all(|block| !matches!(block.kind(), MarkdownBlockKind::FrontMatter)),
                "非法或超限 YAML 不得保留 front matter 语义"
            );
            assert_eq!(invalid.headings().last().unwrap().anchor(), "body");
        }
        assert!(
            MarkdownDocument::parse("---\ntitle: [broken\n---\n# Body\n")
                .visible_text()
                .contains("title: [broken")
        );
    }

    #[test]
    fn gfm_alerts_footnotes_math_and_fallbacks_share_the_projection() {
        let document = MarkdownDocument::parse(
            "| A |\n| - |\n| ~~x~~ |\n\n- [x] done\n\n> [!NOTE]\n> safe\n\n> [!UNKNOWN]\n> ordinary\n\nFootnote[^ok] and missing[^none].\n\n[^ok]: note text\n\n$$\nx^2\n$$\n\nPrice $5 and $6.\n",
        );
        for expected in [
            MarkdownBlockKind::Table,
            MarkdownBlockKind::List,
            MarkdownBlockKind::Alert(MarkdownAlertKind::Note),
            MarkdownBlockKind::Quote,
            MarkdownBlockKind::Footnote,
            MarkdownBlockKind::Math,
        ] {
            assert!(
                document
                    .blocks()
                    .iter()
                    .any(|block| block.kind() == &expected)
            );
        }
        assert!(document.visible_text().contains("note text"));
        assert!(document.visible_text().contains("missing[^none]"));
        assert!(document.visible_text().contains("Price $5 and $6."));
        let note = document
            .anchors()
            .iter()
            .find(|anchor| anchor.kind() == MarkdownAnchorKind::Footnote)
            .unwrap();
        assert!(
            document
                .links()
                .iter()
                .any(|link| link.target() == Some(note.block()))
        );
    }

    #[test]
    fn anchors_normalize_unicode_and_allocate_stable_duplicate_suffixes() {
        let document = MarkdownDocument::parse("# Ｈｅｌｌｏ　世界！\n\n# !!!\n\n# !!!\n");
        let anchors = document
            .headings()
            .iter()
            .map(|heading| heading.anchor())
            .collect::<Vec<_>>();
        assert_eq!(anchors, ["hello-世界", "section", "section-1"]);
    }

    #[test]
    fn source_ranges_preserve_unicode_and_multiline_block_boundaries() {
        let source = "# 你好\n\n```rust\nfn main() {}\n```\n";
        let document = MarkdownDocument::parse(source);
        let ranges = document
            .blocks()
            .iter()
            .map(|block| block.source_range())
            .collect::<Vec<_>>();
        assert_eq!(ranges, [0..8, 10..34]);
        assert_eq!(&source[ranges[0].clone()], "# 你好");
        assert_eq!(&source[ranges[1].clone()], "```rust\nfn main() {}\n```");
        assert_eq!(document.find_literal("你好")[0].block(), BlockId(0));
    }

    #[test]
    fn unsafe_html_is_projected_to_bounded_whitelisted_nodes() {
        let document = MarkdownDocument::parse(
            "<section><script>alert(1)</script><a href=\"https://safe.example\" onclick=\"alert(2)\">Safe</a><a href=\"javascript:alert(3)\">Rejected link</a><img src=\"https://images.example/x.png\" alt=\"Hidden alt\"><kbd>Ctrl</kbd></section>\n",
        );
        let fragment = document
            .blocks()
            .iter()
            .find_map(|block| match block.kind() {
                MarkdownBlockKind::Html(fragment) => Some(fragment),
                _ => None,
            });
        assert!(fragment.is_some());
        assert!(document.visible_text().contains("Safe"));
        assert!(document.visible_text().contains("Rejected link"));
        assert!(document.visible_text().contains("Ctrl"));
        for hidden in [
            "alert(1)",
            "alert(2)",
            "alert(3)",
            "Hidden alt",
            "safe.example",
            "<section>",
            "onclick",
        ] {
            assert!(
                !document.visible_text().contains(hidden),
                "危险或原始 HTML 不得进入查找: {hidden}"
            );
        }
        assert_eq!(
            document
                .links()
                .iter()
                .map(|link| link.url())
                .collect::<Vec<_>>(),
            ["https://safe.example"]
        );

        let inline = MarkdownDocument::parse(
            "Before <script>inline_alert()<a href=\"https://hidden.example\">Hidden link</a></script><kbd>Ctrl</kbd> after\n",
        );
        assert!(
            crate::markdown_html::inline_tag("<script>")
                .unwrap()
                .dangerous
        );
        assert!(
            crate::markdown_html::inline_tag("</script>")
                .unwrap()
                .closing
        );
        assert!(inline.visible_text().contains("Before"));
        assert!(inline.visible_text().contains("Ctrl"));
        assert!(inline.visible_text().contains("after"));
        assert!(!inline.visible_text().contains("inline_alert"));
        assert!(!inline.visible_text().contains("Hidden link"));
        assert!(inline.links().is_empty());
    }

    #[test]
    fn oversized_html_fragment_degrades_without_touching_other_blocks() {
        let attributes = (0..17)
            .map(|index| format!("data-budget-{index}=\"x\""))
            .collect::<Vec<_>>()
            .join(" ");
        for (budget, source) in [
            ("bytes", format!("<div>\n{}\n</div>", "x".repeat(64 * 1024))),
            ("nodes", "<div>".repeat(4097)),
            (
                "text_nodes",
                "<div>".to_owned() + &"x<!---->".repeat(4097) + "</div>",
            ),
            ("depth", "<div>".repeat(80)),
            ("attributes", format!("<div {attributes}>safe</div>")),
        ] {
            let source = format!("{source}\n\n# Safe\n");
            let document = MarkdownDocument::parse(&source);
            assert!(document.headings()[0].anchor() == "safe");
            assert!(!document.visible_text().contains("<div>"));
            assert!(
                document.blocks().iter().any(|block| match block.kind() {
                    MarkdownBlockKind::Html(fragment) => fragment.is_degraded(),
                    _ => false,
                }),
                "{budget} 超限片段必须携带明确降级事实"
            );
        }
    }

    #[test]
    fn inline_projection_shares_styles_images_math_and_visible_ranges() {
        let source = "Plain **strong *both*** ~~gone~~ `code` $x^2$ ![alt](image.png) [link](https://example.com) <sub>lower</sub><sup>upper</sup><kbd>Ctrl</kbd>\n";
        let document = MarkdownDocument::parse(source);
        let inline = document.blocks()[0].inlines();
        assert!(inline.iter().any(|run| {
            run.text().trim() == "strong" && run.style().strong && !run.style().emphasis
        }));
        assert!(
            inline
                .iter()
                .any(|run| { run.text() == "both" && run.style().strong && run.style().emphasis })
        );
        assert!(
            inline
                .iter()
                .any(|run| { run.text() == "gone" && run.style().strikethrough })
        );
        assert!(
            inline
                .iter()
                .any(|run| run.kind() == MarkdownInlineKind::Code)
        );
        let math = inline
            .iter()
            .find(|run| run.kind() == MarkdownInlineKind::Math)
            .unwrap();
        assert_eq!(math.text(), "x^2");
        assert_eq!(&document.visible_text()[math.visible_range()], "x^2");
        let image = inline
            .iter()
            .find(|run| run.kind() == MarkdownInlineKind::Image)
            .unwrap();
        assert_eq!(image.text(), "alt");
        assert_eq!(image.url(), Some("image.png"));
        assert!(image.visible_range().is_empty());
        assert!(document.find_literal("alt").is_empty());
        assert!(
            inline
                .iter()
                .any(|run| { run.text() == "link" && run.url() == Some("https://example.com") })
        );
        for (text, expected) in [("lower", "sub"), ("upper", "sup"), ("Ctrl", "kbd")] {
            let run = inline.iter().find(|run| run.text() == text).unwrap();
            let style = run.style();
            assert_eq!(
                (style.subscript, style.superscript, style.kbd),
                (expected == "sub", expected == "sup", expected == "kbd")
            );
        }
    }

    #[test]
    fn list_and_table_structures_share_the_same_ast_projection() {
        let document = MarkdownDocument::parse(
            "3. first\n4. second\n\n- [ ] parent\n  - [x] child\n\n| Left | Right |\n|:---|---:|\n| a | **b** |\n",
        );
        let lists = document
            .blocks()
            .iter()
            .filter_map(|block| match block.structure() {
                Some(MarkdownStructure::List(list)) => Some(list),
                _ => None,
            })
            .collect::<Vec<_>>();
        let ordered = lists[0].items();
        assert_eq!(ordered[0].ordinal(), Some(3));
        assert_eq!(ordered[1].ordinal(), Some(4));
        let parent = &lists[1].items()[0];
        assert_eq!(parent.checked(), Some(false));
        assert!(parent.inlines().iter().any(|run| run.text() == "parent"));
        let child = &parent.children()[0];
        assert_eq!(child.checked(), Some(true));
        assert_eq!(child.depth(), 1);
        assert!(child.inlines().iter().any(|run| run.text() == "child"));

        let table = document
            .blocks()
            .iter()
            .find_map(|block| match block.structure() {
                Some(MarkdownStructure::Table(table)) => Some(table),
                _ => None,
            });
        let table = table.unwrap();
        assert_eq!(
            table.alignments(),
            [MarkdownAlignment::Left, MarkdownAlignment::Right]
        );
        assert!(table.rows()[0].header());
        assert!(
            table.rows()[1].cells()[1]
                .inlines()
                .iter()
                .any(|run| run.text() == "b" && run.style().strong)
        );
    }

    #[test]
    fn one_projection_drives_outline_visible_search_links_and_mermaid_code() {
        let document = MarkdownDocument::parse(
            "# Hello\n\n# Hello\n\nVisible [guide](https://example.com) and [next](#hello-1).\n\n```mermaid\ngraph TD; A-->B\n```\n",
        );

        let headings = document.headings();
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].anchor(), "hello");
        assert_eq!(headings[1].anchor(), "hello-1");
        assert_eq!(document.outline(), headings);
        assert_eq!(
            document.resolve_anchor("hello-1"),
            Some(headings[1].block())
        );

        let links = document.links();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url(), "https://example.com");
        assert_eq!(links[1].fragment(), Some("hello-1"));
        assert_eq!(links[1].target(), Some(headings[1].block()));

        assert_eq!(document.find_literal("Visible").len(), 1);
        assert!(document.find_literal("example.com").is_empty());
        assert!(document.find_literal("# Hello").is_empty());
        assert_eq!(document.find_literal("graph TD").len(), 1);

        let code = document
            .blocks()
            .iter()
            .find_map(|block| match block.kind() {
                MarkdownBlockKind::Code(code) => Some(code),
                _ => None,
            });
        assert_eq!(code.unwrap().language(), LanguageId::Mermaid);
        assert_eq!(code.unwrap().source(), "graph TD; A-->B\n");
    }

    #[test]
    fn literal_search_maps_each_match_to_its_own_block() {
        let source = (0..12)
            .map(|index| format!("needle {index}\n\n"))
            .collect::<String>();
        let document = MarkdownDocument::parse(&source);
        let matches = document.find_literal("needle");
        assert_eq!(matches.len(), 12);
        for (index, matched) in matches.iter().enumerate() {
            assert_eq!(matched.block(), BlockId(index));
        }
    }
}
