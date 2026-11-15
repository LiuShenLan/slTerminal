use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::rc::Rc;

use gpui::{
    App, ContentMask, Context, Entity, Font, InteractiveElement as _, ParentElement as _, Pixels,
    Render, ScrollWheelEvent, ShapedLine, Styled as _, Task, TextAlign, TextRun, Window, canvas,
    div, point, px,
};
use gpui_component::ActiveTheme as _;
use slterm_document::{
    DocumentId, DocumentPageCache, DocumentSession, PageKey, PagedSource, TextStateId,
};

use super::layout::{FontMetricsKey, LayoutCache, LayoutMetrics, TextLayoutKey, ThemeFingerprint};
use super::metrics::DisplayMap;
use super::source::{PagedTextSource, VisibleLine, VisibleRow};
use crate::app_shell::paged_document::PageReader;

#[cfg(all(test, feature = "gpui-test-support"))]
type PaintedLines = Rc<RefCell<Vec<(usize, Option<String>)>>>;

/// 只读分页文本视图；正文只来自真实 PagedSource 与共享页缓存。
pub(crate) struct PagedTextView {
    document_id: DocumentId,
    source: PagedSource,
    text_state: TextStateId,
    cache: Rc<RefCell<DocumentPageCache>>,
    reader: PageReader,
    pending_pages: BTreeSet<usize>,
    failed_pages: BTreeSet<usize>,
    tasks: BTreeMap<usize, Task<()>>,
    scroll_line: usize,
    scroll_column: usize,
    wheel_remainder: f32,
    layout: Rc<RefCell<Option<LayoutMetrics>>>,
    layout_cache: Rc<RefCell<LayoutCache>>,
    theme_revision: u64,
    theme_fingerprint: Option<ThemeFingerprint>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_lines: PaintedLines,
}

struct PagedPaintFrame {
    cell_width: Pixels,
    line_height: Pixels,
    rows: Vec<(usize, Option<(ShapedLine, VisibleLine)>)>,
}

impl PagedTextView {
    pub(crate) fn new(
        session: &DocumentSession,
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        _cx: &mut Context<'_, Self>,
    ) -> Self {
        let source = session
            .paged()
            .expect("PagedTextView 只接受分页正文")
            .clone();
        Self {
            document_id: session.id(),
            source,
            text_state: session.current_state(),
            cache,
            reader,
            pending_pages: BTreeSet::new(),
            failed_pages: BTreeSet::new(),
            tasks: BTreeMap::new(),
            scroll_line: 0,
            scroll_column: 0,
            wheel_remainder: 0.0,
            layout: Rc::default(),
            layout_cache: Rc::default(),
            theme_revision: 0,
            theme_fingerprint: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            painted_lines: Rc::default(),
        }
    }

    /// 旧代次回执不得结算或污染当前源；失败页在当前视图内不自动重试。
    fn apply_page_result(
        &mut self,
        page: usize,
        generation: u64,
        fingerprint: [u8; 32],
        result: io::Result<String>,
        cx: &mut Context<'_, Self>,
    ) {
        if generation != self.source.source_generation()
            || fingerprint != *self.source.fingerprint()
        {
            return;
        }
        self.pending_pages.remove(&page);
        match result {
            Ok(text) => {
                let key = PageKey::new(self.document_id, generation, page);
                if !self.cache.borrow_mut().insert(key, text) {
                    self.failed_pages.insert(page);
                }
            }
            Err(_) => {
                self.failed_pages.insert(page);
            }
        }
        cx.notify();
    }

