mod support;
#[cfg(windows)]
#[path = "portability/windows.rs"]
mod windows;

use serde_json::{Value, json};
use std::{
	fs,
	path::{Path, PathBuf},
	process::Output,
	sync::Barrier,
	thread,
};
use support::{cli, command, domain};
use tempfile::TempDir;

/// Always keep the store independent of the target. A native acceptance run can
/// place the latter on a specifically configured volume or directory.
struct Fixture {
	store: TempDir,
	target: TempDir,
}

impl Fixture {
	fn new() -> Self {
		let store = tempfile::tempdir().unwrap();
		let base = std::env::var_os("SWISS_PORTABILITY_DIR")
			.map(PathBuf::from)
			.unwrap_or_else(std::env::temp_dir);
		let target = tempfile::tempdir_in(base).unwrap();
		cli(store.path(), &["init"], 0);
		Self { store, target }
	}

	fn run(&self, args: &[&str]) -> Output {
		let mut command = command(self.target.path());
		command
			.arg("--json")
			.arg("--workspace")
			.arg(self.store.path())
			.args(args)
			.output()
			.unwrap()
	}

	fn call(&self, args: &[&str], exit: i32) -> Value {
		domain(self.run(args), exit)
	}

	fn reserve(&self, spelling: &str) -> Value {
		self.call(&["claim", "--reserve", spelling], 0)
	}

	fn insensitive(&self) -> bool {
		lookup_equivalent(self.target.path(), "Oracle", "oracle")
	}
}

// Determine expected behavior from lookup on this exact target, then remove
// the witness before Swiss evaluates a genuinely absent suffix.
fn lookup_equivalent(directory: &Path, original: &str, alternate: &str) -> bool {
	let witness = directory.join(original);
	fs::write(&witness, b"lookup oracle").unwrap();
	let equal = directory.join(alternate).try_exists().unwrap();
	fs::remove_file(witness).unwrap();
	eprintln!(
		"lookup in {}: {original:?} == {alternate:?} -> {equal}",
		directory.display()
	);
	equal
}

#[test]
fn absent_case_variants_follow_the_target_in_both_orders() {
	for (first, second, relation) in [
		("missing/Future", "missing/future/child", "covers"),
		("missing/future/child", "missing/Future", "descendant"),
	] {
		let fixture = Fixture::new();
		let insensitive = fixture.insensitive();
		let original = fixture.reserve(first);

		let read = fixture.call(&["check", second], 0);
		if insensitive {
			assert_eq!(read["overlaps"][0]["claim_id"], original["claim_id"]);
			assert_eq!(read["overlaps"][0]["relation"], relation);
			let refused = fixture.call(&["claim", "--reserve", second, "--reserve", "other"], 1);
			assert_eq!(refused["code"], "conflict");
			assert_eq!(fixture.call(&["check", "other"], 0)["status"], "clear");
		} else {
			assert_eq!(read["status"], "clear");
			let separate = fixture.reserve(second);
			assert_ne!(separate["claim_id"], original["claim_id"]);
			assert_eq!(
				fixture.call(&["list"], 0)["claims"],
				json!([original, separate])
			);
		}
	}
}

#[test]
fn materializing_a_variant_does_not_lose_or_duplicate_coverage() {
	let fixture = Fixture::new();
	let insensitive = fixture.insensitive();
	let original = fixture.reserve("Future");
	fs::create_dir(fixture.target.path().join("future")).unwrap();

	let child = fixture.call(&["check", "future/child"], 0);
	let expected = if insensitive { "claimed" } else { "clear" };
	assert_eq!(child["status"], expected);

	let id = original["claim_id"].as_str().unwrap();
	let extended = fixture.call(&["claim", "--claim-id", id, "future"], 0);
	if insensitive {
		assert_eq!(extended, original);
	} else {
		assert_eq!(extended["paths"].as_array().unwrap().len(), 2);
	}
}

