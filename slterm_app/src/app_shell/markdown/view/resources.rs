//! 可见图片与公式的后台任务；所有结果按请求序号和 revision 对账。

use std::sync::Arc;

use super::super::formula::FormulaRequest;
use gpui::{App, RenderImage, WeakEntity};
use image::Frame;
use sha2::{Digest as _, Sha256};

use super::super::local_resource::LocalMarkdownImageRequest;
use super::super::{MarkdownResourceError, MarkdownView};
use crate::app_shell::text_view::MarkdownMode;

impl MarkdownView {
    pub(super) fn prune_resources(&mut self, visible: std::ops::Range<usize>, overscan: usize) {
        let start = visible.start.saturating_sub(overscan);
        let end = visible.end.saturating_add(overscan);
        for blocks in self.image_blocks.values_mut() {
            blocks.retain(|block| start <= *block && *block < end);
        }
        self.image_blocks.retain(|_, blocks| !blocks.is_empty());
        self.images
            .retain(|url, _| self.image_blocks.contains_key(url));
        self.image_failures
            .retain(|url| self.image_blocks.contains_key(url));
        self.image_tasks
            .retain(|url, _| self.image_blocks.contains_key(url));

        for blocks in self.formula_blocks.values_mut() {
            blocks.retain(|block| start <= *block && *block < end);
        }
        self.formula_blocks.retain(|_, blocks| !blocks.is_empty());
        self.formula_images
            .retain(|key, _| self.formula_blocks.contains_key(key));
        self.formula_failures
            .retain(|key| self.formula_blocks.contains_key(key));
        self.formula_tasks
            .retain(|key, _| self.formula_blocks.contains_key(key));
    }

    fn allocate_resource_id(&mut self) -> u64 {
        self.next_resource_id = self
            .next_resource_id
            .checked_add(1)
            .expect("Markdown 资源请求序号不得耗尽");
        self.next_resource_id
    }

    pub(super) fn image(&self, url: &str) -> Option<Arc<RenderImage>> {
        self.images.get(url).cloned()
    }

    pub(super) fn ensure_image(
        &mut self,
        block: usize,
        url: &str,
        owner: &WeakEntity<MarkdownView>,
        cx: &mut App,
    ) {
        if self.mode != MarkdownMode::Render {
            return;
        }
        self.image_blocks
            .entry(url.to_owned())
            .or_default()
            .insert(block);
        if self.images.contains_key(url)
            || self.image_failures.contains(url)
            || self.image_tasks.contains_key(url)
        {
            return;
        }
        let remote = url.starts_with("http://") || url.starts_with("https://");
        let generation = self.remote_image_generations.get(url).copied();
        if remote && generation.is_none() {
            return;
        }
        self.schedule_image(url.to_owned(), false, generation, owner.clone(), cx);
    }

    pub(super) fn load_image_on_click(
        &mut self,
        url: &str,
        owner: &WeakEntity<MarkdownView>,
        cx: &mut App,
    ) {
        self.image_failures.remove(url);
        self.image_tasks.remove(url);
        let remote = url.starts_with("http://") || url.starts_with("https://");
        let generation = if remote {
            let generation = self
                .remote_image_generations
                .get(url)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .expect("远程图片授权代次不得耗尽");
            self.remote_image_generations
                .insert(url.to_owned(), generation);
            Some(generation)
        } else {
            None
        };
        self.schedule_image(url.to_owned(), remote, generation, owner.clone(), cx);
    }

    fn schedule_image(
        &mut self,
        url: String,
        fetch_remote: bool,
        generation: Option<u64>,
        owner: WeakEntity<MarkdownView>,
        cx: &mut App,
    ) {
        let request_id = self.allocate_resource_id();
        let revision = self.revision;
        let document_id = self.document_id;
        let location = self.location.clone();
        let grants = self.resource_grants.clone();
        let services = self.render_services.images.clone();
        let task_url = url.clone();
        let request = if fetch_remote || generation.is_some() || url.starts_with("data:") {
            None
        } else {
            match LocalMarkdownImageRequest::from_markdown(&location, &url, 0) {
                Ok(request) => Some(request),
                Err(_) => {
                    self.image_failures.insert(url);
                    return;
                }
            }
        };
        let background = cx.background_executor().spawn(async move {
            let loaded = services
                .lock()
                .map_err(|_| MarkdownResourceError::Read)
                .and_then(|mut service| {
                    if fetch_remote {
                        service.fetch_remote_on_click(
                            document_id,
                            &task_url,
                            generation.unwrap_or(0),
                        )
                    } else if generation.is_some() {
                        service
                            .peek_remote(document_id, &task_url, generation.unwrap_or(0))
                            .ok_or(MarkdownResourceError::Unauthorized)
                    } else if task_url.starts_with("data:") {
                        service.load_data_image(&task_url)
                    } else {
                        service.load(request.as_ref().expect("本地图片必须具有请求"), &grants, 0)
                    }
                });
            loaded.map(|loaded| {
                let mut pixels = loaded.pixels().as_ref().clone();
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                Arc::new(RenderImage::new([Frame::new(pixels)]))
            })
        });
        let target = owner.clone();
        let completed_url = url.clone();
        let task = App::spawn(cx, async move |cx| {
            let result = background.await;
            let _ = target.update(cx, |view, cx| {
                view.complete_image(&completed_url, request_id, revision, result, cx);
            });
        });
        self.image_tasks.insert(url, (request_id, task));
    }

