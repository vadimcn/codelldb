use crate::error::Error;
use globset::*;
use std::path::{Component, Path, PathBuf};

pub struct SourceMap {
    glob_set: GlobSet,
    local_prefixes: Vec<Option<String>>,
}

impl SourceMap {
    pub fn empty() -> SourceMap {
        SourceMap {
            glob_set: GlobSetBuilder::new().build().unwrap(),
            local_prefixes: vec![],
        }
    }

    pub fn new<R, L>(source_map: impl IntoIterator<Item = (R, Option<L>)>) -> Result<SourceMap, Error>
    where
        R: AsRef<str>,
        L: AsRef<str>,
    {
        let mut builder = GlobSetBuilder::new();
        let mut locals = vec![];
        for (remote, local) in source_map.into_iter() {
            let glob = match Glob::new(remote.as_ref()) {
                Ok(glob) => glob,
                Err(err) => return Err(Error::UserError(format!("Invalid glob pattern: {}: {}", remote.as_ref(), err))),
            };
            builder.add(glob);
            locals.push(local.map(|l| l.as_ref().to_owned()));
        }
        let glob_set = builder.build().unwrap();
        Ok(SourceMap {
            glob_set: glob_set,
            local_prefixes: locals,
        })
    }

    pub fn to_local(&self, path: impl AsRef<Path>) -> Option<PathBuf> {
        let path = path.as_ref();

        let mut matches = vec![];
        let mut remote_prefix = PathBuf::new();
        let mut components = path.components();
        for component in path.components() {
            components.next();
            remote_prefix.push(component);
            self.glob_set.matches_into(&remote_prefix, &mut matches);
            if !matches.is_empty() {
                let localized = match self.local_prefixes[matches[0]] {
                    None => None,
                    Some(ref local) => {
                        let mut localized = PathBuf::from(&local);
                        localized.push(components.as_path()); // Append remainder of the path
                        Some(normalize_path(&localized))
                    }
                };
                return localized;
            }
        }
        Some(normalize_path(path))
    }
}

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

#[test]
fn test_normalize_path() {
    assert_eq!(normalize_path("/foo/bar"), Path::new("/foo/bar"));
    assert_eq!(normalize_path("foo/bar"), Path::new("foo/bar"));
    assert_eq!(normalize_path("/foo/bar/./baz/./../"), Path::new("/foo/bar"));
    assert_eq!(normalize_path(r"c:\foo\bar/./baz/./../"), Path::new(r"c:\foo\bar"));
    assert_eq!(normalize_path(r"C:/FOO\bar.baz"), Path::new(r"c:\FOO/bar.baz"));
}

#[test]
fn test_source_map() {
    let mappings = [("/foo/*", Some("/quoox")), ("/suppress/*", None)];
    let it = mappings.iter().map(|(r, l)| (*r, *l));
    let map = SourceMap::new(it).unwrap();

    assert_eq!(map.to_local("/aaaa/bbbbb/baz.cpp"), Some("/aaaa/bbbbb/baz.cpp".into()));
    assert_eq!(map.to_local("/foo/bar/baz.cpp"), Some("/quoox/baz.cpp".into()));
    assert_eq!(map.to_local("/suppress/foo/baz.cpp"), None);
}
