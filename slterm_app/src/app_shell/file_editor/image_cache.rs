//! 文档自有解码图缓存(照上游同构)。
//!
//! 虚拟行不得把图片塞进进程级常驻资产图:缓存以字节(64MB)与条目(128)
//! 双上限做 LRU,逐出时同时释放像素缓冲与图集条目。非活跃文档置
//! `active=false` 并清空全部解码图;草稿与撤销历史不受影响。
//!
//! 并发上限 2 个在途解码;单图超过总预算时记一条失败条目,避免反复重试。

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use gpui::{
    App, AppContext as _, Asset as _, AssetLogger, Entity, EntityId, ImageAssetLoader, ImageCache,
    ImageCacheError, RenderImage, Resource, Task, WeakEntity, Window,
};

const IMAGE_BYTES: usize = 64 * 1024 * 1024;
const IMAGE_ENTRIES: usize = 128;
const MAX_LOADING: usize = 2;
const WORKING_BYTES: usize = 16 * 1024 * 1024;

type ImageResult = Result<Arc<RenderImage>, ImageCacheError>;

/// 字节 + 条目双上限 LRU;逐出回调负责把外部存储一并释放。
struct RenderCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    max_bytes: usize,
    max_entries: usize,
    used: usize,
    clock: u64,
}

struct CacheEntry<V> {
    value: V,
    bytes: usize,
    touched: u64,
}

impl<K: Eq + Hash + Clone, V> RenderCache<K, V> {
    fn new(max_bytes: usize, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_bytes,
            max_entries,
            used: 0,
            clock: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        let entry = self.entries.get_mut(key)?;
        self.clock = self.clock.saturating_add(1);
        entry.touched = self.clock;
        Some(&entry.value)
    }

    /// 单值超预算时直接拒收,不打扰已有可用条目。
    fn insert_with_eviction(
        &mut self,
        key: K,
        value: V,
        bytes: usize,
        mut evict: impl FnMut(V),
    ) -> bool {
        let bytes = bytes.max(1);
        if bytes > self.max_bytes || self.max_entries == 0 {
            return false;
        }
        if let Some(value) = self.take(&key) {
            evict(value);
        }
        while self.entries.len() >= self.max_entries
            || self.used.saturating_add(bytes) > self.max_bytes
        {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(key) = oldest else { break };
            if let Some(value) = self.take(&key) {
                evict(value);
            }
        }
        self.clock = self.clock.saturating_add(1);
        self.used += bytes;
        self.entries.insert(
            key,
            CacheEntry {
                value,
                bytes,
                touched: self.clock,
            },
        );
        true
    }

    fn take(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.used -= entry.bytes;
        Some(entry.value)
    }

    fn drain(&mut self) -> impl Iterator<Item = V> + '_ {
        self.used = 0;
        self.entries.drain().map(|(_, entry)| entry.value)
    }

    /// 收缩到工作预算,但不逐出当前帧正在使用的条目。
    fn trim_with_eviction(
        &mut self,
        bytes: usize,
        keep: impl Fn(&K) -> bool,
        mut evict: impl FnMut(V),
    ) {
        while self.used > bytes {
            let oldest = self
                .entries
                .iter()
                .filter(|(key, _)| !keep(key))
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(key) = oldest else { break };
            if let Some(value) = self.take(&key) {
                evict(value);
            }
        }
    }

    fn used_bytes(&self) -> usize {
        self.used
    }
}

pub(super) struct DocumentImageCache {
    owner: WeakEntity<Self>,
    ready: RenderCache<Resource, ImageResult>,
    loading: HashMap<Resource, Task<()>>,
    /// 文档视图实体:解码完成后只通知它重绘。
    view: EntityId,
    frame_images: HashSet<Resource>,
    byte_limit: usize,
    active: bool,
    generation: u64,
}

impl DocumentImageCache {
    pub(super) fn new(view: EntityId, cx: &mut App) -> Entity<Self> {
        Self::with_limits(view, IMAGE_BYTES, IMAGE_ENTRIES, cx)
    }

