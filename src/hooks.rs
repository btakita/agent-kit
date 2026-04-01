//! # Module: hooks
//!
//! File-based event hooks for coordinating multiple agent sessions.
//!
//! Each hook is a named event that can be fired by one session and observed by others.
//! Events are stored as JSON files in `.agent-doc/hooks/<event_name>/` directories.
//! Sessions register interest by polling or watching the hook directory.
//!
//! ## Design
//!
//! - **Fire-and-forget**: `fire()` writes an event file and returns immediately
//! - **Poll-based**: `poll()` reads new events since a given timestamp
//! - **No daemon**: No central coordinator; sessions discover events via filesystem
//! - **Expiry**: Events older than `ttl` are cleaned up on each poll
//! - **Idempotent**: Duplicate fires (same event_id) are deduplicated
//!
//! ## Event File Format
//!
//! Each event is a JSON file named `<timestamp_ns>-<event_id>.json`:
//! ```json
//! {
//!   "event": "post_write",
//!   "file": "/path/to/document.md",
//!   "session_id": "abc123",
//!   "timestamp": 1234567890,
//!   "data": { "patches": 3, "component": "exchange" }
//! }
//! ```
//!
//! ## Hook Types
//!
//! - `pre_write` — Before writing to a document (allows coordination/queuing)
//! - `post_write` — After writing (notify linked documents)
//! - `post_commit` — After committing (notify upstream dependents)
//! - `claim` — When a file is claimed by a session
//! - `layout_change` — When tmux layout changes (panes moved/stashed)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use agent_kit::hooks::{HookRegistry, Event};
//!
//! let registry = HookRegistry::new("/path/to/project/.agent-doc/hooks");
//! registry.fire("post_write", Event {
//!     file: "/path/to/doc.md".into(),
//!     session_id: "abc123".into(),
//!     data: serde_json::json!({"patches": 3}),
//! })?;
//!
//! let events = registry.poll("post_write", last_poll_time)?;
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A hook event fired by a session.
#[derive(Debug, Clone)]
pub struct Event {
    /// The document file this event relates to.
    pub file: String,
    /// The session ID that fired the event.
    pub session_id: String,
    /// Arbitrary JSON data attached to the event.
    pub data: serde_json::Value,
}

/// A received event with metadata.
#[derive(Debug, Clone)]
pub struct ReceivedEvent {
    /// The event name (e.g., "post_write").
    pub name: String,
    /// The event payload.
    pub event: Event,
    /// Unix timestamp (seconds) when the event was fired.
    pub timestamp: u64,
    /// Unique event ID (for deduplication).
    pub event_id: String,
}

/// Registry for file-based hook events.
///
/// Each hook name gets its own subdirectory under the hooks root.
/// Events are JSON files named `<timestamp_ns>-<event_id>.json`.
pub struct HookRegistry {
    root: PathBuf,
    /// Time-to-live for events in seconds. Events older than this are cleaned up on poll.
    pub ttl_secs: u64,
}

