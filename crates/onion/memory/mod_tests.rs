//! `mod` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use serde_json::Value;

#[test]
fn memory_error_display_is_informative() {
    let io = MemoryError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
    assert_eq!(io.to_string(), "io: gone");
    let path = MemoryError::InvalidPath("bad".into());
    assert_eq!(path.to_string(), "invalid memory path: bad");
    let json = MemoryError::Json(serde_json::from_str::<Value>("{").unwrap_err());
    assert!(json.to_string().starts_with("json:"));
}

#[test]
fn result_alias_is_std_result() {
    fn ok() -> Result<u32> {
        Ok(42)
    }
    assert_eq!(ok().unwrap(), 42);
}
