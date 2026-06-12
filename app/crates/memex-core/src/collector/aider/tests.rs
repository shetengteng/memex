use std::fs;

use tempfile::TempDir;

use super::*;
use crate::storage::models::Role;

#[test]
fn test_parse_aider_history() {
    let content = r#"# aider chat started at 2026-05-30 14:00:00

#### add a hello world function

Here's the function:

```python
def hello():
    print("hello world")
```

> Applied edit to main.py

#### now add tests

Sure, I'll add tests:

```python
def test_hello():
    hello()
```

> Applied edit to test_main.py
"#;
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join(HISTORY_FILENAME);
    fs::write(&file, content).unwrap();

    let adapter = AiderAdapter::with_scan_dirs(vec![tmp.path().to_path_buf()]);
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 1);

    let messages = adapter.collect(&sessions[0]).unwrap();
    assert!(
        messages.len() >= 4,
        "expected at least 4 messages, got {}",
        messages.len()
    );

    assert_eq!(messages[0].role, Role::User);
    assert!(messages[0].content.contains("hello world"));
}

#[test]
fn test_multiple_sessions_in_one_file() {
    let content = r#"# aider chat started at 2026-05-30 10:00:00

#### first session message

Response to first.

# aider chat started at 2026-05-30 14:00:00

#### second session message

Response to second.
"#;
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join(HISTORY_FILENAME);
    fs::write(&file, content).unwrap();

    let adapter = AiderAdapter::with_scan_dirs(vec![tmp.path().to_path_buf()]);
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 2);

    let m1 = adapter.collect(&sessions[0]).unwrap();
    assert!(m1[0].content.contains("first session"));

    let m2 = adapter.collect(&sessions[1]).unwrap();
    assert!(m2[0].content.contains("second session"));
}
