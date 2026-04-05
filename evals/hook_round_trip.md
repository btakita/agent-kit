# Eval: Hook Round Trip

**Status:** planned

## Goal

Verify that the hook system's fire -> poll -> receive pipeline works end-to-end across all transport types.

## Test Cases

1. **Basic round trip** -- `fire("post_write", event)` then `poll("post_write", 0)` returns the event with correct fields
2. **Timestamp filtering** -- Events before `since_secs` are excluded from poll results
3. **TTL expiry** -- Events older than TTL are cleaned up on poll and not returned
4. **FileTransport delivery** -- `FileTransport::deliver` writes event that a separate `HookRegistry` can poll
5. **SocketTransport unavailable** -- Returns `is_available() == false` when no socket exists
6. **ChainTransport fallthrough** -- Socket unavailable, falls through to FileTransport successfully
7. **fire_and_deliver** -- Fires locally AND delivers to target sessions via transport
8. **Multiple events ordering** -- Multiple fires return in timestamp order from poll
9. **Nonexistent hook** -- `poll("nonexistent", 0)` returns empty vec, no error
10. **GC across hooks** -- `gc()` triggers poll-based cleanup on all hook directories

## Pass Criteria

All 10 cases pass. Events survive the full fire -> serialize -> deserialize -> poll cycle with data intact. No file descriptor leaks. Expired events are cleaned up.

## Coverage

Unit tests in `hooks.rs` cover cases 1-9. This eval adds case 10 and serves as the end-to-end acceptance spec.