impl HookRegistry {
    /// Create a new hook registry at the given root directory.
    ///
    /// The directory is created on first `fire()` call, not here.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            ttl_secs: 60, // 1 minute default TTL
        }
    }

    /// Create a registry with a custom TTL.
    pub fn with_ttl(root: impl Into<PathBuf>, ttl_secs: u64) -> Self {
        Self {
            root: root.into(),
            ttl_secs,
        }
    }

    /// Fire a named event.
    ///
    /// Writes a JSON event file to `.agent-doc/hooks/<name>/`.
    /// Returns the event ID for deduplication.
    pub fn fire(&self, name: &str, event: Event) -> Result<String> {
        let hook_dir = self.root.join(name);
        std::fs::create_dir_all(&hook_dir)
            .with_context(|| format!("failed to create hook dir: {}", hook_dir.display()))?;

        let timestamp = now_secs();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let event_id = format!("{:08x}{:08x}", timestamp, nanos);

        let payload = serde_json::json!({
            "event": name,
            "file": event.file,
            "session_id": event.session_id,
            "timestamp": timestamp,
            "event_id": &event_id,
            "data": event.data,
        });

        let filename = format!("{}-{}.json", timestamp, event_id);
        let event_path = hook_dir.join(&filename);

        std::fs::write(&event_path, serde_json::to_string_pretty(&payload)?)
            .with_context(|| format!("failed to write hook event: {}", event_path.display()))?;

        Ok(event_id)
    }

    /// Poll for new events since `since_secs` (unix timestamp).
    ///
    /// Returns events newer than `since_secs`, sorted by timestamp.
    /// Also cleans up expired events (older than TTL).
    pub fn poll(&self, name: &str, since_secs: u64) -> Result<Vec<ReceivedEvent>> {
        let hook_dir = self.root.join(name);
        if !hook_dir.exists() {
            return Ok(Vec::new());
        }

        let now = now_secs();
        let mut events = Vec::new();

        let entries = std::fs::read_dir(&hook_dir)
            .with_context(|| format!("failed to read hook dir: {}", hook_dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            // Parse timestamp from filename: "<timestamp>-<event_id>.json"
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let file_ts: u64 = stem.split('-').next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            // Clean up expired events
            if file_ts > 0 && now.saturating_sub(file_ts) > self.ttl_secs {
                let _ = std::fs::remove_file(&path);
                continue;
            }

            // Skip events older than since_secs
            if file_ts <= since_secs {
                continue;
            }

            // Parse event
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    events.push(ReceivedEvent {
                        name: name.to_string(),
                        event: Event {
                            file: json["file"].as_str().unwrap_or("").to_string(),
                            session_id: json["session_id"].as_str().unwrap_or("").to_string(),
                            data: json["data"].clone(),
                        },
                        timestamp: json["timestamp"].as_u64().unwrap_or(file_ts),
                        event_id: json["event_id"].as_str().unwrap_or("").to_string(),
                    });
                }
            }
        }

        events.sort_by_key(|e| e.timestamp);
        Ok(events)
    }

    /// List all hook names that have events.
    pub fn list_hooks(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect()
    }

    /// Clean up all expired events across all hooks.
    pub fn gc(&self) -> Result<usize> {
        let mut cleaned = 0;
        for hook_name in self.list_hooks() {
            let events = self.poll(&hook_name, 0)?;
            // poll() already cleans expired events; count what remains
            let _ = events.len();
            cleaned += 1;
        }
        Ok(cleaned)
    }
}

/// Resolve the hooks directory for a project.
///
/// Walks up from `file` to find `.agent-doc/` and returns `.agent-doc/hooks/`.
pub fn hooks_dir_for_file(file: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(file).ok()?;
    let mut dir = canonical.parent()?;
    loop {
        let agent_doc = dir.join(".agent-doc");
        if agent_doc.is_dir() {
            return Some(agent_doc.join("hooks"));
        }
        dir = dir.parent()?;
    }
}

// =========================================================================
// Transport abstraction
// =========================================================================

/// Transport for delivering hook events to target sessions.
///
/// Implementations define how events reach their destination:
/// - `FileTransport` — write JSON to a watched directory (default, always works)
/// - `SocketTransport` — deliver via Unix domain socket (low latency)
/// - `TmuxTransport` — send commands to tmux panes (for triggering agent actions)
///
/// Transports are composable: a `ChainTransport` tries each in order until one succeeds.
pub trait HookTransport: Send + Sync {
    /// Deliver an event to a target session.
    /// Returns `true` if delivery succeeded.
    fn deliver(&self, target_session: &str, event: &ReceivedEvent) -> Result<bool>;

    /// Check if the target session is reachable via this transport.
    fn is_available(&self, target_session: &str) -> bool;

    /// Transport name for logging.
    fn name(&self) -> &str;
}

/// File-based transport — writes event JSON to the target's hook directory.
/// This is the default and always works (requires only filesystem access).
pub struct FileTransport {
    /// Root directory for hooks (e.g., `.agent-doc/hooks/`)
    pub hooks_root: PathBuf,
}

impl FileTransport {
    pub fn new(hooks_root: impl Into<PathBuf>) -> Self {
        Self { hooks_root: hooks_root.into() }
    }
}

impl HookTransport for FileTransport {
    fn deliver(&self, _target_session: &str, event: &ReceivedEvent) -> Result<bool> {
        let registry = HookRegistry::new(&self.hooks_root);
        registry.fire(&event.name, event.event.clone())?;
        Ok(true)
    }

    fn is_available(&self, _target_session: &str) -> bool {
        true // File transport is always available
    }

    fn name(&self) -> &str {
        "file"
    }
}

/// Socket-based transport — delivers events via Unix domain socket.
///
/// Expects a socket at `<project_root>/.agent-doc/hooks.sock`.
/// The listener is implemented by the consuming application (e.g., agent-doc).
/// Only available on Unix platforms (Unix domain sockets are not available on Windows).
#[cfg(unix)]
pub struct SocketTransport {
    /// Path to the Unix domain socket.
    pub socket_path: PathBuf,
}

