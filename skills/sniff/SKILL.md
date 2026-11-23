---
name: sniff
description: Use Sniff to check, reserve, extend, and release file or directory claims before editing in a shared workspace.
---

# Coordinate edits with Sniff

Use Sniff when several people or agents might edit the same filesystem tree. A
claim covers its path and every descendant. Cooperating callers need to use the
same Sniff workspace. Confirm the selected workspace when working from a different
directory, worktree, or stdio host.

## Before editing

- Run `sniff check <path>` to see whether a path overlaps a claim. A claimed result
  is a successful check, so read the result even when the command exits zero.
  An undetermined result means you need to resolve the reported problem.
- Use `sniff claim <path>...` to reserve the paths you need. A check is a snapshot.
  The claim operation checks and acquires the whole request together.
- List existing paths as positional arguments. For a path you expect to create,
  use `--reserve <path>`. Reserve the narrowest scope that covers your work.
  A reservation still conflicts with another claim on a parent or descendant,
  even when the requested paths do not exist yet.
- If a claim conflicts, read the overlapping path, claim ID, and owner label.
  Coordinate with the holder or choose disjoint work. A matching owner label
  does not grant control of the reservation.

```sh
sniff check src/payments/refund.rs
sniff claim src/payments/refund.rs --reserve tests/refunds.rs --owner payments-agent
sniff list
```

Save the returned ID in `claim_id`. Use it to extend the same claim when scope
grows and to release that claim when your work is done.

```sh
sniff claim --claim-id "$claim_id" --reserve tests/refund-fixtures
sniff unclaim "$claim_id"
```

Release leaves the files in place.
Repeating release with the same ID is safe. Claims survive process exit. Release
only the ID you own or have been asked to release. If a mutation reply is lost
or publication is uncertain, inspect the same workspace with `sniff list` before
retrying.

## Choosing the workspace and interface

`sniff init [directory]` establishes a new coordination root when needed.
Commands use the nearest ancestor workspace or an exact `--workspace <directory>`.
Relative target paths still start at the process working directory. Use absolute
workspace and target paths when a host might have a different working directory.

Use `--json` for CLI automation. For an agent with MCP available, start
`sniff mcp` over stdio and use the discovered `check`, `claim`, `list`, and
`unclaim` tools. For local program integration, `sniff rpc` serves the same
actions. With either host, pass `workspace` explicitly when needed. Consult the
advertised input schemas or `sniff <command> --help` for input details.
