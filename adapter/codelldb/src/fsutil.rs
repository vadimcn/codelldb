use std::path::{Component, Path, PathBuf};

/// Remove '..' in path.
pub fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
        }
    }
    normalized
}

#[allow(unused)]
pub fn is_same_path(path1: &Path, path2: &Path) -> bool {
    if path1 == path2 {
        true
    } else {
        match (path1.canonicalize(), path2.canonicalize()) {
            (Ok(path1), Ok(path2)) => path1 == path2,
            _ => false,
        }
    }
}

/// Quote and escape the input string.
pub fn lldb_quoted_string(input: &str) -> String {
    let mut result = String::with_capacity(input.len() + 2);
    result.push('"');
    for c in input.chars() {
        match c {
            '\\' => result.push_str(r#"\\"#),
            '"' => result.push_str(r#"\""#),
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}

#[test]
fn test_normalize_path() {
    assert_eq!(normalize_path("/foo/bar"), Path::new("/foo/bar"));
    assert_eq!(normalize_path("foo/bar"), Path::new("foo/bar"));
    assert_eq!(normalize_path("/foo/bar/./baz/./../"), Path::new("/foo/bar"));
    assert_eq!(normalize_path(r"c:\foo\bar/./baz/./../"), Path::new(r"c:\foo\bar"));
    #[cfg(windows)]
    assert_eq!(
        normalize_path(r"C:/QQQ/WWW/..\..\FOO/\bar.baz"),
        Path::new(r"c:\FOO/bar.baz")
    );
}

#[test]
fn test_lldb_quoting() {
    let quoted = lldb_quoted_string(&r#"foo " 'bar \ baz"#);
    assert_eq!(quoted, r#""foo \" 'bar \\ baz""#)
}
