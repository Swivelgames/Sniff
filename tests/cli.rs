mod support;

use std::{fs, process::Stdio};
use support::{cli, command, domain};
use tempfile::tempdir;

#[test]
fn reserves_either_side_of_a_missing_tree_and_allows_siblings() {
	for (first, second, relation) in [
		("future", "future/payments", "covers"),
		("future/payments", "future", "descendant"),
	] {
		let fixture = tempdir().unwrap();
		let root = fixture.path();
		cli(root, &["init"], 0);
		let winner = cli(root, &["claim", "--owner", "same", "--reserve", first], 0);

		let loser = cli(
			root,
			&[
				"claim",
				"--owner",
				"same",
				"--reserve",
				"extra",
				"--reserve",
				second,
			],
			1,
		);
		assert_eq!(loser["code"], "conflict");
		let hit = &loser["conflicts"][0];
		assert_eq!(hit["claim_id"], winner["claim_id"]);
		assert_eq!(hit["relation"], relation);
		assert_eq!(hit["owner"], "same");
		assert_eq!(cli(root, &["check", "extra"], 0)["status"], "clear");
		assert_eq!(cli(root, &[second], 0)["overlaps"][0], *hit);
		cli(root, &["claim", "--reserve", "future-adjacent"], 0);
	}
}

#[test]
fn simultaneous_multi_path_requests_publish_exactly_one_complete_claim() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	// Queue independent processes at the same gate, then release them together.
	let gate = fs::File::options()
		.read(true)
		.write(true)
		.open(root.join(".swiss/store.lock"))
		.unwrap();
	gate.lock().unwrap();
	let requests = [("future", "first-extra"), ("future/child", "second-extra")];
	let children: Vec<_> = requests
		.iter()
		.map(|(shared, extra)| {
			command(root)
				.args(["claim", "--json", "--reserve", extra, "--reserve", shared])
				.stdout(Stdio::piped())
				.stderr(Stdio::piped())
				.spawn()
				.unwrap()
		})
		.collect();
	gate.unlock().unwrap();

	let results: Vec<_> = children
		.into_iter()
		.map(|child| child.wait_with_output().unwrap())
		.collect();
	assert_eq!(results.iter().filter(|o| o.status.success()).count(), 1);
	let winner_index = usize::from(!results[0].status.success());
	let loser_index = 1 - winner_index;
	let mut results = results
		.into_iter()
		.enumerate()
		.map(|(i, out)| domain(out, i32::from(i != winner_index)))
		.collect::<Vec<_>>();
	let winner_id = results[winner_index]["claim_id"].take();
	assert_eq!(results[loser_index]["conflicts"][0]["claim_id"], winner_id);
	assert_eq!(
		cli(root, &[requests[winner_index].1], 0)["overlaps"][0]["claim_id"],
		winner_id
	);
	assert_eq!(cli(root, &[requests[loser_index].1], 0)["status"], "clear");
}

#[test]
fn extensions_preserve_identity_and_reject_a_whole_invalid_batch() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	let record = root.join(".swiss/claims.json");
	cli(root, &["init"], 0);
	fs::write(root.join("existing"), "").unwrap();
	let original = cli(
		root,
		&["claim", "--reserve", "existing", "--owner", "agent"],
		0,
	);
	let id = original["claim_id"].as_str().unwrap();
	let conflict = cli(root, &["claim", "--reserve", "existing"], 1);
	assert_eq!(conflict["conflicts"][0]["relation"], "exact");

	let extended = cli(
		root,
		&[
			"claim",
			"--claim-id",
			id,
			"existing",
			"--reserve",
			"future/sub",
		],
		0,
	);
	assert_eq!(extended["claim_id"], id);
	assert_eq!(extended["paths"].as_array().unwrap().len(), 2);
	cli(root, &["claim", "--reserve", "theirs"], 0);

	let before = fs::read(&record).unwrap();
	for (options, expected) in [
		(
			vec!["--claim-id", id, "typo", "--reserve", "extra"],
			"path_unresolved",
		),
		(
			vec![
				"--claim-id",
				id,
				"--reserve",
				"extra",
				"--reserve",
				"theirs",
			],
			"conflict",
		),
		(
			vec!["--claim-id", "stale", "--reserve", "extra"],
			"claim_not_found",
		),
	] {
		let mut args = vec!["claim"];
		args.extend(options);
		assert_eq!(cli(root, &args, 1)["code"], expected);
		assert_eq!(fs::read(&record).unwrap(), before);
	}
	cli(
		root,
		&["claim", "--claim-id", id, "--reserve", "future/sub/file"],
		0,
	);
	cli(root, &["init"], 0);
	assert_eq!(fs::read(&record).unwrap(), before);
	assert_eq!(cli(root, &["extra"], 0)["status"], "clear");
}

