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
    assert_eq!(
        service.poll(now + Duration::from_millis(300)),
        vec![root.clone()]
    );
    assert_eq!(
        service.take_dirty(&root),
        vec![path("D:/work/project/a.txt"), path("D:/work/project/b.txt")],
        "复核必须携带本轮合并的脏路径"
    );
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
    service.register(root.clone(), path("D:/work/new"), true);

    service.on_path_event(&path("D:/work/old/stale.txt"), now);
    assert!(service.poll(now + Duration::from_millis(200)).is_empty());
    assert!(
        service.take_dirty(&root).is_empty(),
        "根替换必须丢弃旧路径事件"
    );
    assert_eq!(service.visible_roots(), vec![path("D:/work/new")]);
}

#[test]
fn agent_files_visibility_controls_watchers() {
    use slterm_settings::AgentRootId;

    let agent_root =
        FileTreeRoot::Agent(AgentRootId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c").unwrap());
    let now = Instant::now();
    let mut service = WatcherService::new();
    service.register(agent_root.clone(), path("D:/agents/work"), true);
    assert_eq!(service.visible_roots(), vec![path("D:/agents/work")]);

    service.on_path_event(&path("D:/agents/work/a.txt"), now);
    assert_eq!(
        service.poll(now + Duration::from_millis(200)),
        vec![agent_root.clone()],
        "可见根的事件合并为一次复核"
    );
    assert!(
        service.poll(now + Duration::from_millis(400)).is_empty(),
        "同一次复核不得重复回填"
    );

    // 根收起/整个视图不可见：交出可见集合时移除该根，期间事件不累计。
    service.set_visible(&agent_root, false, now);
    assert!(service.visible_roots().is_empty());
    service.on_path_event(
        &path("D:/agents/work/b.txt"),
        now + Duration::from_millis(500),
    );
    assert!(
        service.poll(now + Duration::from_millis(10_000)).is_empty(),
        "不可见根的事件不得形成待复核"
    );

    // 重新可见只复核一次，随后回到静默。
    service.set_visible(&agent_root, true, now + Duration::from_millis(20_000));
    assert_eq!(service.visible_roots(), vec![path("D:/agents/work")]);
    assert_eq!(
        service.poll(now + Duration::from_millis(20_000)),
        vec![agent_root.clone()],
        "重新可见必须复核一次"
    );
    assert!(
        service.poll(now + Duration::from_millis(30_000)).is_empty(),
        "复核后不得再产生迟到回填"
    );
}