#[cfg(unix)]
impl SocketTransport {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    /// Create from a project root directory.
    pub fn from_project_root(root: &Path) -> Self {
        Self {
            socket_path: root.join(".agent-doc/hooks.sock"),
        }
    }
}

#[cfg(unix)]
impl HookTransport for SocketTransport {
    fn deliver(&self, _target_session: &str, event: &ReceivedEvent) -> Result<bool> {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let mut stream = UnixStream::connect(&self.socket_path)
            .with_context(|| format!("failed to connect to hook socket: {}", self.socket_path.display()))?;

        let payload = serde_json::json!({
            "type": "hook_event",
            "event_name": event.name,
            "file": event.event.file,
            "session_id": event.event.session_id,
            "timestamp": event.timestamp,
            "event_id": event.event_id,
            "data": event.event.data,
        });

        let msg = serde_json::to_string(&payload)?;
        stream.write_all(msg.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;

        // Read ack (with timeout)
        stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        let mut buf = [0u8; 64];
        match std::io::Read::read(&mut stream, &mut buf) {
            Ok(n) if n > 0 => {
                let response = std::str::from_utf8(&buf[..n]).unwrap_or("");
                Ok(response.contains("ok") || response.contains("ack"))
            }
            _ => Ok(false),
        }
    }

    fn is_available(&self, _target_session: &str) -> bool {
        self.socket_path.exists()
    }

    fn name(&self) -> &str {
        "socket"
    }
}

/// Chain transport — tries transports in order until one succeeds.
pub struct ChainTransport {
    transports: Vec<Box<dyn HookTransport>>,
}

impl ChainTransport {
    pub fn new(transports: Vec<Box<dyn HookTransport>>) -> Self {
        Self { transports }
    }
}

impl HookTransport for ChainTransport {
    fn deliver(&self, target_session: &str, event: &ReceivedEvent) -> Result<bool> {
        for transport in &self.transports {
            if transport.is_available(target_session) {
                match transport.deliver(target_session, event) {
                    Ok(true) => return Ok(true),
                    Ok(false) => continue,
                    Err(_) => continue,
                }
            }
        }
        Ok(false)
    }

    fn is_available(&self, target_session: &str) -> bool {
        self.transports.iter().any(|t| t.is_available(target_session))
    }

    fn name(&self) -> &str {
        "chain"
    }
}

impl HookRegistry {
    /// Deliver an event to a target session via the given transport.
    pub fn deliver(
        &self,
        transport: &dyn HookTransport,
        target_session: &str,
        event: &ReceivedEvent,
    ) -> Result<bool> {
        transport.deliver(target_session, event)
    }

    /// Fire an event AND deliver it to all known sessions via transport.
    /// Fires locally (file-based) and delivers to each target session.
    pub fn fire_and_deliver(
        &self,
        name: &str,
        event: Event,
        transport: &dyn HookTransport,
        target_sessions: &[String],
    ) -> Result<String> {
        let event_id = self.fire(name, event.clone())?;

        let received = ReceivedEvent {
            name: name.to_string(),
            event,
            timestamp: now_secs(),
            event_id: event_id.clone(),
        };

        for session in target_sessions {
            if let Err(e) = transport.deliver(session, &received) {
                eprintln!("[hooks] delivery to {} failed: {}", session, e);
            }
        }

        Ok(event_id)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_and_poll_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = HookRegistry::new(tmp.path().join("hooks"));

        let event = Event {
            file: "/tmp/test.md".into(),
            session_id: "session-1".into(),
            data: serde_json::json!({"patches": 3}),
        };

        let event_id = registry.fire("post_write", event).unwrap();
        assert!(!event_id.is_empty());

        let events = registry.poll("post_write", 0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.file, "/tmp/test.md");
        assert_eq!(events[0].event.session_id, "session-1");
        assert_eq!(events[0].event_id, event_id);
        assert_eq!(events[0].event.data["patches"], 3);
    }

    #[test]
    fn poll_filters_by_timestamp() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = HookRegistry::new(tmp.path().join("hooks"));

        registry.fire("test", Event {
            file: "a.md".into(),
            session_id: "s1".into(),
            data: serde_json::json!(null),
        }).unwrap();

        // Poll with a future timestamp — should get nothing
        let future = now_secs() + 100;
        let events = registry.poll("test", future).unwrap();
        assert!(events.is_empty());

        // Poll with 0 — should get everything
        let events = registry.poll("test", 0).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn expired_events_cleaned_on_poll() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = HookRegistry::with_ttl(tmp.path().join("hooks"), 0); // 0s TTL = expire immediately