#[test]
fn workspace_selection_is_exact_and_independent_from_target_paths() {
	let fixture = tempdir().unwrap();
	let parent = fixture.path();
	let child = parent.join("nested");
	fs::create_dir(&child).unwrap();
	let result = cli(&child, &["missing"], 1);
	assert_eq!(result["status"], "undetermined");
	assert_eq!(result["problem"]["code"], "workspace_not_found");
	assert!(!child.join(".swiss").exists());

	cli(parent, &["init"], 0);
	cli(&child, &["claim", "--reserve", "../external"], 0);
	assert_eq!(cli(parent, &["external"], 0)["status"], "claimed");
	assert_eq!(
		cli(&child, &["--workspace", ".", "../external"], 1)["status"],
		"undetermined"
	);

	cli(&child, &["init"], 0);
	assert_eq!(cli(&child, &["../external"], 0)["status"], "clear");
	assert_eq!(
		cli(&child, &["--workspace", "..", "check", "../external"], 0)["status"],
		"claimed"
	);
}

#[test]
fn corrupt_store_reports_uncertainty_and_is_preserved_on_reinit() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let state = root.join(".swiss/claims.json");
	for invalid in [
		"{",
		r#"{"version":2,"claims":[]}"#,
		r#"{"version":1,"claims":[{}]}"#,
	] {
		fs::write(&state, invalid).unwrap();
		let result = cli(root, &["absent"], 1);
		assert_eq!(result["status"], "undetermined");
		assert_eq!(result["problem"]["code"], "invalid_store");
		assert_eq!(cli(root, &["init"], 1)["code"], "invalid_store");
		assert_eq!(
			cli(root, &["claim", "--reserve", "absent"], 1)["code"],
			"invalid_store"
		);
		assert_eq!(fs::read_to_string(&state).unwrap(), invalid);
	}
}

#[cfg(unix)]
#[test]
fn resolved_alias_is_stable_and_unresolved_aliases_refuse_the_entire_request() {
	use std::os::unix::fs::symlink;
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	for dir in ["first", "second"] {
		fs::create_dir(root.join(dir)).unwrap();
	}
	let alias = root.join("alias");
	symlink("first", &alias).unwrap();
	let claim = cli(root, &["claim", "--reserve", "alias/future"], 0);
	let id = claim["claim_id"].as_str().unwrap();
	assert_eq!(
		cli(root, &["first/future"], 0)["overlaps"][0]["claim_id"],
		id
	);

	fs::remove_file(&alias).unwrap();
	symlink("second", &alias).unwrap();
	assert_eq!(cli(root, &["alias/future"], 0)["status"], "clear");
	assert_eq!(cli(root, &["first/future"], 0)["status"], "claimed");

	let record = root.join(".swiss/claims.json");
	let before = fs::read(&record).unwrap();
	for (name, dest) in [("dangling", "missing-target"), ("loop", "loop")] {
		symlink(dest, root.join(name)).unwrap();
		let unresolved = format!("{name}/file");
		let problem = cli(
			root,
			&[
				"claim",
				"--claim-id",
				id,
				"--reserve",
				"extra",
				"--reserve",
				&unresolved,
			],
			1,
		);
		assert_eq!(problem["code"], "path_unresolved");
		assert!(problem["message"].as_str().unwrap().contains(name));
		assert_eq!(fs::read(&record).unwrap(), before);
		assert_eq!(cli(root, &[&unresolved], 1)["status"], "undetermined");
	}
}

