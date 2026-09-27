use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A path in the system temp directory that no other test run uses.
pub(crate) fn temp_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cstyle-{stamp}-{name}"))
}
