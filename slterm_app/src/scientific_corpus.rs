//! 维护中的数学语料是回归输入与基准的来源。
//!
//! 化学/生物条目已消亡：这里只保留数学文档与数学用例；
//! `smiles` 代码块不再产出用例。

use std::path::PathBuf;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options};

use crate::math::cache::{FormulaCacheKey, LAYOUT_CACHE_BUDGET, MathLayoutCache};
use crate::math::layout::MathLayout;
use crate::math::{DEFAULT_LIMITS, compile_formula_source};

/// `(文档名, 数学用例下限)`。下限防语料缩水，不登记任何快照计数。
pub(crate) const DOCUMENTS: [(&str, usize); 2] = [
    ("math-rendering-test.md", 50),
    ("scientific-rendering-acceptance-paper.md", 20),
];

#[derive(Debug)]
pub(crate) struct Case {
    pub(crate) source: String,
    pub(crate) display: bool,
    pub(crate) fallback: bool,
    pub(crate) preserve: bool,
    pub(crate) line: usize,
}

fn document_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/corpus")
        .join(name)
}

pub(crate) fn read_document(name: &str) -> String {
    let path = document_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert!(
        text.len() <= 128 * 1024,
        "default regression corpus must stay small"
    );
    text
}

pub(crate) fn cases(source: &str) -> Vec<Case> {
    let mut events = fixture_events(source);
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.math_dollars = true;
    let root = comrak::parse_document(&arena, source, &options);
    collect_inline_math(root, &mut events);
    events.sort_by_key(|event| event.line());
    let mut output = Vec::new();
    let mut expectation = (false, false);
    for event in events {
        match event {
            CorpusEvent::Mark(_, next) => {
                assert_eq!(expectation, (false, false), "unconsumed expectation marker");
                expectation = next;
            }
            CorpusEvent::Clear(_) => expectation = (false, false),
            CorpusEvent::Math {
                source,
                display,
                line,
            } => {
                output.push(Case {
                    source,
                    display,
                    fallback: expectation.0,
                    preserve: expectation.1,
                    line,
                });
                expectation = (false, false);
            }
        }
    }
    assert_eq!(expectation, (false, false), "orphan corpus expectation");
    output
}

enum CorpusEvent {
    Mark(usize, (bool, bool)),
    Clear(usize),
    Math {
        source: String,
        display: bool,
        line: usize,
    },
}

impl CorpusEvent {
    const fn line(&self) -> usize {
        match self {
            Self::Math { line, .. } | Self::Mark(line, _) | Self::Clear(line) => *line,
        }
    }
}

/// 夹具允许块级公式含空行；这只恢复上游数学输入，不参与生产 Markdown 语义。
fn fixture_events(source: &str) -> Vec<CorpusEvent> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut events = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        match line {
            "<!-- slterm-test: source-fallback -->" => {
                events.push(CorpusEvent::Mark(index + 1, (true, false)));
            }
            "<!-- slterm-test: preserve-source -->" => {
                events.push(CorpusEvent::Mark(index + 1, (false, true)));
            }
            _ => {}
        }
        if let Some(fence) = line.strip_prefix("```") {
            let clear = fence
                .split_whitespace()
                .next()
                .is_some_and(|language| language.eq_ignore_ascii_case("smiles"));
            let mut end = index + 1;
            while end < lines.len() && lines[end].trim() != "```" {
                end += 1;
            }
            assert!(end < lines.len(), "unclosed fixture code fence");
            if clear {
                events.push(CorpusEvent::Clear(index + 1));
            }
            index = end + 1;
            continue;
        }
        if line == "$$" || (line.starts_with("$$") && line.ends_with("$$") && line.len() > 4) {
            let (content, end) = if line == "$$" {
                let mut end = index + 1;
                while end < lines.len() && lines[end].trim() != "$$" {
                    end += 1;
                }
                assert!(end < lines.len(), "unclosed fixture display math");
                (lines[index + 1..end].join("\n"), end)
            } else {
                (line[2..line.len() - 2].to_owned(), index)
            };
            events.push(CorpusEvent::Math {
                source: content,
                display: true,
                line: index + 1,
            });
            index = end + 1;
            continue;
        }
        index += 1;
    }
    events
}