#[cfg(unix)]
#[test]
fn unreadable_store_and_inaccessible_path_report_uncertainty() {
	use std::os::unix::fs::PermissionsExt;

	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let dir = root.join("restricted");
	fs::create_dir(&dir).unwrap();
	fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
	// A privileged account can still traverse mode 000, so it cannot exercise this case.
	if fs::read_dir(&dir).is_err() {
		let outcome = cli(root, &["check", "restricted/missing"], 1);
		assert_eq!(outcome["status"], "undetermined");
		assert_eq!(outcome["problem"]["code"], "path_unresolved");
		assert_eq!(
			cli(root, &["claim", "--reserve", "restricted/missing"], 1)["code"],
			"path_unresolved"
		);
	}
	fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();

	let record = root.join(".swiss/claims.json");
	fs::set_permissions(&record, fs::Permissions::from_mode(0o000)).unwrap();
	if fs::File::open(&record).is_err() {
		let outcome = cli(root, &["missing"], 1);
		assert_eq!(outcome["status"], "undetermined");
		assert_eq!(outcome["problem"]["code"], "store_unavailable");
		assert_eq!(cli(root, &["list"], 1)["code"], "store_unavailable");
		assert_eq!(
			cli(root, &["unclaim", "absent"], 1)["code"],
			"store_unavailable"
		);
	}
	fs::set_permissions(&record, fs::Permissions::from_mode(0o600)).unwrap();
}

#[cfg(unix)]
#[test]
fn non_unicode_paths_survive_acquisition_and_reopening_the_store() {
	use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let raw = OsStr::from_bytes(b"future-\xff");
	let claim = cli(
		root,
		&[OsStr::new("claim"), OsStr::new("--reserve"), raw],
		0,
	);
	let path = &claim["paths"][0];
	assert_eq!(path["platform"], "unix");
	assert_eq!(cli(root, &["list"], 0)["claims"][0], claim);
	let checked = cli(root, &[raw], 0);
	assert_eq!(checked["path"], *path);
	assert_eq!(checked["overlaps"][0]["claim_id"], claim["claim_id"]);
}

