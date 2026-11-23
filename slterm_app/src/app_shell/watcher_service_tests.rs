use std::str::FromStr;
use std::time::{Duration, Instant};

use slterm_path::CanonicalPath;
use slterm_sidebar::FileTreeRoot;
use slterm_workspace::ProjectId;

use super::watcher_service::WatcherService;

fn root() -> FileTreeRoot {
    FileTreeRoot::Project(ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap())
}

fn path(value: &str) -> CanonicalPath {
    CanonicalPath::new(value).unwrap()
}

#[test]
fn watcher_burst_coalesces_for_two_hundred_milliseconds() {
    let mut service = WatcherService::new();
    let root = root();
    let now = Instant::now();
    service.register(root.clone(), path("D:/work/project"), true);
    service.on_path_event(&path("D:/work/project/a.txt"), now);
    service.on_path_event(
        &path("D:/work/project/b.txt"),
        now + Duration::from_millis(100),
    );
    assert!(service.poll(now + Duration::from_millis(199)).is_empty());
    assert_eq!(service.poll(now + Duration::from_millis(300)), vec![root]);
    assert!(service.poll(now + Duration::from_millis(301)).is_empty());
}

#[test]
fn watcher_continuous_events_review_within_one_second() {
    let mut service = WatcherService::new();
    let root = root();
    let now = Instant::now();
    service.register(root.clone(), path("D:/work/project"), true);
    for elapsed in [0, 400, 800, 999] {
        service.on_path_event(
            &path("D:/work/project/a.txt"),
            now + Duration::from_millis(elapsed),
        );
    }
    assert!(service.poll(now + Duration::from_millis(999)).is_empty());
    assert_eq!(service.poll(now + Duration::from_millis(1000)), vec![root]);
}

#[test]
fn explorer_hidden_refresh_coalesced() {
    let mut service = WatcherService::new();
    let root = root();
    let root_path = path("D:/work/project");
    let now = Instant::now();
    service.register(root.clone(), root_path.clone(), false);
    service.on_path_event(&path("D:/work/project/hidden.txt"), now);
    assert!(service.visible_roots().is_empty());
    assert!(service.poll(now + Duration::from_secs(2)).is_empty());

    service.set_visible(&root, true, now + Duration::from_secs(2));
    assert_eq!(service.visible_roots(), vec![root_path]);
    assert_eq!(
        service.poll(now + Duration::from_secs(2)),
        vec![root.clone()]
    );
    assert!(service.poll(now + Duration::from_secs(2)).is_empty());

    service.set_visible(&root, false, now + Duration::from_secs(3));
    service.on_path_event(
        &path("D:/work/project/hidden-again.txt"),
        now + Duration::from_secs(3),
    );
    service.set_visible(&root, true, now + Duration::from_secs(4));
    assert_eq!(service.poll(now + Duration::from_secs(4)), vec![root]);
}

#[test]
fn watcher_unregister_drops_pending_review() {
    let mut service = WatcherService::new();
    let root = root();
    let now = Instant::now();
    service.register(root.clone(), path("D:/work/project"), true);
    service.on_path_event(&path("D:/work/project/a.txt"), now);
    service.unregister(&root);

    assert!(service.visible_roots().is_empty());
    assert!(service.poll(now + Duration::from_millis(200)).is_empty());
}

#[test]
fn watcher_root_replacement_drops_old_path_events() {
    let mut service = WatcherService::new();
    let root = root();
    let now = Instant::now();
    service.register(root.clone(), path("D:/work/old"), true);
    service.on_path_event(&path("D:/work/old/a.txt"), now);
    service.register(root, path("D:/work/new"), true);

    service.on_path_event(&path("D:/work/old/stale.txt"), now);
    assert!(service.poll(now + Duration::from_millis(200)).is_empty());
    assert_eq!(service.visible_roots(), vec![path("D:/work/new")]);
}