    fn scroll_wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<'_, Self>) {
        let Some(metrics) = *self.layout.borrow() else {
            return;
        };
        let pixels = event.delta.pixel_delta(metrics.line_height).y.as_f32();
        if !pixels.is_finite() {
            return;
        }
        self.wheel_remainder += pixels;
        let lines = (self.wheel_remainder / metrics.line_height.as_f32().max(1.0)).trunc() as i32;
        self.wheel_remainder -= lines as f32 * metrics.line_height.as_f32();
        if lines > 0 {
            self.scroll_line = self.scroll_line.saturating_sub(lines as usize);
        } else if lines < 0 {
            self.scroll_line = self
                .scroll_line
                .saturating_add(lines.unsigned_abs() as usize)
                .min(self.source.line_index().line_count().saturating_sub(1));
        }
        if lines != 0 {
            cx.notify();
        }
    }

    fn claim_pages(&mut self, pages: &[usize]) -> Vec<(usize, u64, [u8; 32], PageReader)> {
        let generation = self.source.source_generation();
        let fingerprint = *self.source.fingerprint();
        pages
            .iter()
            .copied()
            .filter(|page| {
                if self.source.index().page(*page).is_none()
                    || self.pending_pages.contains(page)
                    || self.failed_pages.contains(page)
                {
                    return false;
                }
                let key = PageKey::new(self.document_id, generation, *page);
                if self.cache.borrow_mut().get(&key).is_some() {
                    return false;
                }
                self.pending_pages.insert(*page);
                true
            })
            .map(|page| (page, generation, fingerprint, self.reader.clone()))
            .collect()
    }

    /// 缺页读取由 foreground Task 持有；Drop 会级联取消等待中的后台读取。
    fn request_pages(view: &Entity<Self>, pages: Vec<usize>, cx: &mut App) {
        let requests = view.update(cx, |view, _| {
            view.tasks.retain(|_, task| !task.is_ready());
            view.claim_pages(&pages)
        });
        for (page, generation, fingerprint, reader) in requests {
            let background = cx.background_executor().spawn(async move { reader(page) });
            let target = view.downgrade();
            let task = cx.spawn(async move |cx| {
                let result = background.await;
                let Some(target) = target.upgrade() else {
                    return;
                };
                target.update(cx, |view, cx| {
                    view.apply_page_result(page, generation, fingerprint, result, cx);
                });
            });
            view.update(cx, |view, _| {
                view.tasks.insert(page, task);
            });
        }
    }

    fn shape_line(
        line: VisibleLine,
        cache: &mut LayoutCache,
        window: &mut Window,
        font_size: Pixels,
        run: &TextRun,
    ) -> (ShapedLine, VisibleLine) {
        let shaped = cache.get(line.index()).unwrap_or_else(|| {
            let shaped = window.text_system().shape_line(
                line.text().to_owned().into(),
                font_size,
                std::slice::from_ref(run),
                None,
            );
            cache.insert(line.index(), shaped.clone());
            shaped
        });
        (shaped, line)
    }
}