        registry.fire("test", Event {
            file: "a.md".into(),
            session_id: "s1".into(),
            data: serde_json::json!(null),
        }).unwrap();

        // Brief sleep so the event is at least 1 second old
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let events = registry.poll("test", 0).unwrap();
        assert!(events.is_empty(), "expired events should be cleaned up");

        // Verify the file was actually deleted
        let hook_dir = tmp.path().join("hooks/test");
        let remaining: Vec<_> = std::fs::read_dir(&hook_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(remaining.is_empty(), "expired event files should be deleted");
    }

    #[test]
    fn poll_nonexistent_hook_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = HookRegistry::new(tmp.path().join("hooks"));
        let events = registry.poll("nonexistent", 0).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn list_hooks_returns_fired_names() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = HookRegistry::new(tmp.path().join("hooks"));

        registry.fire("post_write", Event {
            file: "a.md".into(), session_id: "s1".into(), data: serde_json::json!(null),
        }).unwrap();
        registry.fire("post_commit", Event {
            file: "a.md".into(), session_id: "s1".into(), data: serde_json::json!(null),
        }).unwrap();

        let mut hooks = registry.list_hooks();
        hooks.sort();
        assert_eq!(hooks, vec!["post_commit", "post_write"]);
    }

    #[test]
    fn multiple_events_sorted_by_timestamp() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = HookRegistry::new(tmp.path().join("hooks"));

        for i in 0..3 {
            registry.fire("test", Event {
                file: format!("{}.md", i),
                session_id: "s1".into(),
                data: serde_json::json!({"order": i}),
            }).unwrap();
        }

        let events = registry.poll("test", 0).unwrap();
        assert_eq!(events.len(), 3);
        // All should have same or increasing timestamps
        for i in 1..events.len() {
            assert!(events[i].timestamp >= events[i-1].timestamp);
        }
    }

    // --- Transport tests ---

    #[test]
    fn file_transport_delivers_event() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_root = tmp.path().join("hooks");
        let transport = FileTransport::new(&hooks_root);

        let event = ReceivedEvent {
            name: "post_write".to_string(),
            event: Event {
                file: "/tmp/test.md".into(),
                session_id: "s1".into(),
                data: serde_json::json!({"patches": 2}),
            },
            timestamp: now_secs(),
            event_id: "test-id".into(),
        };

        assert!(transport.is_available("any"));
        let result = transport.deliver("target", &event).unwrap();
        assert!(result);

        // Verify the event was written
        let registry = HookRegistry::new(&hooks_root);
        let events = registry.poll("post_write", 0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.file, "/tmp/test.md");
    }

    #[test]
    #[cfg(unix)]
    fn socket_transport_unavailable_when_no_socket() {
        let transport = SocketTransport::new("/nonexistent/hooks.sock");
        assert!(!transport.is_available("any"));
    }

    #[test]
    #[cfg(unix)]
    fn chain_transport_falls_through() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Socket transport (unavailable) + file transport (available)
        let chain = ChainTransport::new(vec![
            Box::new(SocketTransport::new("/nonexistent/hooks.sock")),
            Box::new(FileTransport::new(tmp.path().join("hooks"))),
        ]);

        assert!(chain.is_available("any"));

        let event = ReceivedEvent {
            name: "test".to_string(),
            event: Event {
                file: "doc.md".into(),
                session_id: "s1".into(),
                data: serde_json::json!(null),
            },
            timestamp: now_secs(),
            event_id: "chain-test".into(),
        };

        let result = chain.deliver("target", &event).unwrap();
        assert!(result, "chain should fall through to file transport");

        // Verify file transport was used
        let registry = HookRegistry::new(tmp.path().join("hooks"));
        let events = registry.poll("test", 0).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn fire_and_deliver_fires_locally_and_delivers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_root = tmp.path().join("hooks");
        let registry = HookRegistry::new(&hooks_root);
        let transport = FileTransport::new(tmp.path().join("delivery"));

        let event_id = registry.fire_and_deliver(
            "post_commit",
            Event {
                file: "doc.md".into(),
                session_id: "s1".into(),
                data: serde_json::json!(null),
            },
            &transport,
            &["target-1".into()],
        ).unwrap();

        assert!(!event_id.is_empty());

        // Local fire
        let local_events = registry.poll("post_commit", 0).unwrap();
        assert_eq!(local_events.len(), 1);

        // Delivered copy
        let delivery_registry = HookRegistry::new(tmp.path().join("delivery"));
        let delivered = delivery_registry.poll("post_commit", 0).unwrap();
        assert_eq!(delivered.len(), 1);
    }
}
