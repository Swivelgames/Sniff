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

Checks and claims use the target directory’s naming rules, including when the store lives on a different volume. On a case-insensitive directory, reserving `Future` also covers `future/child` before either exists. On a case-sensitive directory these can be independent reservations. Existing stores keep their claim IDs and recorded spellings.

Sniff queries the Windows directory case flag, the macOS volume case setting, and the case flags of Linux ext4 and tmpfs directories. On Linux ZFS it reads the dataset’s reported case mode. Byte-sensitive directories with a known exact-name policy can compare arbitrary native names. Where the filesystem can normalize or fold names, Sniff compares ASCII spellings and uses actual directory lookup to resolve other spellings that exist. If the relevant spellings are both missing and their equivalence cannot be established, the check returns `undetermined` and acquisition reports `path_unresolved`. Use the spelling from `sniff list` when addressing an existing reservation. Target directories need no writable probe files.

For native validation, run `cargo test --locked --test portability -- --nocapture`. The suite prints the file lookup it used as an oracle. Set `SNIFF_PORTABILITY_DIR` to an existing directory to exercise a particular target volume or casefold-enabled directory. Run against both naming modes where available. The native acceptance suite requires working filename rules on the selected volume. Native Windows and macOS runtime verification is pending.

Linux overlayfs and other unrecognized filesystems can return `undetermined` even for two differently named missing entries. Sniff can still recognize identical spellings and inspect or release reservations. To verify conservative refusal on a deliberately unsupported target, set `SNIFF_PORTABILITY_DIR` to that directory and run `cargo test --locked --test portability unknown_target_refuses -- --ignored --nocapture`. Run the regular suite separately on supported storage, setting `TMPDIR` there as well if needed.

## Development

The executable uses Trine pinned to a Git revision. Building requires read access to the private Trine repository.

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
```
