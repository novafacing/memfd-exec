//! Wholesale copied from rust-lang/rust/library/std/src/sys_common/process.rs

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};

// Stores a set of changes to an environment
#[derive(Clone, Debug, Default)]
pub struct CommandEnv {
    clear: bool,
    saw_path: bool,
    vars: BTreeMap<OsString, Option<OsString>>,
}

impl CommandEnv {
    // Capture the current environment with these changes applied
    pub fn capture(&self) -> BTreeMap<OsString, OsString> {
        let mut result = BTreeMap::<OsString, OsString>::new();
        if !self.clear {
            for (k, v) in env::vars_os() {
                result.insert(k, v);
            }
        }
        for (k, maybe_v) in &self.vars {
            match maybe_v {
                Some(v) => {
                    result.insert(k.clone(), v.clone());
                }
                _ => {
                    result.remove(k);
                }
            }
        }
        result
    }

    pub fn is_unchanged(&self) -> bool {
        !self.clear && self.vars.is_empty()
    }

    pub fn capture_if_changed(&self) -> Option<BTreeMap<OsString, OsString>> {
        if self.is_unchanged() {
            None
        } else {
            Some(self.capture())
        }
    }

    // The following functions build up changes
    pub fn set(&mut self, key: &OsStr, value: &OsStr) {
        let key = OsString::from(key);
        self.maybe_saw_path(&key);
        self.vars.insert(key, Some(value.to_owned()));
    }

    pub fn remove(&mut self, key: &OsStr) {
        let key = OsString::from(key);
        self.maybe_saw_path(&key);
        if self.clear {
            self.vars.remove(&key);
        } else {
            self.vars.insert(key, None);
        }
    }

    pub fn clear(&mut self) {
        self.clear = true;
        self.vars.clear();
    }

    pub fn have_changed_path(&self) -> bool {
        self.saw_path || self.clear
    }

    fn maybe_saw_path(&mut self, key: &OsString) {
        if !self.saw_path && key == "PATH" {
            self.saw_path = true;
        }
    }
}

#[derive(Debug)]
pub struct CommandEnvs<'a> {
    iter: std::collections::btree_map::Iter<'a, OsString, Option<OsString>>,
}

impl<'a> Iterator for CommandEnvs<'a> {
    type Item = (&'a OsStr, Option<&'a OsStr>);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|(key, value)| (key.as_ref(), value.as_deref()))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> ExactSizeIterator for CommandEnvs<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}