#[test]
fn unicode_locations_follow_real_directory_entries() {
	for (first, second) in [
		("CAFÉ", "café"),
		("café", "cafe\u{301}"),
		("café", "mañana"),
	] {
		let fixture = Fixture::new();
		let equivalent = lookup_equivalent(fixture.target.path(), first, second);
		let original = fixture.reserve(first);
		fs::write(fixture.target.path().join(second), b"exists now").unwrap();
		let check = fixture.call(&["check", second], 0);
		if equivalent {
			assert_eq!(check["overlaps"][0]["claim_id"], original["claim_id"]);
		} else {
			assert_eq!(check["status"], "clear");
		}
	}
}

#[test]
fn concurrent_variant_batches_respect_native_parent_coverage() {
	let fixture = Fixture::new();
	let insensitive = fixture.insensitive();
	let gate = Barrier::new(2);
	let batch = |root, extra| {
		gate.wait();
		fixture.run(&["claim", "--reserve", root, "--reserve", extra])
	};
	let (a, b) = thread::scope(|scope| {
		let child = scope.spawn(|| batch("Future", "extra-a"));
		let b = batch("future/child", "extra-b");
		(child.join().unwrap(), b)
	});
	if !insensitive {
		assert!(a.status.success() && b.status.success());
		return;
	}
	assert_ne!(a.status.success(), b.status.success());
	let (winner, loser, extra) = if a.status.success() {
		(a, b, "extra-b")
	} else {
		(b, a, "extra-a")
	};
	let id = domain(winner, 0)["claim_id"].clone();
	assert_eq!(domain(loser, 1)["conflicts"][0]["claim_id"], id);
	assert_eq!(fixture.call(&["check", extra], 0)["status"], "clear");
}

#[test]
fn legacy_variant_claims_remain_visible_and_individually_releasable() {
	let fixture = Fixture::new();
	let insensitive = fixture.insensitive();
	let a = uuid::Uuid::new_v4().to_string();
	let b = uuid::Uuid::new_v4().to_string();
	let target = fixture.target.path().canonicalize().unwrap();
	let records = json!([
		{"claim_id":a,"paths":[target.join("Future")]},
		{"claim_id":b,"paths":[target.join("future")]}
	]);
	fs::write(
		fixture.store.path().join(".swiss/claims.json"),
		json!({"version":1,"claims":records}).to_string(),
	)
	.unwrap();

	assert_eq!(
		fixture.call(&["list"], 0)["claims"]
			.as_array()
			.unwrap()
			.len(),
		2
	);
	let matches = fixture.call(&["check", "future/child"], 0);
	assert_eq!(
		matches["overlaps"].as_array().unwrap().len(),
		if insensitive { 2 } else { 1 }
	);
	assert_eq!(fixture.call(&["unclaim", &a], 0)["released"], true);
	assert_eq!(
		fixture.call(&["check", "future/child"], 0)["overlaps"][0]["claim_id"],
		b
	);
}

#[cfg(unix)]
#[test]
fn external_symlinks_observe_the_target_and_preserve_the_resolved_location() {
	use std::os::unix::fs::symlink;
	let fixture = Fixture::new();
	let insensitive = fixture.insensitive();
	let alias = fixture.store.path().join("alias");
	symlink(fixture.target.path(), &alias).unwrap();
	let original = cli(
		fixture.store.path(),
		&["claim", "--reserve", "alias/Future"],
		0,
	);
	let check = fixture.call(&["check", "future/file"], 0);
	if insensitive {
		assert_eq!(check["overlaps"][0]["claim_id"], original["claim_id"]);
	} else {
		assert_eq!(check["status"], "clear");
	}
	fs::remove_file(&alias).unwrap();
	symlink(fixture.store.path(), &alias).unwrap();
	assert_eq!(
		cli(fixture.store.path(), &["check", "alias/Future"], 0)["status"],
		"clear"
	);
	assert_eq!(
		fixture.call(&["check", "Future"], 0)["overlaps"][0]["claim_id"],
		original["claim_id"]
	);
}