    fn with_limits(view: EntityId, bytes: usize, entries: usize, cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|cx| Self {
            owner: cx.entity().downgrade(),
            ready: RenderCache::new(bytes, entries),
            loading: HashMap::new(),
            view,
            frame_images: HashSet::new(),
            byte_limit: bytes,
            active: true,
            generation: 0,
        });
        cx.observe_release(&cache, |cache, cx| {
            cache.clear(None, cx);
        })
        .detach();
        cache
    }

    pub(super) fn begin_frame(&mut self) {
        self.frame_images.clear();
    }

    fn clear(&mut self, window: Option<&mut Window>, cx: &mut App) {
        self.generation = self.generation.wrapping_add(1);
        self.loading.clear();
        self.frame_images.clear();
        let images: Vec<_> = self.ready.drain().flatten().collect();
        if let Some(window) = window {
            for image in images {
                cx.drop_image(image, Some(window));
            }
        } else if !images.is_empty() {
            // 释放观察者可能在窗口更新期间运行;延迟到窗口恢复后再放图集。
            cx.defer(move |cx| {
                for image in images {
                    cx.drop_image(image, None);
                }
            });
        }
    }

    pub(super) fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
        if self.active == active {
            return;
        }
        self.active = active;
        if !active {
            self.clear(Some(window), cx);
        }
    }

    pub(super) fn finish_frame(&mut self, window: &mut Window, cx: &mut App) {
        self.ready.trim_with_eviction(
            WORKING_BYTES,
            |source| self.frame_images.contains(source),
            |old| {
                if let Ok(image) = old {
                    cx.drop_image(image, Some(window));
                }
            },
        );
    }

    /// 测试专用:当前解码图占用的字节数(逐出断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn used_bytes_for_test(&self) -> usize {
        self.ready.used_bytes()
    }

    /// 测试专用:在途解码数(并发上限断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn loading_for_test(&self) -> usize {
        self.loading.len()
    }
}

impl ImageCache for DocumentImageCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<ImageResult> {
        if !self.active {
            return None;
        }
        self.frame_images.insert(resource.clone());
        if let Some(image) = self.ready.get(resource) {
            return Some(image.clone());
        }
        if self.loading.contains_key(resource) || self.loading.len() >= MAX_LOADING {
            return None;
        }

        // 直接调 loader:fetch_asset/use_asset 会把结果留在进程级常驻资产图,
        // 破坏本缓存「随文档逐出」的纪律。
        let future = AssetLogger::<ImageAssetLoader>::load(resource.clone(), cx);
        let background = cx.background_executor().spawn(future);
        let owner = self.owner.clone();
        let source = resource.clone();
        let generation = self.generation;
        let view = self.view;
        let task = window.spawn(cx, async move |cx| {
            let mut result = background.await;
            let _ = owner.update_in(cx, |cache, window, cx| {
                if !cache.active || cache.generation != generation {
                    return;
                }
                cache.loading.remove(&source);
                let mut bytes = result.as_ref().map_or(1, |image| {
                    (0..image.frame_count()).fold(0usize, |total, frame| {
                        total.saturating_add(image.as_bytes(frame).map_or(0, <[u8]>::len))
                    })
                });
                if bytes > cache.byte_limit {
                    // 解码缓存上限不约束瞬时解码存储;留一条失败条目防重复解码。
                    result = Err(
                        image::ImageError::Limits(image::error::LimitError::from_kind(
                            image::error::LimitErrorKind::InsufficientMemory,
                        ))
                        .into(),
                    );
                    bytes = 1;
                }
                cache
                    .ready
                    .insert_with_eviction(source, result, bytes, |old| {
                        if let Ok(image) = old {
                            cx.drop_image(image, Some(window));
                        }
                    });
                App::notify(cx, view);
            });
        });
        self.loading.insert(resource.clone(), task);
        None
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod image_cache_tests {
    use super::*;
    use gpui::{ImageSource, IntoElement, Render, TestAppContext, div};

    struct Dummy;

    impl Render for Dummy {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<'_, Self>) -> impl IntoElement {
            div()
        }
    }

    fn dummy_window(cx: &mut TestAppContext) -> (EntityId, &mut gpui::VisualTestContext) {
        let (dummy, window) = cx.add_window_view(|_, _| Dummy);
        (dummy.entity_id(), window)
    }

