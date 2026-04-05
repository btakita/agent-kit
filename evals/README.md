# agent-kit Evals

Planned evaluations for verifying agent-kit behavior.

## Eval Index

| Eval | Status | What it verifies |
|------|--------|------------------|
| [environment_detection](environment_detection.md) | planned | `detect()` correctly identifies each environment from env vars |
| [context_invariant](context_invariant.md) | planned | `check_context_invariant` catches all machine-local path patterns |
| [hook_round_trip](hook_round_trip.md) | planned | fire -> poll -> receive works end-to-end across transports |

## Running

Evals are designed to run as standalone test scenarios. Each eval document describes inputs, expected outputs, and pass criteria.
