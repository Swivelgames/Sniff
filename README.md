# Swiss

**Don’t hem and haw about who moved your cheese.** 🧀

Swiss helps people and agents share a workspace. Claim the files you’re editing, check who’s working in a directory, or reserve a path before you create it.

## Install

Install Swiss with Cargo from any directory.

```sh
cargo install --locked swiss
```

## Claim your first path

```sh
swiss init
swiss claim --owner editor --reserve src/payments
swiss src/payments/refund.rs
swiss list
```

The claim returns an ID. Another caller checking the child sees which reservation covers it and the owner's label. A caller checking the parent sees which descendants are reserved. Checks are snapshots. Request a claim to acquire coverage.

Init stores claims in `.swiss/` and installs the bundled agent skill at `.agents/skills/swiss/SKILL.md`.
Re-running init restores a missing skill and preserves any existing edits.
If your agent does not discover that directory automatically, point it at the file.

## Commands

| Command | Effect |
| --- | --- |
| `swiss init [directory]` | Create a workspace at the supplied directory or CWD. Running it again retains every claim. |
| `swiss check <path>` or `swiss <path>` | Report clear, claimed, or undetermined. Use `swiss check -- <path>` for names that look like commands or options. |
| `swiss claim [paths...] [--reserve <path>]...` | Acquire all supplied paths under one ID. Positional paths must exist. Use repeatable `--reserve` for anticipated paths. |
| `swiss claim --claim-id <id> ...` | Add coverage to an active claim. Repeating an extension with its ID is safe. |
| `swiss list` | Show each active claim’s ID, owner label and reserved roots. |
| `swiss unclaim <claim-id>` | Release every path held by the specified ID. An already-absent ID succeeds. |
| `swiss rpc` or `swiss mcp` | Serve the same actions over stdio. |

Checks, claims, listings and releases discover the nearest ancestor workspace or accept an exact `--workspace <directory>`. Relative target paths resolve from the process working directory, including when a different workspace is selected. Structured clients can use absolute paths to choose both explicitly.

Supply `--json` for a machine-readable CLI result. A completed check exits zero whether clear or claimed. An undetermined check, a refused claim, or an invalid invocation exits nonzero. An acquisition conflict identifies the supplied path, overlapping location, claim ID, and optional owner. The ID alone identifies the claim for extension.

Claims persist across process exits. Copy an ID from `swiss list` and pass it to `swiss unclaim` to release its reservation. The files remain in place. Structured releases return the ID and a `released` boolean. A retry with an old ID returns `false` even if someone has since claimed the same paths under a new ID.

## Filesystem coverage

Checks and claims use the target directory’s naming rules, including when the store lives on a different volume. On a case-insensitive directory, reserving `Future` also covers `future/child` before either exists. On a case-sensitive directory these can be independent reservations. Existing stores keep their claim IDs and recorded spellings.

If two missing names might be equivalent and the filesystem cannot confirm it, Swiss reports an undetermined check and refuses the claim. Use the recorded spelling from `swiss list` to address an existing reservation.

## Shell completion

Run the registration command for your shell in the current session. Add the same command to your shell profile to enable completion in future sessions.

| Shell | Command |
| --- | --- |
| Bash | `source <(swiss completion bash)` |
| Zsh (after `compinit`) | `source <(swiss completion zsh)` |
| Fish | `swiss completion fish \| source` |
| PowerShell | `swiss completion powershell \| Out-String \| Invoke-Expression` |

Type `swiss unclaim ` and press Tab to see active claim IDs. Use `--workspace` to select another workspace. Path completion also works with shorthand checks and `claim --reserve`; you can type a future path directly.

## License

You may use Swiss under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE).
