//! 有界 HTML TreeSink 与白名单投影；不输出 HTML，也不执行资源加载。

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use html5ever::tendril::{StrTendril, TendrilSink as _};
use html5ever::tree_builder::{Attribute, ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{ExpandedName, LocalName, Namespace, QualName};

const MAX_HTML_BYTES: usize = 64 * 1024;
const MAX_HTML_NODES: usize = 4096;
const MAX_HTML_DEPTH: usize = 64;
const MAX_HTML_ATTRIBUTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownHtmlTag {
    Br,
    Hr,
    Sub,
    Sup,
    Kbd,
    A,
    Img,
    Picture,
    Source,
    Details,
    Summary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownHtmlAttribute {
    Href(String),
    Src(String),
    SrcSet(Vec<String>),
    Alt(String),
    Title(String),
    Width(u32),
    Height(u32),
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownHtmlElement {
    tag: MarkdownHtmlTag,
    attributes: Vec<MarkdownHtmlAttribute>,
    children: Vec<MarkdownHtmlNode>,
}

impl MarkdownHtmlElement {
    pub const fn tag(&self) -> MarkdownHtmlTag {
        self.tag
    }

    pub fn attributes(&self) -> &[MarkdownHtmlAttribute] {
        &self.attributes
    }

    pub fn children(&self) -> &[MarkdownHtmlNode] {
        &self.children
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownHtmlNode {
    Text(String),
    Element(MarkdownHtmlElement),
    /// 未知标签只保留安全子内容；占位不携带原始标签或属性。
    Placeholder,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownHtmlFragment {
    nodes: Vec<MarkdownHtmlNode>,
    degraded: bool,
    links: Vec<String>,
}

impl MarkdownHtmlFragment {
    pub fn nodes(&self) -> &[MarkdownHtmlNode] {
        &self.nodes
    }

    pub const fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub(crate) fn visible_text(&self) -> String {
        let mut text = String::new();
        for node in &self.nodes {
            visible_node_text(node, &mut text);
        }
        text
    }

    pub(crate) fn links(&self) -> &[String] {
        &self.links
    }
}

pub(crate) struct InlineHtml {
    pub(crate) name: String,
    pub(crate) closing: bool,
    pub(crate) dangerous: bool,
    pub(crate) line_break: bool,
    pub(crate) link: Option<String>,
}

pub(crate) fn project(source: &str) -> MarkdownHtmlFragment {
    let parsed = parse(source, HtmlSink::new());
    let mut state = SanitizeState {
        degraded: parsed.exceeded,
    };
    let nodes = sanitize_children(&parsed.document, 0, &mut state);
    let mut links = Vec::new();
    for node in &nodes {
        collect_node_links(node, &mut links);
    }
    MarkdownHtmlFragment {
        nodes,
        degraded: state.degraded,
        links,
    }
}

/// 行内标签仍先经 TreeSink；闭合标签只提取用于取消抑制的封闭名称。
pub(crate) fn inline_tag(source: &str) -> Option<InlineHtml> {
    let trimmed = source.trim();
    if let Some(rest) = trimmed.strip_prefix("</") {
        let name = rest
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        return (!name.is_empty()).then(|| InlineHtml {
            dangerous: dangerous_tag(&name),
            name,
            closing: true,
            line_break: false,
            link: None,
        });
    }
    if !trimmed.starts_with('<') {
        return None;
    }
    let parsed = parse(trimmed, HtmlSink::new());
    let element = deepest_element(&parsed.document)?;
    let InternalKind::Element { name, .. } = &element.0.kind else {
        return None;
    };
    let name = name.local.to_string().to_ascii_lowercase();
    let tag = allowed_tag(&name);
    let link = if tag == Some(MarkdownHtmlTag::A) {
        let InternalKind::Element { attributes, .. } = &element.0.kind else {
            unreachable!()
        };
        sanitize_attributes(tag.unwrap(), &attributes.borrow())
            .into_iter()
            .find_map(|attribute| match attribute {
                MarkdownHtmlAttribute::Href(url) => Some(url),
                _ => None,
            })
    } else {
        None
    };
    Some(InlineHtml {
        dangerous: dangerous_tag(&name),
        line_break: matches!(tag, Some(MarkdownHtmlTag::Br | MarkdownHtmlTag::Hr)),
        name,
        closing: false,
        link,
    })
}

#[derive(Clone)]
struct Handle(Rc<InternalNode>);

struct InternalNode {
    kind: InternalKind,
    parent: RefCell<Weak<InternalNode>>,
    children: RefCell<Vec<Handle>>,
}

enum InternalKind {
    Document,
    Element {
        name: QualName,
        attributes: RefCell<Vec<Attribute>>,
    },
    Text(RefCell<String>),
    Ignored,
}

struct ParsedHtml {
    document: Handle,
    exceeded: bool,
}

struct HtmlSink {
    document: Handle,
    nodes: usize,
    exceeded: bool,
}

impl HtmlSink {
    fn new() -> Self {
        Self {
            document: Handle::new(InternalKind::Document),
            nodes: 0,
            exceeded: false,
        }
    }

    fn node(&mut self, kind: InternalKind) -> Handle {
        self.nodes = self.nodes.saturating_add(1);
        self.exceeded |= self.nodes > MAX_HTML_NODES;
        Handle::new(kind)
    }

    fn attach(&mut self, parent: &Handle, child: Handle) {
        self.attach_at(parent, child, None);
    }

    fn attach_at(&mut self, parent: &Handle, child: Handle, index: Option<usize>) {
        if self.nodes > MAX_HTML_NODES || handle_depth(parent) >= MAX_HTML_DEPTH {
            self.exceeded = true;
            return;
        }
        *child.0.parent.borrow_mut() = Rc::downgrade(&parent.0);
        let mut children = parent.0.children.borrow_mut();
        match index {
            Some(index) => {
                let index = index.min(children.len());
                children.insert(index, child);
            }
            None => children.push(child),
        }
    }

    fn append_text(&mut self, parent: &Handle, text: String) {
        if let Some(last) = parent.0.children.borrow().last().cloned()
            && let InternalKind::Text(previous) = &last.0.kind
        {
            previous.borrow_mut().push_str(&text);
            return;
        }
        let text = self.node(InternalKind::Text(text.into()));
        self.attach(parent, text);
    }
}

impl Handle {
    fn new(kind: InternalKind) -> Self {
        Self(Rc::new(InternalNode {
            kind,
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(Vec::new()),
        }))
    }
}

impl TreeSink for HtmlSink {
    type Handle = Handle;
    type Output = ParsedHtml;

    fn finish(self) -> Self::Output {
        ParsedHtml {
            document: self.document,
            exceeded: self.exceeded,
        }
    }

    fn parse_error(&mut self, _message: Cow<'static, str>) {}

    fn get_document(&mut self) -> Self::Handle {
        self.document.clone()
    }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> ExpandedName<'a> {
        let InternalKind::Element { name, .. } = &target.0.kind else {
            panic!("html5ever 请求了非元素名称")
        };
        name.expanded()
    }

    fn create_element(
        &mut self,
        name: QualName,
        attributes: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> Self::Handle {
        self.exceeded |= attributes.len() > MAX_HTML_ATTRIBUTES;
        self.node(InternalKind::Element {
            name,
            attributes: attributes.into(),
        })
    }

    fn create_comment(&mut self, _text: StrTendril) -> Self::Handle {
        self.node(InternalKind::Ignored)
    }

    fn create_pi(&mut self, _target: StrTendril, _data: StrTendril) -> Self::Handle {
        self.node(InternalKind::Ignored)
    }

    fn append(&mut self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        match child {
            NodeOrText::AppendNode(node) => self.attach(parent, node),
            NodeOrText::AppendText(text) => self.append_text(parent, text.to_string()),
        }
    }

    fn append_based_on_parent_node(
        &mut self,
        element: &Self::Handle,
        _previous: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        self.append(element, child);
    }

    fn append_doctype_to_document(
        &mut self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
    }

    fn get_template_contents(&mut self, target: &Self::Handle) -> Self::Handle {
        target.clone()
    }

    fn same_node(&self, left: &Self::Handle, right: &Self::Handle) -> bool {
        Rc::ptr_eq(&left.0, &right.0)
    }

    fn set_quirks_mode(&mut self, _mode: QuirksMode) {}

    fn append_before_sibling(
        &mut self,
        sibling: &Self::Handle,
        new_node: NodeOrText<Self::Handle>,
    ) {
        let parent = sibling.0.parent.borrow().upgrade().map(Handle);
        let Some(parent) = parent else { return };
        let index = parent
            .0
            .children
            .borrow()
            .iter()
            .position(|child| self.same_node(child, sibling));
        match new_node {
            NodeOrText::AppendNode(node) => self.attach_at(&parent, node, index),
            NodeOrText::AppendText(text) => {
                let node = self.node(InternalKind::Text(text.to_string().into()));
                self.attach_at(&parent, node, index);
            }
        }
    }

    fn add_attrs_if_missing(&mut self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let InternalKind::Element { attributes, .. } = &target.0.kind else {
            return;
        };
        let mut attributes = attributes.borrow_mut();
        for attribute in attrs {
            if !attributes
                .iter()
                .any(|existing| existing.name == attribute.name)
            {
                attributes.push(attribute);
            }
        }
        self.exceeded |= attributes.len() > MAX_HTML_ATTRIBUTES;
    }

    fn remove_from_parent(&mut self, target: &Self::Handle) {
        let parent = target.0.parent.borrow().upgrade().map(Handle);
        if let Some(parent) = parent {
            parent
                .0
                .children
                .borrow_mut()
                .retain(|child| !self.same_node(child, target));
        }
    }

    fn reparent_children(&mut self, node: &Self::Handle, new_parent: &Self::Handle) {
        let children = std::mem::take(&mut *node.0.children.borrow_mut());
        for child in children {
            self.attach(new_parent, child);
        }
    }
}

fn parse(source: &str, sink: HtmlSink) -> ParsedHtml {
    let truncated = source.len() > MAX_HTML_BYTES;
    let bounded = if truncated {
        &source[..source.floor_char_boundary(MAX_HTML_BYTES)]
    } else {
        source
    };
    let mut parsed = html5ever::driver::parse_fragment(
        sink,
        html5ever::ParseOpts::default(),
        QualName::new(
            None,
            Namespace::from("http://www.w3.org/1999/xhtml"),
            LocalName::from("div"),
        ),
        Vec::new(),
    )
    .one(bounded);
    parsed.exceeded |= truncated;
    parsed
}

fn handle_depth(handle: &Handle) -> usize {
    let mut depth = 0;
    let mut current = handle.0.parent.borrow().upgrade();
    while let Some(parent) = current {
        depth += 1;
        current = parent.parent.borrow().upgrade();
    }
    depth
}

#[derive(Default)]
struct SanitizeState {
    degraded: bool,
}

fn sanitize_children(
    parent: &Handle,
    depth: usize,
    state: &mut SanitizeState,
) -> Vec<MarkdownHtmlNode> {
    if depth >= MAX_HTML_DEPTH {
        state.degraded = true;
        return vec![MarkdownHtmlNode::Placeholder];
    }
    let children = parent.0.children.borrow().clone();
    let mut output = Vec::new();
    for child in children {
        match &child.0.kind {
            InternalKind::Document | InternalKind::Ignored => {}
            InternalKind::Text(text) => {
                let text = text.borrow();
                if !text.is_empty() {
                    output.push(MarkdownHtmlNode::Text(text.clone()));
                }
            }
            InternalKind::Element { name, attributes } => {
                let name = name.local.to_string().to_ascii_lowercase();
                if dangerous_tag(&name) {
                    continue;
                }
                let children = sanitize_children(&child, depth + 1, state);
                if let Some(tag) = allowed_tag(&name) {
                    output.push(MarkdownHtmlNode::Element(MarkdownHtmlElement {
                        tag,
                        attributes: sanitize_attributes(tag, &attributes.borrow()),
                        children,
                    }));
                } else {
                    output.push(MarkdownHtmlNode::Placeholder);
                    output.extend(children);
                }
            }
        }
    }
    output
}

fn allowed_tag(name: &str) -> Option<MarkdownHtmlTag> {
    Some(match name {
        "br" => MarkdownHtmlTag::Br,
        "hr" => MarkdownHtmlTag::Hr,
        "sub" => MarkdownHtmlTag::Sub,
        "sup" => MarkdownHtmlTag::Sup,
        "kbd" => MarkdownHtmlTag::Kbd,
        "a" => MarkdownHtmlTag::A,
        "img" => MarkdownHtmlTag::Img,
        "picture" => MarkdownHtmlTag::Picture,
        "source" => MarkdownHtmlTag::Source,
        "details" => MarkdownHtmlTag::Details,
        "summary" => MarkdownHtmlTag::Summary,
        _ => return None,
    })
}

fn dangerous_tag(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "style"
            | "iframe"
            | "frame"
            | "frameset"
            | "object"
            | "embed"
            | "applet"
            | "form"
            | "input"
            | "button"
            | "textarea"
            | "select"
            | "option"
            | "link"
            | "meta"
            | "base"
            | "template"
            | "svg"
            | "math"
    )
}

fn sanitize_attributes(
    tag: MarkdownHtmlTag,
    attributes: &[Attribute],
) -> Vec<MarkdownHtmlAttribute> {
    let mut output = Vec::new();
    for attribute in attributes.iter().take(MAX_HTML_ATTRIBUTES) {
        let name = attribute.name.local.to_string().to_ascii_lowercase();
        let value = attribute.value.to_string();
        let projected = match (tag, name.as_str()) {
            (_, "title") => Some(MarkdownHtmlAttribute::Title(value)),
            (MarkdownHtmlTag::A, "href") => {
                safe_url(&value, false).map(MarkdownHtmlAttribute::Href)
            }
            (MarkdownHtmlTag::Img | MarkdownHtmlTag::Source, "src") => {
                safe_url(&value, true).map(MarkdownHtmlAttribute::Src)
            }
            (MarkdownHtmlTag::Img | MarkdownHtmlTag::Source, "srcset") => {
                let candidates = value
                    .split(',')
                    .filter_map(|candidate| {
                        let mut fields = candidate.split_whitespace();
                        let url = fields.next()?;
                        safe_url(url, true)?;
                        Some(candidate.trim().to_owned())
                    })
                    .collect::<Vec<_>>();
                Some(MarkdownHtmlAttribute::SrcSet(candidates))
            }
            (MarkdownHtmlTag::Img, "alt") => Some(MarkdownHtmlAttribute::Alt(value)),
            (MarkdownHtmlTag::Img, "width") => dimension(&value).map(MarkdownHtmlAttribute::Width),
            (MarkdownHtmlTag::Img, "height") => {
                dimension(&value).map(MarkdownHtmlAttribute::Height)
            }
            (MarkdownHtmlTag::Details, "open") => Some(MarkdownHtmlAttribute::Open),
            _ => None,
        };
        if let Some(projected) = projected {
            output.push(projected);
        }
    }
    output
}

fn dimension(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().filter(|value| *value <= 32_768)
}

fn safe_url(value: &str, image: bool) -> Option<String> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("data:image/") && image {
        return Some(value.to_owned());
    }
    let scheme_end = lower.find(':');
    if let Some(index) = scheme_end {
        let scheme = &lower[..index];
        let windows_drive = scheme.len() == 1
            && scheme.as_bytes()[0].is_ascii_alphabetic()
            && matches!(lower.as_bytes().get(index + 1), Some(b'/' | b'\\'));
        if !windows_drive {
            let allowed = if image {
                matches!(scheme, "http" | "https")
            } else {
                matches!(scheme, "http" | "https" | "mailto" | "tel")
            };
            if !allowed || !scheme.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                return None;
            }
        }
    }
    Some(value.to_owned())
}

fn visible_node_text(node: &MarkdownHtmlNode, output: &mut String) {
    match node {
        MarkdownHtmlNode::Text(text) => output.push_str(text),
        MarkdownHtmlNode::Placeholder => {}
        MarkdownHtmlNode::Element(element) => {
            if matches!(element.tag, MarkdownHtmlTag::Br | MarkdownHtmlTag::Hr) {
                output.push('\n');
            }
            if element.tag != MarkdownHtmlTag::Img {
                for child in &element.children {
                    visible_node_text(child, output);
                }
            }
        }
    }
}

fn collect_node_links(node: &MarkdownHtmlNode, links: &mut Vec<String>) {
    let MarkdownHtmlNode::Element(element) = node else {
        return;
    };
    if element.tag == MarkdownHtmlTag::A {
        links.extend(
            element
                .attributes
                .iter()
                .filter_map(|attribute| match attribute {
                    MarkdownHtmlAttribute::Href(url) => Some(url.clone()),
                    _ => None,
                }),
        );
    }
    for child in &element.children {
        collect_node_links(child, links);
    }
}

fn deepest_element(handle: &Handle) -> Option<Handle> {
    for child in &*handle.0.children.borrow() {
        if let Some(found) = deepest_element(child) {
            return Some(found);
        }
    }
    matches!(handle.0.kind, InternalKind::Element { .. }).then(|| handle.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_element(
        nodes: &[MarkdownHtmlNode],
        tag: MarkdownHtmlTag,
    ) -> Option<&MarkdownHtmlElement> {
        for node in nodes {
            let MarkdownHtmlNode::Element(element) = node else {
                continue;
            };
            if element.tag == tag {
                return Some(element);
            }
            if let Some(found) = find_element(element.children(), tag) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn safe_html_projection_keeps_image_srcset_dimensions_and_details_structure() {
        let fragment = project(
            r##"<details open><summary>More</summary><img src="https://example.invalid/a.png" srcset="https://example.invalid/a.png 1x, https://example.invalid/b.png 2x" alt="a" width="10" height="20"></details><a href="#frag">go</a>"##,
        );
        assert!(!fragment.is_degraded());
        let details = find_element(fragment.nodes(), MarkdownHtmlTag::Details).unwrap();
        assert!(
            details
                .attributes()
                .iter()
                .any(|attribute| matches!(attribute, MarkdownHtmlAttribute::Open))
        );
        assert!(find_element(details.children(), MarkdownHtmlTag::Summary).is_some());
        let image = find_element(details.children(), MarkdownHtmlTag::Img).unwrap();
        assert!(image.attributes().iter().any(|attribute| {
            matches!(attribute, MarkdownHtmlAttribute::Src(url) if url == "https://example.invalid/a.png")
        }));
        assert!(image.attributes().iter().any(|attribute| {
            matches!(
                attribute,
                MarkdownHtmlAttribute::SrcSet(candidates)
                    if candidates == &vec![
                        "https://example.invalid/a.png 1x".to_owned(),
                        "https://example.invalid/b.png 2x".to_owned(),
                    ]
            )
        }));
        assert!(image.attributes().iter().any(|attribute| {
            matches!(attribute, MarkdownHtmlAttribute::Alt(value) if value == "a")
        }));
        assert!(
            image
                .attributes()
                .iter()
                .any(|attribute| { matches!(attribute, MarkdownHtmlAttribute::Width(10)) })
        );
        assert!(
            image
                .attributes()
                .iter()
                .any(|attribute| { matches!(attribute, MarkdownHtmlAttribute::Height(20)) })
        );
        assert_eq!(fragment.links(), ["#frag"]);
        assert!(!fragment.visible_text().contains("<img"));
    }

    #[test]
    fn unsafe_srcset_candidates_and_dangerous_subtrees_are_dropped() {
        let fragment = project(
            r#"<img srcset="javascript:alert(1) 1x, https://example.invalid/ok.png 2x"><script>alert(2)</script><a href="https://example.invalid/good">ok</a>"#,
        );
        let image = find_element(fragment.nodes(), MarkdownHtmlTag::Img).unwrap();
        assert!(image.attributes().iter().any(|attribute| {
            matches!(
                attribute,
                MarkdownHtmlAttribute::SrcSet(candidates)
                    if candidates == &vec!["https://example.invalid/ok.png 2x".to_owned()]
            )
        }));
        assert!(!fragment.visible_text().contains("alert(2)"));
        assert_eq!(fragment.links(), ["https://example.invalid/good"]);
    }
}