#[cfg(unix)]
#[test]
fn distinct_hard_links_and_non_text_names_keep_their_own_locations() {
	use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
	let fixture = Fixture::new();
	let dir = fixture.target.path();
	fs::write(dir.join("café"), b"hard links").unwrap();
	fs::hard_link(dir.join("café"), dir.join("mañana")).unwrap();
	fixture.call(&["claim", "café"], 0);
	assert_eq!(fixture.call(&["check", "mañana"], 0)["status"], "clear");
	let raw = OsStr::from_bytes(b"raw-\xff");
	// Some name-encoding policies reject non-text entries at the filesystem boundary.
	if let Err(error) = fs::write(dir.join(raw), b"native") {
		eprintln!("filesystem refused non-text fixture: {error}");
		return;
	}
	let result = cli(
		dir,
		&[
			OsStr::new("--workspace"),
			fixture.store.path().as_os_str(),
			OsStr::new("claim"),
			raw,
		],
		0,
	);
	assert_eq!(result["paths"][0]["platform"], "unix");
}

#[test]
fn unproven_unicode_comparisons_cannot_publish_a_partial_batch() {
	let fixture = Fixture::new();
	let equivalent = lookup_equivalent(fixture.target.path(), "Café", "CAFÉ");
	fixture.reserve("Café");
	let old_state = fs::read(fixture.store.path().join(".swiss/claims.json")).unwrap();

	let observed = fixture.run(&["check", "CAFÉ"]);
	if observed.status.success() {
		// A native backend may know the complete naming rule for this directory.
		let expected = if equivalent { "claimed" } else { "clear" };
		assert_eq!(domain(observed, 0)["status"], expected);
		return;
	}
	let unknown = domain(observed, 1);
	assert_eq!(unknown["status"], "undetermined");
	assert_eq!(unknown["problem"]["code"], "path_unresolved");
	assert!(
		unknown["problem"]["message"]
			.as_str()
			.unwrap()
			.contains("CAFÉ")
	);
	fs::write(fixture.target.path().join("separate"), "unclaimed").unwrap();
	let failed = fixture.call(&["claim", "separate", "--reserve", "CAFÉ"], 1);
	assert_eq!(failed["code"], "path_unresolved");
	assert_eq!(
		fs::read(fixture.store.path().join(".swiss/claims.json")).unwrap(),
		old_state
	);

	// A second pre-existing claim gives a definite answer even when the first
	// entry's spelling cannot be compared. Inspection reports the known owner.
	let document = fixture.store.path().join(".swiss/claims.json");
	let mut legacy: Value = serde_json::from_slice(&old_state).unwrap();
	let id = uuid::Uuid::new_v4().to_string();
	legacy["claims"]
		.as_array_mut()
		.unwrap()
		.push(json!({"claim_id":id,
		"paths":[fixture.target.path().canonicalize().unwrap().join("CAFÉ")]}));
	fs::write(document, legacy.to_string()).unwrap();
	let known = fixture.call(&["check", "CAFÉ"], 0);
	assert_eq!(known["overlaps"][0]["claim_id"], id);
}

#[test]
#[ignore = "use SWISS_PORTABILITY_DIR on a filesystem with unknown name rules and run --ignored"]
fn unknown_target_refuses_ambiguous_coverage_and_preserves_existing_claims() {
	assert!(
		std::env::var_os("SWISS_PORTABILITY_DIR").is_some(),
		"select an unsupported target directory explicitly"
	);
	let fixture = Fixture::new();
	let mine = fixture.reserve("Future");
	let record = fixture.store.path().join(".swiss/claims.json");
	let initial = fs::read(&record).unwrap();
	for candidate in ["future/child", "other"] {
		let check = fixture.call(&["check", candidate], 1);
		assert_eq!(check["status"], "undetermined");
		assert_eq!(check["problem"]["code"], "path_unresolved");
		assert_eq!(
			fixture.call(&["claim", "--reserve", candidate], 1)["code"],
			"path_unresolved"
		);
	}
	assert_eq!(fs::read(record).unwrap(), initial);
	assert_eq!(
		fixture.call(&["check", "Future/child"], 0)["overlaps"][0]["claim_id"],
		mine["claim_id"]
	);
}