fn collect_inline_math<'a>(node: &'a AstNode<'a>, events: &mut Vec<CorpusEvent>) {
    let data = node.data.borrow();
    let math = match &data.value {
        NodeValue::Math(math) if !math.display_math => Some(CorpusEvent::Math {
            source: math.literal.clone(),
            display: false,
            line: data.sourcepos.start.line,
        }),
        _ => None,
    };
    drop(data);
    if let Some(math) = math {
        events.push(math);
    }
    for child in node.children() {
        collect_inline_math(child, events);
    }
}

fn assert_math_pixels(layout: &MathLayout, scale: f32) {
    assert!(layout.metrics.width.is_finite() && layout.metrics.width >= 0.0);
    assert!(layout.metrics.height.is_finite() && layout.metrics.depth.is_finite());
    let rasterizer = crate::math::rasterizer::MathGlyphRasterizer::new().unwrap();
    let geometry =
        crate::math::bitmap::compose(&rasterizer, layout, scale, [0, 0, 0]).expect("formula image");
    let bytes = &geometry.pixels;
    assert_eq!(
        bytes.len(),
        geometry.width as usize * geometry.height as usize * 4
    );
    assert!(geometry.width > 0 && geometry.height > 0);
    if !layout.glyphs.is_empty() || !layout.rules.is_empty() {
        assert!(
            bytes.chunks_exact(4).any(|pixel| pixel[3] != 0),
            "empty mathematical image"
        );
    }
}

/// 语料渲染的确定性指纹：文档顺序 × 用例顺序 × 布局操作全字段。
///
/// 与上游数学引擎逐例对拍：只要移植后有任何几何/字形/文本兜底差异，
/// 指纹即变。基线值由上游重构前源码独立编译取得，不是本仓自证。
fn digest_write(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn digest_u64(hash: &mut u64, value: u64) {
    digest_write(hash, &value.to_le_bytes());
}

fn corpus_digest() -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for (name, _) in DOCUMENTS {
        digest_write(&mut hash, name.as_bytes());
        for case in cases(&read_document(name)) {
            // 行尾是传输层属性而非数学内容；归一后指纹不随 checkout 的
            // core.autocrlf 漂移。
            let source = case.source.replace("\r\n", "\n");
            digest_write(&mut hash, source.as_bytes());
            digest_u64(&mut hash, u64::from(case.display));
            digest_u64(&mut hash, u64::from(case.fallback));
            digest_u64(&mut hash, u64::from(case.preserve));
            let Ok(layout) =
                compile_formula_source(&source, case.display, 18.0, 1.0, DEFAULT_LIMITS)
            else {
                continue;
            };
            digest_u64(&mut hash, u64::from(layout.metrics.width.to_bits()));
            digest_u64(&mut hash, u64::from(layout.metrics.height.to_bits()));
            digest_u64(&mut hash, u64::from(layout.metrics.depth.to_bits()));
            digest_u64(&mut hash, u64::from(layout.metrics.axis.to_bits()));
            digest_u64(&mut hash, layout.glyphs.len() as u64);
            for glyph in &layout.glyphs {
                digest_u64(&mut hash, u64::from(glyph.glyph_id));
                digest_u64(&mut hash, u64::from(glyph.x.to_bits()));
                digest_u64(&mut hash, u64::from(glyph.baseline_y.to_bits()));
                digest_u64(&mut hash, u64::from(glyph.pixel_size.to_bits()));
            }
            digest_u64(&mut hash, layout.rules.len() as u64);
            for rule in &layout.rules {
                digest_u64(&mut hash, u64::from(rule.x.to_bits()));
                digest_u64(&mut hash, u64::from(rule.y.to_bits()));
                digest_u64(&mut hash, u64::from(rule.width.to_bits()));
                digest_u64(&mut hash, u64::from(rule.height.to_bits()));
            }
            digest_u64(&mut hash, layout.text.len() as u64);
            for text in &layout.text {
                digest_u64(&mut hash, u64::from(text.character as u32));
                digest_u64(&mut hash, u64::from(text.x.to_bits()));
                digest_u64(&mut hash, u64::from(text.baseline_y.to_bits()));
                digest_u64(&mut hash, u64::from(text.pixel_size.to_bits()));
            }
        }
    }
    hash
}

