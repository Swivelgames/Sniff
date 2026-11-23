# Sniff

**Give people and agents room to work in the same tree.** Sniff shows who has claimed a file or directory before you start editing. Reserve a whole subtree with one claim, including files you plan to create. Everyone using the same workspace can see the reservation.

## Claim your first path

```sh
sniff init
sniff claim --owner editor --reserve src/payments
sniff src/payments/refund.rs
sniff list
```

The claim returns an ID. Another caller checking the child sees which reservation covers it and the owner's label. A caller checking the parent sees which descendants are reserved. Checks are snapshots. Request a claim to acquire coverage.

Init installs the bundled agent skill at `.agents/skills/sniff/SKILL.md`.
Re-running init restores a missing skill and preserves any existing edits.
If your agent does not discover that directory automatically, point it at the file.

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

If two missing names might be equivalent and the filesystem cannot confirm it, Sniff reports an undetermined check and refuses the claim. Use the recorded spelling from `sniff list` to address an existing reservation.

## Build from source

With Rust and Git installed, clone and build the executable:

```sh
git clone https://github.com/Swivelgames/Sniff.git
cd Sniff
cargo install --locked --path .
```

## Shell completion

Run the registration command for your shell in the current session. Add the same command to your shell profile to enable completion in future sessions.

| Shell | Command |
| --- | --- |
| Bash | `source <(sniff completion bash)` |
| Zsh (after `compinit`) | `source <(sniff completion zsh)` |
| Fish | `sniff completion fish \| source` |
| PowerShell | `sniff completion powershell \| Out-String \| Invoke-Expression` |

Type `sniff unclaim ` and press Tab to see active claim IDs. Use `--workspace` to select another workspace. Path completion also works with shorthand checks and `claim --reserve`; you can type a future path directly.

## License

You may use Sniff under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE).