impl Render for PagedTextView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let background = theme.background;
        let font_size = theme.mono_font_size;
        let fingerprint = ThemeFingerprint {
            foreground,
            accent: theme.accent,
            mono_font_family: theme.mono_font_family.clone(),
            mono_font_size: font_size,
        };
        if self.theme_fingerprint.as_ref() != Some(&fingerprint) {
            self.theme_revision = self
                .theme_revision
                .checked_add(1)
                .expect("主题 revision 不得耗尽");
            self.theme_fingerprint = Some(fingerprint);
        }
        let font = Font {
            family: theme.mono_font_family.clone(),
            ..Font::default()
        };
        let run = TextRun {
            len: 0,
            font: font.clone(),
            color: foreground,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let view = cx.entity();
        let document_id = self.document_id;
        let source = self.source.clone();
        let text_state = self.text_state;
        let cache = self.cache.clone();
        let layout_cache = self.layout_cache.clone();
        let layout = self.layout.clone();
        let theme_revision = self.theme_revision;
        let scroll_line = self.scroll_line;
        let scroll_column = self.scroll_column;
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_lines = self.painted_lines.clone();
        let content = canvas(
            move |bounds, window, cx| {
                let line_height = px(20.0);
                let sample = window.text_system().shape_line(
                    "M".into(),
                    font_size,
                    &[TextRun {
                        len: 1,
                        ..run.clone()
                    }],
                    None,
                );
                let cell_width = sample.width;
                *layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width,
                    line_height,
                });
                layout_cache.borrow_mut().activate(TextLayoutKey {
                    document_id,
                    text_state,
                    language_revision: 0,
                    font: FontMetricsKey {
                        family: font.family.clone(),
                        font_size_bits: font_size.as_f32().to_bits(),
                        cell_width_bits: cell_width.as_f32().to_bits(),
                        line_height_bits: line_height.as_f32().to_bits(),
                    },
                    wrap_width_bits: bounds.size.width.as_f32().to_bits(),
                    first_visible_column: scroll_column,
                    dpi_bits: window.scale_factor().to_bits(),
                    theme_revision,
                });
                let height: f32 = bounds.size.height.into();
                let count = (height / line_height.as_f32()).ceil().max(1.0) as usize;
                let columns = (bounds.size.width / cell_width).ceil().max(1.0) as usize;
                let (rows, missing) = {
                    let mut cache = cache.borrow_mut();
                    let mut text =
                        PagedTextSource::new(document_id, &source, &mut cache, DisplayMap::new(4));
                    let rows = text.visible_line_windows(
                        scroll_line..scroll_line + count,
                        scroll_column..scroll_column + columns,
                    );
                    let missing = text.missing_pages().iter().copied().collect::<Vec<_>>();
                    (rows, missing)
                };
                Self::request_pages(&view, missing, cx);
                #[cfg(all(test, feature = "gpui-test-support"))]
                {
                    *painted_lines.borrow_mut() = rows
                        .iter()
                        .map(|row| {
                            (
                                row.index(),
                                row.as_text().map(|line| line.text().to_owned()),
                            )
                        })
                        .collect();
                }
                let mut cache = layout_cache.borrow_mut();
                let rows = rows
                    .into_iter()
                    .map(|row| {
                        let index = row.index();
                        let line = match row {
                            VisibleRow::Text(line) => {
                                let run = TextRun {
                                    len: line.text().len(),
                                    ..run.clone()
                                };
                                Some(Self::shape_line(line, &mut cache, window, font_size, &run))
                            }
                            VisibleRow::Loading { .. } => None,
                        };
                        (index, line)
                    })
                    .collect();
                PagedPaintFrame {
                    cell_width,
                    line_height,
                    rows,
                }
            },
            move |bounds, frame, window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    for (row, (_, line)) in frame.rows.iter().enumerate() {
                        if let Some((shaped, line)) = line {
                            let origin = point(
                                bounds.origin.x
                                    + frame.cell_width
                                        * (line.display_start() as f32 - scroll_column as f32),
                                bounds.origin.y + frame.line_height * row as f32,
                            );
                            let _ = shaped.paint(
                                origin,
                                frame.line_height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                });
            },
        )
        .size_full()
        .overflow_hidden()
        .bg(background);
        div()
            .size_full()
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _, cx| {
                view.scroll_wheel(event, cx);
            }))
            .child(content)
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests {
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use gpui::{AppContext as _, Entity, Render, TestAppContext, Window, px};
    use slterm_document::{
        DocumentId, DocumentMetadata, DocumentPageCache, DocumentRegistry, PageIndex,
        PagedLineIndex, PagedSource, SourceByteRange, TextStateId,
    };
    use slterm_path::{CanonicalPath, ResolvedPath};
    use uuid::Uuid;

    use super::PagedTextView;
    use crate::app_shell::paged_document::PageReader;

    fn source_for(pages: &[&str]) -> PagedSource {
        let mut offset = 0_u64;
        let mut ranges = Vec::with_capacity(pages.len());
        let mut stats = Vec::with_capacity(pages.len());
        for page in pages {
            let end = offset + page.len() as u64;
            ranges.push(SourceByteRange::new(offset, end).unwrap());
            stats.push((
                page.bytes().filter(|byte| *byte == b'\n').count(),
                page.len(),
                page.ends_with('\n'),
            ));
            offset = end;
        }
        PagedSource::new(
            PageIndex::new(offset, ranges).unwrap(),
            [21; 32],
            PagedLineIndex::from_page_stats(&stats),
        )
    }

    struct Host {
        view: Entity<PagedTextView>,
    }

    impl Host {
        fn new(
            cache: Rc<RefCell<DocumentPageCache>>,
            reader: PageReader,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            Self::with_source(cache, reader, source_for(&["hello\n", "world\n"]), cx)
        }

        fn with_source(
            cache: Rc<RefCell<DocumentPageCache>>,
            reader: PageReader,
            source: PagedSource,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
            let mut registry = DocumentRegistry::new();
            registry
                .open(
                    id,
                    ResolvedPath::new(CanonicalPath::new("D:/work/paged.txt").unwrap(), None),
                    source,
                    TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                    Arc::<[u8]>::default(),
                    DocumentMetadata {
                        read_only: true,
                        ..DocumentMetadata::default()
                    },
                )
                .unwrap();
            let view =
                cx.new(|cx| PagedTextView::new(registry.session(id).unwrap(), cache, reader, cx));
            Self { view }
        }
    }

    impl Render for Host {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut gpui::Context<'_, Self>,
        ) -> impl gpui::IntoElement {
            self.view.clone()
        }
    }

    #[gpui::test]
    fn paged_view_loads_missing_pages_into_shared_cache(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "hello\n".to_owned(),
                1 => "world\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });

        let painted = window
            .update(cx, |host, _, cx| host.view.read(cx).painted_lines.clone())
            .unwrap();
        assert_eq!(painted.borrow().len(), 3);
        visual.run_until_parked();
        assert_eq!(cache.borrow().len(), 2);

        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        assert_eq!(
            painted.borrow().as_slice(),
            &[
                (0, Some("hello".to_owned())),
                (1, Some("world".to_owned())),
                (2, Some(String::new())),
            ]
        );
        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 1]);
        let tasks = window
            .update(cx, |host, _, cx| host.view.read(cx).tasks.len())
            .unwrap();
        assert_eq!(tasks, 0);
    }

    #[gpui::test]
    fn paged_view_does_not_retry_failed_pages_on_every_frame(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "读取失败"))
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        for _ in 0..2 {
            visual.refresh().unwrap();
            visual.update(|window, cx| {
                window.draw(cx).clear(cx);
            });
        }

        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 1]);
        assert!(cache.borrow().is_empty());
        let failed = window
            .update(cx, |host, _, cx| host.view.read(cx).failed_pages.len())
            .unwrap();
        assert_eq!(failed, 2);
    }

    #[gpui::test]
    fn paged_view_scrolls_to_request_a_later_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let first = (0..100).map(|line| format!("{line}\n")).collect::<String>();
        let pages = Arc::new(vec![first, "last\n".to_owned()]);
        let page_refs = pages.iter().map(String::as_str).collect::<Vec<_>>();
        let source = source_for(&page_refs);
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader_pages = pages.clone();
        let reader: PageReader = Arc::new(move |page| {
            reader_pages
                .get(page)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "页号越界"))
        });
        let window = cx.add_window(|_, cx| Host::with_source(cache.clone(), reader, source, cx));
        window
            .update(cx, |_, window, _| {
                window.resize(gpui::size(px(160.0), px(40.0)))
            })
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 0))
                .is_some()
        );
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_none()
        );

        let metrics = window
            .update(cx, |host, _, cx| *host.view.read(cx).layout.borrow())
            .unwrap()
            .unwrap();
        visual.simulate_event(gpui::ScrollWheelEvent {
            position: metrics.bounds.center(),
            delta: gpui::ScrollDelta::Lines(gpui::point(0.0, -100.0)),
            ..Default::default()
        });
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_some()
        );
    }

    #[gpui::test]
    fn paged_view_deduplicates_inflight_page_requests(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok("only\n".to_owned())
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache.clone(), reader, source_for(&["only\n"]), cx)
        });
        window
            .update(cx, |host, _, cx| {
                PagedTextView::request_pages(&host.view, vec![0], cx);
                PagedTextView::request_pages(&host.view, vec![0], cx);
            })
            .unwrap();
        let visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        assert_eq!(requests.lock().unwrap().as_slice(), &[0]);
    }

    #[gpui::test]
    fn destroying_paged_view_cancels_requests_and_ignores_late_results(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("late\n".to_owned()));
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache.clone(), reader, source_for(&["only\n"]), cx)
        });
        let weak = window
            .update(cx, |host, _, cx| {
                let weak = host.view.downgrade();
                PagedTextView::request_pages(&host.view, vec![0], cx);
                weak
            })
            .unwrap();
        window
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        let visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();

        assert!(weak.upgrade().is_none(), "在途任务不得强持已销毁视图");
        assert!(cache.borrow().is_empty(), "迟到回执不得进入共享缓存");
    }

    #[gpui::test]
    fn paged_view_rejects_a_mismatched_source_generation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("unused\n".to_owned()));
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        window
            .update(cx, |host, _, cx| {
                host.view.update(cx, |view, cx| {
                    view.pending_pages.insert(0);
                    view.apply_page_result(0, 1, [21; 32], Ok("stale\n".to_owned()), cx);
                    assert!(view.pending_pages.contains(&0));
                });
            })
            .unwrap();
        assert!(cache.borrow().is_empty());
    }
}
