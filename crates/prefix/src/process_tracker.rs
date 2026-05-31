use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct ProcessTracker {
    running: HashMap<PathBuf, Vec<Child>>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        Self {
            running: HashMap::new(),
        }
    }

    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new()))
    }

    pub fn register(&mut self, exe_path: &Path, child: Child) {
        self.running
            .entry(exe_path.to_path_buf())
            .or_default()
            .push(child);
    }

    pub fn poll_dead(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        self.running.retain(|path, children| {
            children.retain_mut(|child| match child.try_wait() {
                Ok(Some(_)) => {
                    changed.push(path.clone());
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    changed.push(path.clone());
                    false
                }
            });
            !children.is_empty()
        });
        changed.sort();
        changed.dedup();
        changed
    }

    pub fn is_running(&self, exe_path: &Path) -> bool {
        self.running.contains_key(exe_path)
    }

    pub fn running_paths(&self) -> Vec<PathBuf> {
        self.running.keys().cloned().collect()
    }

    pub fn kill(&mut self, exe_path: &Path) -> bool {
        if let Some(children) = self.running.remove(exe_path) {
            for mut child in children {
                let _ = child.kill();
            }
            true
        } else {
            false
        }
    }
}
