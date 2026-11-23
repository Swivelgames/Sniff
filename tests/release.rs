mod support;

use serde_json::json;
use std::{fs, path::Path, process::Output, sync::Barrier, thread};
use support::{cli, command, domain};
use tempfile::tempdir;

#[test]
fn release_preserves_files_and_a_stale_id_cannot_touch_the_next_owner() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	let file = root.join("file");
	fs::write(&file, "keep").unwrap();
	cli(root, &["init"], 0);
	let claim = cli(
		root,
		&["claim", "file", "--reserve", "future", "--owner", "team"],
		0,
	);
	let other = cli(root, &["claim", "--reserve", "other", "--owner", "team"], 0);
	let id = claim["claim_id"].as_str().unwrap();
	fs::rename(&file, root.join("moved")).unwrap();

	assert_eq!(cli(root, &["list"], 0), json!({"claims":[claim,other]}));
	fs::write(&file, "replacement").unwrap();
	assert_eq!(
		cli(root, &["unclaim", id], 0),
		json!({"claim_id":id,"released":true})
	);
	assert_eq!(fs::read(root.join("moved")).unwrap(), b"keep");
	assert_eq!(fs::read(&file).unwrap(), b"replacement");
	assert!(!root.join("future").exists());
	let fresh = cli(
		root,
		&["claim", "--reserve", "file", "--reserve", "future"],
		0,
	);
	assert_ne!(fresh["claim_id"], id);
	let document = root.join(".sniff/claims.json");
	let before = fs::read(&document).unwrap();

	assert_eq!(
		cli(root, &["unclaim", id], 0),
		json!({"claim_id":id,"released":false})
	);
	assert_eq!(
		cli(root, &["claim", "--claim-id", id, "--reserve", "extra"], 1)["code"],
		"claim_not_found"
	);
	assert_eq!(fs::read(document).unwrap(), before);
	assert_eq!(cli(root, &["list"], 0), json!({"claims":[other,fresh]}));
}

#[test]
fn workspace_selection_and_damaged_state_apply_to_both_actions() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	let nested = root.join("sub");
	fs::create_dir(&nested).unwrap();
	let actions: &[&[&str]] = &[&["list"], &["unclaim", "unknown"]];
	for args in actions {
		assert_eq!(cli(root, args, 1)["code"], "workspace_not_found");
	}

	cli(root, &["init"], 0);
	let claim = cli(root, &["claim", "--reserve", "future"], 0);
	let id = claim["claim_id"].as_str().unwrap();
	assert_eq!(cli(&nested, &["list"], 0), json!({"claims":[claim]}));
	cli(&nested, &["init"], 0);
	assert_eq!(
		cli(root, &["list", "--workspace", "sub"], 0),
		json!({"claims":[]})
	);
	assert_eq!(
		cli(root, &["unclaim", id, "--workspace", "sub"], 0)["released"],
		false
	);

	fs::write(nested.join(".sniff/claims.json"), "damaged").unwrap();
	for args in actions {
		assert_eq!(cli(&nested, args, 1)["code"], "invalid_store");
	}
	assert_eq!(cli(root, &["list"], 0), json!({"claims":[claim]}));
}

fn race(root: &Path, a: &[&str], b: &[&str]) -> (Output, Output) {
	let start = Barrier::new(2);
	let run = |args: &[&str]| {
		start.wait();
		command(root).arg("--json").args(args).output().unwrap()
	};
	thread::scope(|scope| {
		let first = scope.spawn(|| run(a));
		let second = run(b);
		(first.join().unwrap(), second)
	})
}

#[test]
fn concurrent_releases_and_extensions_serialize_by_id() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let other = cli(root, &["claim", "--reserve", "other"], 0);
	let reserve = || cli(root, &["claim", "--reserve", "race"], 0);

	let claim = reserve();
	let id = claim["claim_id"].as_str().unwrap();
	let (a, b) = race(root, &["unclaim", id], &["unclaim", id]);
	let outcomes = [domain(a, 0), domain(b, 0)];
	assert_eq!(outcomes.iter().filter(|r| r["released"] == true).count(), 1);
	assert_eq!(
		outcomes.iter().filter(|r| r["released"] == false).count(),
		1
	);

	let claim = reserve();
	let id = claim["claim_id"].as_str().unwrap();
	let (released, extended) = race(
		root,
		&["unclaim", id],
		&["claim", "--claim-id", id, "--reserve", "extra"],
	);
	assert_eq!(domain(released, 0)["released"], true);
	if extended.status.success() {
		assert_eq!(domain(extended, 0)["claim_id"], id);
	} else {
		assert_eq!(domain(extended, 1)["code"], "claim_not_found");
	}
	assert_eq!(cli(root, &["list"], 0), json!({"claims":[other]}));
}

#[test]
fn terminal_groups_each_claims_paths_and_distinguishes_release_from_retry() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let text = |args: &[&str]| {
		let out = command(root).args(args).output().unwrap();
		assert!(out.status.success());
		assert!(out.stderr.is_empty());
		String::from_utf8(out.stdout).unwrap()
	};
	assert_eq!(text(&["list"]), "  No active claims.\n");
	let first = cli(
		root,
		&[
			"claim",
			"--owner",
			"team",
			"--reserve",
			"first",
			"--reserve",
			"second",
		],
		0,
	);
	let next = cli(root, &["claim", "--reserve", "third"], 0);
	let id = first["claim_id"].as_str().unwrap();
	assert_eq!(
		text(&["list"]),
		format!(
			"  Active claims\n\n    {id} (team)\n      first\n      second\n\n    {}\n      third\n",
			next["claim_id"].as_str().unwrap(),
		)
	);
	assert_eq!(text(&["unclaim", id]), format!("  Released claim · {id}\n"));
	assert_eq!(
		text(&["unclaim", id]),
		format!("  Claim already absent · {id}\n")
	);
}

#[cfg(unix)]
#[test]
fn terminal_escapes_control_characters_and_json_retains_the_supplied_values() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let owner = "team\nother";
	let filename = "file\nother";
	let claim = cli(root, &["claim", "--owner", owner, "--reserve", filename], 0);

	let output = command(root).arg("list").output().unwrap();
	assert!(output.status.success());
	let displayed = String::from_utf8(output.stdout).unwrap();
	assert_eq!(displayed.lines().count(), 4);
	assert!(displayed.contains(r"(team\nother)"));
	assert!(displayed.contains(r"file\nother"));
	assert_eq!(cli(root, &["list"], 0)["claims"][0], claim);

	let checked = command(root).args(["check", filename]).output().unwrap();
	assert!(checked.status.success());
	let displayed = String::from_utf8(checked.stdout).unwrap();
	assert_eq!(displayed.lines().count(), 2);
	assert!(displayed.contains(r"(team\nother)"));
	assert!(displayed.contains(r"file\nother"));

	let rejected = command(root)
		.args(["claim", "--reserve", filename])
		.output()
		.unwrap();
	assert!(!rejected.status.success());
	let displayed = String::from_utf8(rejected.stderr).unwrap();
	assert_eq!(displayed.lines().count(), 2);
	assert!(displayed.contains(r"(team\nother)"));
	assert!(displayed.contains(r"file\nother"));

	let id = "absent\x1b[2J";
	let output = command(root).args(["unclaim", id]).output().unwrap();
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8(output.stdout).unwrap(),
		"  Claim already absent · absent\\u{1b}[2J\n"
	);
	assert_eq!(cli(root, &["unclaim", id], 0)["claim_id"], id);
}