    pub(super) fn complete_image(
        &mut self,
        url: &str,
        request_id: u64,
        revision: u64,
        result: Result<Arc<RenderImage>, MarkdownResourceError>,
        cx: &mut gpui::Context<'_, MarkdownView>,
    ) {
        let is_current = self
            .image_tasks
            .get(url)
            .is_some_and(|(current, _)| *current == request_id);
        if !is_current || self.revision != revision {
            return;
        }
        self.image_tasks.remove(url);
        match result {
            Ok(image) => {
                self.images.insert(url.to_owned(), image);
            }
            Err(_) => {
                self.image_failures.insert(url.to_owned());
            }
        }
        cx.notify();
    }

    pub(super) fn formula(&self, key: [u8; 32]) -> Option<Arc<RenderImage>> {
        self.formula_images.get(&key).cloned()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn ensure_formula(
        &mut self,
        block: usize,
        source: &str,
        display: bool,
        font_size: f32,
        scale_factor: f32,
        color: [u8; 3],
        owner: &WeakEntity<MarkdownView>,
        cx: &mut App,
    ) -> [u8; 32] {
        let zoom = self.zoom as f32 / 1_000.0;
        let key = formula_key(source, display, font_size, scale_factor, zoom, color);
        if self.mode != MarkdownMode::Render {
            return key;
        }
        self.formula_blocks.entry(key).or_default().insert(block);
        if self.formula_images.contains_key(&key)
            || self.formula_failures.contains(&key)
            || self.formula_tasks.contains_key(&key)
        {
            return key;
        }
        let Some(cache) = self.render_services.formulas.clone() else {
            self.formula_failures.insert(key);
            return key;
        };
        let request_id = self.allocate_resource_id();
        let revision = self.revision;
        let request = FormulaRequest::new(source, display, font_size, scale_factor, zoom, 0, color);
        let background = cx.background_executor().spawn(async move {
            cache
                .lock()
                .map_err(|_| ())
                .and_then(|mut cache| cache.render(&request).map_err(|_| ()))
                .map(|bitmap| {
                    let image = image::RgbaImage::from_raw(
                        bitmap.width,
                        bitmap.height,
                        bitmap.pixels.clone(),
                    )
                    .expect("公式位图尺寸已经由预算校验");
                    Arc::new(RenderImage::new([Frame::new(image)]))
                })
        });
        let target = owner.clone();
        let task = App::spawn(cx, async move |cx| {
            let result = background.await;
            let _ = target.update(cx, |view, cx| {
                view.complete_formula(key, request_id, revision, result, cx);
            });
        });
        self.formula_tasks.insert(key, (request_id, task));
        key
    }

    pub(super) fn complete_formula(
        &mut self,
        key: [u8; 32],
        request_id: u64,
        revision: u64,
        result: Result<Arc<RenderImage>, ()>,
        cx: &mut gpui::Context<'_, MarkdownView>,
    ) {
        let is_current = self
            .formula_tasks
            .get(&key)
            .is_some_and(|(current, _)| *current == request_id);
        if !is_current || self.revision != revision {
            return;
        }
        self.formula_tasks.remove(&key);
        match result {
            Ok(image) => {
                self.formula_images.insert(key, image);
            }
            Err(()) => {
                self.formula_failures.insert(key);
            }
        }
        cx.notify();
    }
}

fn formula_key(
    source: &str,
    display: bool,
    font_size: f32,
    scale_factor: f32,
    zoom: f32,
    color: [u8; 3],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(source.as_bytes());
    hash.update([display as u8]);
    hash.update(font_size.to_bits().to_le_bytes());
    hash.update(scale_factor.to_bits().to_le_bytes());
    hash.update(zoom.to_bits().to_le_bytes());
    hash.update(color);
    hash.finalize().into()
}