    /// 挂起即释放解码图与在途任务;恢复后按需重新解码。
    #[gpui::test]
    fn suspension_drops_decodes_and_reloads_only_on_resume(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("image.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        let (view, window) = dummy_window(cx);
        let cache = window.update(|_, cx| DocumentImageCache::with_limits(view, 128, 8, cx));
        let source = Resource::Path(Arc::from(path));
        window.update(|window, cx| {
            cache.update(cx, |cache, cx| {
                assert!(cache.load(&source, window, cx).is_none())
            });
        });
        window.run_until_parked();
        let old = window.update(|window, cx| {
            cache.update(cx, |cache, cx| {
                let image = cache.load(&source, window, cx).unwrap().unwrap();
                let weak = Arc::downgrade(&image);
                cache.set_active(false, window, cx);
                assert_eq!(cache.ready.used_bytes(), 0);
                assert!(cache.load(&source, window, cx).is_none());
                assert!(cache.loading.is_empty());
                weak
            })
        });
        assert!(old.upgrade().is_none(), "挂起必须释放像素缓冲");
        window.update(|window, cx| {
            cache.update(cx, |cache, cx| {
                cache.set_active(true, window, cx);
                assert!(cache.load(&source, window, cx).is_none());
                assert_eq!(cache.loading.len(), 1);
            });
        });
        window.run_until_parked();
        window.update(|window, cx| {
            cache.update(cx, |cache, cx| {
                assert!(cache.load(&source, window, cx).unwrap().is_ok());
                assert_eq!(cache.ready.used_bytes(), 64);
            });
        });
    }

    /// 热图复用;冷图按 LRU 逐出;缓存 drop 时全部释放。
    #[gpui::test]
    fn decoding_reuses_hot_images_and_releases_with_the_document(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let (view, window) = dummy_window(cx);
        let cache = window.update(|_, cx| DocumentImageCache::with_limits(view, 128, 8, cx));
        let resources: Vec<_> = (0..3)
            .map(|index| {
                let path = directory.path().join(format!("image-{index}.png"));
                image::RgbaImage::from_pixel(4, 4, image::Rgba([index, 0, 0, 255]))
                    .save(&path)
                    .unwrap();
                Resource::Path(Arc::from(path))
            })
            .collect();
        let mut weak_images = Vec::new();
        for resource in &resources {
            window.update(|window, cx| {
                assert!(
                    cache
                        .update(cx, |cache, cx| cache.load(resource, window, cx))
                        .is_none()
                );
            });
            window.run_until_parked();
            let image = window.update(|window, cx| {
                cache
                    .update(cx, |cache, cx| cache.load(resource, window, cx))
                    .unwrap()
                    .unwrap()
            });
            weak_images.push(Arc::downgrade(&image));
            window.update(|window, cx| {
                let reused = cache.update(cx, |cache, cx| cache.load(resource, window, cx));
                assert_eq!(reused.unwrap().unwrap().id, image.id);
                assert!(!ImageSource::Resource(resource.clone()).is_asset_cached(cx));
                assert!(cache.read(cx).ready.used_bytes() <= 128);
            });
        }
        assert!(weak_images[0].upgrade().is_none(), "LRU 必须逐出最冷图");
        assert!(weak_images[1].upgrade().is_some());
        assert!(weak_images[2].upgrade().is_some());
        window.update(|_, _| drop(cache));
        window.run_until_parked();
        assert!(weak_images.iter().all(|image| image.upgrade().is_none()));
    }

    /// 单图超预算记失败条目;并发解码有上限。
    #[gpui::test]
    fn oversized_decodes_are_not_retried_and_pending_work_is_bounded(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let source = Resource::Path(Arc::from(path));
        let (view, window) = dummy_window(cx);
        let cache = window.update(|_, cx| DocumentImageCache::with_limits(view, 16, 8, cx));
        window.update(|window, cx| {
            assert!(
                cache
                    .update(cx, |cache, cx| cache.load(&source, window, cx))
                    .is_none()
            );
        });
        window.run_until_parked();
        window.update(|window, cx| {
            assert!(
                cache
                    .update(cx, |cache, cx| cache.load(&source, window, cx))
                    .unwrap()
                    .is_err()
            );
            assert_eq!(cache.read(cx).ready.used_bytes(), 1);
            assert!(cache.read(cx).loading.is_empty());
            for index in 0..10 {
                let source =
                    Resource::Path(Arc::from(directory.path().join(format!("{index}.png"))));
                cache.update(cx, |cache, cx| cache.load(&source, window, cx));
            }
            assert_eq!(cache.read(cx).loading.len(), MAX_LOADING);
        });
    }
}
