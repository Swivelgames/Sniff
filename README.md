# Sniff

Sniff lets cooperating people and agents reserve filesystem paths in a shared workspace. A claim covers its resolved location and every descendant, including files that have yet to be created.

```sh
sniff init
sniff claim --owner editor --reserve src/payments
sniff src/payments/refund.rs
sniff list
```

The claim returns an ID. Another caller checking the child sees which reservation covers it and the owner's label. A caller checking the parent sees which descendants are reserved. Checks are snapshots. Request a claim to acquire coverage.

## Commands

| Command | Effect |
| --- | --- |
| `sniff init [directory]` | Create a workspace at the supplied directory or CWD. Running it again retains every claim. |
| `sniff check <path>` or `sniff <path>` | Report clear, claimed, or undetermined. Use `sniff check -- <path>` for names that look like commands or options. |
| `sniff claim [paths...] [--reserve <path>]...` | Acquire all supplied paths under one ID. Positional paths must exist. Use repeatable `--reserve` for anticipated paths. |
| `sniff claim --claim-id <id> ...` | Add coverage to an active claim. Repeating an extension with its ID is safe. |
| `sniff list` | Show each active claim’s ID, owner label and reserved roots. |
| `sniff unclaim <claim-id>` | Release every path held by the specified ID. An already-absent ID succeeds. |
| `sniff rpc` or `sniff mcp` | Serve the same actions over stdio. |

Checks, claims, listings and releases discover the nearest ancestor workspace or accept an exact `--workspace <directory>`. Relative target paths resolve from the process working directory, including when a different workspace is selected. Structured clients can use absolute paths to choose both explicitly.

Supply `--json` for a machine-readable CLI result. A completed check exits zero whether clear or claimed. An undetermined check, a refused claim, or an invalid invocation exits nonzero. An acquisition conflict identifies the supplied path, overlapping location, claim ID, and optional owner. The ID alone identifies the claim for extension.

Claims persist across process exits. Copy an ID from `sniff list` and pass it to `sniff unclaim` to release its reservation. The files remain in place. Structured releases return the ID and a `released` boolean. A retry with an old ID returns `false` even if someone has since claimed the same paths under a new ID.

## Filesystem coverage

This slice is verified on case-sensitive Linux filesystems. On a case-insensitive volume, two spellings of an anticipated path (such as `Future` and `future`) can receive separate claim IDs. Use a case-sensitive volume for this preview. Native filename equivalence and Windows/macOS validation are scheduled for the portability increment.

## Development

The executable uses Trine pinned to a Git revision. Building requires read access to the private Trine repository.

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
```
