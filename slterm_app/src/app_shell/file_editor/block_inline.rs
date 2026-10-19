//! 混合段落的内联 run(照上游同构):只有嵌入对象切分原生输入 run,
//! 周边 Markdown 在规范源码里逐字节保留。
//!
//! 病理块(对象数超上限)回退为单字面量部件 + 单输入,避免逐对象物化视图。
//!
//! 消费点:内联分段渲染。

use super::block_structure::{EditPart, PartKind, StructureNode, add, span};
use super::inline_edit::Marks;
use ::markdown::mdast::Node;

const MAX_INLINE_RUNS: usize = 128;

#[derive(Clone, Debug)]
pub(super) struct InlineRun {
    pub part: usize,
    pub marks: Marks,
}

pub(super) fn contains_object(node: &Node) -> bool {
    is_object(node)
        || node
            .children()
            .is_some_and(|children| children.iter().any(contains_object))
}

fn is_object(node: &Node) -> bool {
    matches!(
        node,
        Node::InlineMath(_) | Node::Image(_) | Node::ImageReference(_)
    )
}

pub(super) fn build(
    node: &Node,
    source: &str,
    offset: usize,
    parts: &mut Vec<EditPart>,
) -> Option<StructureNode> {
    let initial = parts.len();
    let heading = if let Node::Heading(heading) = node {
        Some(heading.depth)
    } else {
        None
    };
    let mut builder = Builder {
        source,
        offset,
        parts,
        runs: vec![],
        heading,
    };
    let built = node
        .children()
        .is_some_and(|children| builder.collect(children, Marks::default()));
    if !built {
        // Do not instantiate a TextView per tiny object in a pathological block.
        // A literal preview and a single source input keep this fallback bounded.
        builder.parts.truncate(initial);
        return Some(StructureNode::Literal(add(
            builder.parts,
            span(node, offset)?,
            PartKind::Source,
            heading,
        )));
    }
    Some(StructureNode::Inline(builder.runs))
}

struct Builder<'a> {
    source: &'a str,
    offset: usize,
    parts: &'a mut Vec<EditPart>,
    runs: Vec<InlineRun>,
    heading: Option<u8>,
}

impl Builder<'_> {
    fn collect(&mut self, nodes: &[Node], marks: Marks) -> bool {
        for node in nodes {
            if !is_object(node) && contains_object(node) {
                let mut inherited = marks;
                match node {
                    Node::Strong(_) => inherited.bold = true,
                    Node::Emphasis(_) => inherited.italic = true,
                    Node::Delete(_) => inherited.strike = true,
                    Node::Link(_) | Node::LinkReference(_) => inherited.link = true,
                    _ => {}
                }
                if let Some(children) = node.children() {
                    if !self.collect(children, inherited) {
                        return false;
                    }
                    continue;
                }
            }
            let Some(range) = span(node, self.offset) else {
                return false;
            };
            if !is_object(node)
                && let Some(previous) = self.runs.last()
            {
                let part = &mut self.parts[previous.part];
                if matches!(part.kind, PartKind::Rich)
                    && previous.marks == marks
                    && part.range.end <= range.start
                    && self
                        .source
                        .get(part.range.end..range.start)
                        .is_some_and(|gap| gap.chars().all(char::is_whitespace))
                {
                    part.range.end = range.end;
                    continue;
                }
            }
            if self.runs.len() == MAX_INLINE_RUNS {
                return false;
            }
            let kind = if is_object(node) {
                PartKind::Source
            } else {
                PartKind::Rich
            };
            let part = add(self.parts, range, kind, self.heading);
            self.runs.push(InlineRun { part, marks });
        }
        true
    }
}

#[cfg(test)]
mod block_inline_tests {
    use super::*;
    use crate::app_shell::file_editor::block_structure::BlockStructure;

    fn parse(source: &str) -> BlockStructure {
        let mut options = ::markdown::ParseOptions::gfm();
        options.constructs.math_text = true;
        let Node::Root(root) = ::markdown::to_mdast(source, &options).unwrap() else {
            panic!()
        };
        BlockStructure::from_node(&root.children[0], source, 0).unwrap()
    }

    #[test]
    fn identical_objects_have_distinct_source_spans_and_keep_nested_marks() {
        let source = "前 **bold $x$ after** ![pic](a.png) $x$ 后";
        let structure = parse(source);
        let StructureNode::Inline(runs) = &structure.root else {
            panic!()
        };
        let formulas: Vec<_> = runs
            .iter()
            .filter(|run| &source[structure.parts[run.part].range.clone()] == "$x$")
            .collect();
        assert_eq!(formulas.len(), 2);
        assert!(formulas[0].marks.bold);
        assert!(!formulas[1].marks.bold);
        assert_ne!(
            structure.parts[formulas[0].part].range,
            structure.parts[formulas[1].part].range
        );
        assert!(
            structure
                .parts
                .iter()
                .any(|part| &source[part.range.clone()] == "![pic](a.png)")
        );
    }

    #[test]
    fn excessive_objects_have_one_literal_part_and_no_partial_allocation() {
        let source = "$x$ ".repeat(MAX_INLINE_RUNS + 1);
        let structure = parse(&source);
        assert!(matches!(structure.root, StructureNode::Literal(0)));
        assert_eq!(structure.parts.len(), 1);
    }
}
