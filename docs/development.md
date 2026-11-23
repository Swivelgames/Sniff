# Filesystem behavior and native validation

Swiss queries the Windows directory case flag, the macOS volume case setting, and the case flags of Linux ext4 and tmpfs directories. On Linux ZFS it reads the dataset’s reported case mode. Byte-sensitive directories with a known exact-name policy can compare arbitrary native names. Where the filesystem can normalize or fold names, Swiss compares ASCII spellings and uses actual directory lookup to resolve other spellings that exist. If the relevant spellings are both missing and their equivalence cannot be established, the check returns `undetermined` and acquisition reports `path_unresolved`. Use the spelling from `swiss list` when addressing an existing reservation. Target directories need no writable probe files.

For native validation, run `cargo test --locked --test portability -- --nocapture`. The suite prints the file lookup it used as an oracle. Set `SWISS_PORTABILITY_DIR` to an existing directory to exercise a particular target volume or casefold-enabled directory. Run against both naming modes where available. The native acceptance suite requires working filename rules on the selected volume.

Linux overlayfs and other unrecognized filesystems can return `undetermined` even for two differently named missing entries. Swiss can still recognize identical spellings and inspect or release reservations. To verify conservative refusal on a deliberately unsupported target, set `SWISS_PORTABILITY_DIR` to that directory and run `cargo test --locked --test portability unknown_target_refuses -- --ignored --nocapture`. Run the regular suite separately on supported storage, setting `TMPDIR` there as well if needed.

## Development

Build and test with the locked crates.io dependencies. Generate the source reference with `cargo doc --no-deps --document-private-items`.

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
```
