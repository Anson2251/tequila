use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};

/// Tracks running Wine child processes, keyed by the executable path they were launched from.
/// Multiple instances of the same executable can run concurrently.
#[derive(Debug)]
pub struct ProcessTracker {
    running: HashMap<PathBuf, Vec<Child>>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        Self { running: HashMap::new() }
    }

    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new()))
    }

    /// Register a newly spawned child process for the given executable path.
    pub fn register(&mut self, exe_path: &Path, child: Child) {
        self.running.entry(exe_path.to_path_buf()).or_default().push(child);
    }

    /// Reap dead children. Returns the set of executable paths that changed
    /// (i.e. had at least one process exit).
    pub fn poll_dead(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        self.running.retain(|path, children| {
            children.retain_mut(|child| {
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        changed.push(path.clone());
                        false // remove dead child
                    }
                    Ok(None) => true, // still running
                    Err(_) => {
                        changed.push(path.clone());
                        false
                    }
                }
            });
            !children.is_empty() // keep entry if any children still running
        });
        changed.sort();
        changed.dedup();
        changed
    }

    /// Check if any instance of the given executable is running.
    pub fn is_running(&self, exe_path: &Path) -> bool {
        self.running.contains_key(exe_path)
    }

    /// Return the set of executable paths that currently have running processes.
    pub fn running_paths(&self) -> Vec<PathBuf> {
        self.running.keys().cloned().collect()
    }

    /// Kill all running processes for the given executable and return true if any were killed.
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