#[test]
fn maintained_math_documents_generate_math_pixels() {
    let mut failures = Vec::new();
    for (name, minimum_math) in DOCUMENTS {
        let input = read_document(name);
        let cases = cases(&input);
        assert!(
            cases.iter().filter(|case| !case.fallback).count() >= minimum_math,
            "{name}: mathematical coverage shrank"
        );
        for case in cases {
            let result =
                compile_formula_source(&case.source, case.display, 18.0, 1.0, DEFAULT_LIMITS);
            match (case.fallback, result) {
                (true, Err(_)) => {}
                (false, Ok(layout)) => {
                    assert_math_pixels(&layout, 1.0);
                    if case.preserve {
                        let font = crate::math::font::MathFont::load().unwrap();
                        if !case.source.contains('=') {
                            let equal = font.glyph_id('=').unwrap().0;
                            assert!(
                                !layout.glyphs.iter().any(|op| op.glyph_id == equal),
                                "must not invent a missing equality"
                            );
                        }
                        if case.source.contains(",dx") {
                            let comma = font.glyph_id(',').unwrap().0;
                            assert!(
                                layout.glyphs.iter().any(|op| op.glyph_id == comma),
                                "comma must not become invisible spacing"
                            );
                        }
                    }
                }
                (expected, actual) => failures.push(format!(
                    "{name}:{} expected fallback={expected}; result={:?}; source={}",
                    case.line,
                    actual.map(|_| "rendered"),
                    case.source
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Math corpus failures:\n{}",
        failures.join("\n")
    );
}

/// 与上游数学引擎的零 diff 门禁：语料逐例渲染指纹必须等于基线。
///
/// 基线由上游应用 crate 的 `math/` 十模块独立编译（同一语料、同一
/// pulldown-latex/ttf-parser 版本、同一 18px/ppp 1.0 编译参数）后取得；
/// 不是本仓自证。
#[test]
fn corpus_rendering_matches_the_upstream_baseline() {
    const UPSTREAM_BASELINE_DIGEST: u64 = 6_833_597_031_715_159_241;
    assert_eq!(
        corpus_digest(),
        UPSTREAM_BASELINE_DIGEST,
        "corpus rendering diverged from the upstream baseline"
    );
}

#[test]
fn repeated_corpus_resize_uses_bounded_lru_and_keeps_geometry_finite() {
    let cases = cases(&read_document("scientific-rendering-acceptance-paper.md"));
    let cases: Vec<_> = cases
        .iter()
        .filter(|case| !case.fallback)
        .take(12)
        .collect();
    let mut cache = MathLayoutCache::default();
    for width in [880.0_f32, 320.0, 640.0, 240.0, 880.0, 320.0] {
        for (index, case) in cases.iter().enumerate() {
            let base =
                compile_formula_source(&case.source, case.display, 18.0, 1.0, DEFAULT_LIMITS)
                    .unwrap();
            let pixel_size =
                (18.0 * (width / base.metrics.width.max(1.0)).min(1.0) * 0.98).max(6.0);
            let key = FormulaCacheKey::new(index as u64, pixel_size, 1.0, case.display);
            if cache.get(key).is_none() {
                let layout = cache
                    .get_or_insert_with(key, || {
                        compile_formula_source(
                            &case.source,
                            case.display,
                            pixel_size,
                            1.0,
                            DEFAULT_LIMITS,
                        )
                    })
                    .unwrap();
                assert_math_pixels(layout, 1.25);
            }
            assert!(cache.used_bytes() <= LAYOUT_CACHE_BUDGET);
        }
    }
}
