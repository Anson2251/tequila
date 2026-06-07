use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct ProcessTracker {
    /// Children owned by the tracker (normal launch).
    running: HashMap<PathBuf, Vec<Child>>,
    /// PIDs tracked separately where the caller keeps the Child handle
    /// (e.g. debug mode — the debug window owns the Child for pipe I/O).
    extra_pids: HashMap<PathBuf, Vec<u32>>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        Self {
            running: HashMap::new(),
            extra_pids: HashMap::new(),
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

    /// Track a PID separately (caller keeps the Child handle).
    /// Used for debug-mode launches where the debug window needs the Child.
    pub fn track_pid(&mut self, exe_path: &Path, pid: u32) {
        self.extra_pids
            .entry(exe_path.to_path_buf())
            .or_default()
            .push(pid);
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
        // Also clean up dead extra PIDs
        self.extra_pids.retain(|path, pids| {
            pids.retain(|pid| {
                let status = std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .output();
                // keep if process exists (kill -0 succeeds)
                status.map(|s| s.status.success()).unwrap_or(false)
            });
            !pids.is_empty()
        });
        changed.sort();
        changed.dedup();
        changed
    }

    pub fn is_running(&self, exe_path: &Path) -> bool {
        self.running.contains_key(exe_path) || self.extra_pids.contains_key(exe_path)
    }

    pub fn running_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.running.keys().cloned().collect();
        paths.extend(self.extra_pids.keys().cloned());
        paths.sort();
        paths.dedup();
        paths
    }

    pub fn kill(&mut self, exe_path: &Path) -> bool {
        let mut killed = false;
        if let Some(children) = self.running.remove(exe_path) {
            for mut child in children {
                let _ = child.kill();
                killed = true;
            }
        }
        if let Some(pids) = self.extra_pids.remove(exe_path) {
            for pid in pids {
                let _ = std::process::Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .output();
                killed = true;
            }
        }
        killed
    }

    /// Kill all tracked processes. Returns the number of processes killed.
    pub fn kill_all(&mut self) -> usize {
        let mut count = 0;
        // Kill registered children
        for (_path, children) in self.running.drain() {
            for mut child in children {
                if child.kill().is_ok() {
                    count += 1;
                }
            }
        }
        // Kill PIDs tracked separately (debug mode, etc.)
        for (_path, pids) in self.extra_pids.drain() {
            for pid in pids {
                let _ = std::process::Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .output();
                count += 1;
            }
        }
        count
    }

    /// Number of running processes (total across all executable paths).
    pub fn count(&self) -> usize {
        self.running.values().map(|v| v.len()).sum::<usize>()
            + self.extra_pids.values().map(|v| v.len()).sum::<usize>()
    }
}
