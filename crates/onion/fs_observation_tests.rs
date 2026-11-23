//! `fs_observation` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn observation() -> FsObservation {
    FsObservation::new()
}

#[test]
fn unseen_target_is_rejected() {
    let table = observation();
    let error = table.guard("a.txt", Some("v1")).unwrap_err();
    assert!(error.contains("FS_NOT_OBSERVED"), "unexpected error: {error}");
    assert!(error.contains("has not been read"), "unexpected error: {error}");
}

#[test]
fn observed_and_fresh_passes_without_stale_token() {
    let table = observation();
    table.refresh("a.txt", version_of(b"hello"));
    assert_eq!(table.guard("a.txt", Some(&version_of(b"hello"))), Ok(None));
}

#[test]
fn changed_since_read_yields_stale_token() {
    let table = observation();
    let observed = version_of(b"before");
    table.refresh("a.txt", observed.clone());
    let result = table.guard("a.txt", Some(&version_of(b"after")));
    assert_eq!(result, Ok(Some(observed)));
}

#[test]
fn observed_present_but_now_missing_yields_stale_token() {
    let table = observation();
    let observed = version_of(b"before");
    table.refresh("a.txt", observed.clone());
    assert_eq!(table.guard("a.txt", None), Ok(Some(observed)));
}

#[test]
fn observed_absent_allows_create_creation() {
    let table = observation();
    table.record("new.txt", observed_absent());
    // 当前仍缺失 → 放行且无过期凭证（创建路径）。
    assert_eq!(table.guard("new.txt", None), Ok(None));
}

#[test]
fn observed_absent_but_now_exists_is_rejected() {
    let table = observation();
    table.record("new.txt", observed_absent());
    let error = table.guard("new.txt", Some(&version_of(b"other"))).unwrap_err();
    assert!(error.contains("FS_NOT_FOUND"), "unexpected error: {error}");
}

#[test]
fn keys_are_normalized_across_separators() {
    let table = observation();
    table.refresh("dir\\nested\\a.txt", version_of(b"x"));
    // 不同分隔符写法命中同一观测。
    assert_eq!(table.guard("./dir/nested/a.txt", Some(&version_of(b"x"))), Ok(None));
}

#[test]
fn clear_drops_all_observations() {
    let table = observation();
    table.refresh("a.txt", version_of(b"x"));
    table.clear();
    assert!(table.guard("a.txt", Some(&version_of(b"x"))).is_err());
}

#[test]
fn digest_is_content_addressed_and_stable() {
    assert_eq!(version_of(b"abc"), version_of(b"abc"));
    assert_ne!(version_of(b"abc"), version_of(b"abd"));
}