#[cfg(unix)]
#[test]
fn failed_publication_preserves_existing_coverage() {
	use std::os::unix::fs::PermissionsExt;

	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let original = cli(root, &["claim", "--reserve", "mine"], 0);
	let id = original["claim_id"].as_str().unwrap();
	let store = root.join(".swiss");
	let record = store.join("claims.json");
	let before = fs::read(&record).unwrap();

	fs::set_permissions(&store, fs::Permissions::from_mode(0o500)).unwrap();
	if fs::File::create(store.join("permission-probe")).is_err() {
		// A no-op release also works when replacing the document is impossible.
		assert_eq!(cli(root, &["unclaim", "absent"], 0)["released"], false);
		let problem = cli(root, &["claim", "--claim-id", id, "--reserve", "extra"], 1);
		assert_eq!(problem["code"], "store_unavailable");
		assert_eq!(cli(root, &["unclaim", id], 1)["code"], "store_unavailable");
		assert_eq!(fs::read(record).unwrap(), before);
		assert_eq!(cli(root, &["mine"], 0)["overlaps"][0]["claim_id"], id);
		assert_eq!(cli(root, &["extra"], 0)["status"], "clear");
	}
	fs::set_permissions(&store, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn widening_a_claim_replaces_children_and_keeps_siblings_independent() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let claim = cli(root, &["claim", "--reserve", "future/payments/service"], 0);
	let id = claim["claim_id"].as_str().unwrap();
	let sibling = cli(root, &["claim", "--reserve", "future/payments-api"], 0);

	let widened = cli(
		root,
		&["claim", "--claim-id", id, "--reserve", "future/payments"],
		0,
	);
	let parent = root.canonicalize().unwrap().join("future/payments");
	assert_eq!(widened["paths"], serde_json::json!([parent]));
	assert_eq!(
		cli(root, &["future/payments/other"], 0)["overlaps"][0]["claim_id"],
		id
	);
	assert_eq!(
		cli(root, &["future/payments-api"], 0)["overlaps"][0]["claim_id"],
		sibling["claim_id"]
	);

	let conflict = cli(root, &["claim", "--reserve", "future/payments/new.rs"], 1);
	assert_eq!(
		conflict["conflicts"][0]["requested_path"],
		"future/payments/new.rs"
	);
	assert_eq!(
		conflict["conflicts"][0]["claimed_path"],
		serde_json::json!(parent)
	);
}

#[test]
fn valid_json_with_impossible_coverage_never_reports_clear() {
	use serde_json::json;

	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let path = root.join(".swiss/claims.json");
	let id_a = uuid::Uuid::new_v4().to_string();
	let id_b = uuid::Uuid::new_v4().to_string();
	let record = |id: &str, path| json!({ "claim_id":id, "paths":[path], "owner":null });

	for bad_claims in [
		vec![
			record(&id_a, root.join("one")),
			record(&id_a, root.join("two")),
		],
		vec![
			record(&id_a, root.join("tree")),
			record(&id_b, root.join("tree/nested")),
		],
		vec![record(&id_a, std::path::PathBuf::from("relative"))],
	] {
		let corrupt = json!({"version":1, "claims":bad_claims}).to_string();
		fs::write(&path, &corrupt).unwrap();
		let observation = cli(root, &["unrelated"], 1);
		assert_eq!(observation["status"], "undetermined");
		assert_eq!(observation["problem"]["code"], "invalid_store");
		assert_eq!(
			cli(root, &["claim", "--reserve", "unrelated"], 1)["code"],
			"invalid_store"
		);
		assert_eq!(fs::read_to_string(&path).unwrap(), corrupt);
	}
}

#[test]
fn human_mode_sends_facts_to_stdout_and_refusals_to_stderr() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	let human = |args: &[&str], expected_code| {
		let output = command(root).args(args).output().unwrap();
		assert_eq!(output.status.code(), Some(expected_code));
		let stdout = String::from_utf8(output.stdout).unwrap();
		let stderr = String::from_utf8(output.stderr).unwrap();
		if expected_code == 0 {
			assert!(stderr.is_empty(), "{stderr}");
			return stdout;
		}
		assert!(stdout.is_empty(), "{stdout}");
		stderr
	};
	assert_eq!(
		human(&["thing"], 1),
		"  Undetermined\n    No Swiss workspace found.\n    .\n\n  From the intended workspace directory, run\n    swiss init\n"
	);
	assert!(human(&["init"], 0).starts_with("  Workspace ready · .\n\n"));
	assert_eq!(human(&["thing"], 0), "  Clear · thing\n");

	let missing = human(&["claim", "new-parent/thing"], 1);
	assert!(missing.contains("    new-parent/thing\n\n"));
	assert!(missing.contains("    swiss claim --reserve <path>\n"));

	let created = human(&["claim", "--reserve", "thing", "--owner", "writer"], 0);
	let checked = cli(root, &["thing"], 0);
	let claim_id = checked["overlaps"][0]["claim_id"].as_str().unwrap();
	assert!(created.contains(claim_id));
	let status = human(&["thing"], 0);
	assert!(status.contains("Claimed") && status.contains(claim_id) && status.contains("writer"));
	let error = human(&["claim", "--reserve", "thing"], 1);
	assert!(error.contains("overlap") && error.contains(claim_id) && error.contains("writer"));
}
