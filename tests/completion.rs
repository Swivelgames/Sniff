mod support;

use std::{fs, path::Path};
use support::{cli, command};

fn candidates(root: &Path, words: &[&str], index: usize) -> Vec<String> {
	let output = command(root)
		.env("TRINE_COMPLETE", "zsh")
		.env_remove("TRINE_COMPLETE_METADATA")
		.env("_CLAP_COMPLETE_INDEX", index.to_string())
		.args(["--", "sniff"])
		.args(words)
		.output()
		.unwrap();
	assert!(output.status.success());
	assert!(output.stderr.is_empty());
	String::from_utf8(output.stdout)
		.unwrap()
		.lines()
		.map(str::to_owned)
		.collect()
}

#[test]
fn scripts_paths_and_commands_work_before_initialization() {
	let root = tempfile::tempdir().unwrap();
	let root = root.path();
	fs::create_dir(root.join("example dir")).unwrap();
	fs::write(root.join("example-file"), "").unwrap();

	for shell in ["bash", "zsh", "fish", "powershell"] {
		let output = command(root).args(["completion", shell]).output().unwrap();
		assert!(output.status.success());
		assert!(
			String::from_utf8(output.stdout)
				.unwrap()
				.contains("TRINE_COMPLETE")
		);
	}
	assert_eq!(candidates(root, &["unc"], 1), ["unclaim"]);
	assert!(candidates(root, &["example-file", "unc"], 2).is_empty());
	assert!(candidates(root, &["example-file", "--work"], 2).contains(&"--workspace".into()));
	for words in [
		&["exam"][..],
		&["check", "exam"],
		&["claim", "exam"],
		&["claim", "--reserve", "exam"],
	] {
		let paths = candidates(root, words, words.len());
		assert!(
			paths
				.iter()
				.any(|p| p.trim_end_matches(['/', '\\']) == "example dir")
		);
		assert!(paths.contains(&"example-file".into()));
	}
	for words in [
		&["init", "exam"][..],
		&["--workspace", "exam"],
		&["claim", "--workspace", "exam"],
	] {
		let paths = candidates(root, words, words.len());
		assert_eq!(paths.len(), 1);
		assert_eq!(paths[0].trim_end_matches(['/', '\\']), "example dir");
	}
	assert!(candidates(root, &["unclaim", "abc"], 2).is_empty());
	assert!(!root.join(".sniff").exists());
}

#[test]
fn claim_ids_follow_workspace_selection_and_disappear_after_release() {
	let root = tempfile::tempdir().unwrap();
	let root = root.path();
	let inner = root.join("nested");
	cli(root, &["init"], 0);
	cli(root, &["init", "nested"], 0);
	let outer = cli(root, &["claim", "--reserve", "file"], 0);
	let other = cli(&inner, &["claim", "--reserve", "file"], 0);
	let id = outer["claim_id"].as_str().unwrap();
	let prefix = &id[..8];
	let nested = other["claim_id"].as_str().unwrap();
	let descendant = inner.join("child");
	fs::create_dir(&descendant).unwrap();
	assert_eq!(candidates(&descendant, &["unclaim", nested], 2), [nested]);

	for (words, index) in [
		(vec!["--workspace", "..", "unclaim", prefix], 4),
		(vec!["--workspace=..", "claim", "--claim-id", prefix], 4),
		(vec!["unclaim", prefix, "--workspace", ".."], 2),
		(vec!["claim", "--workspace=..", "--claim-id", prefix], 4),
	] {
		assert_eq!(candidates(&inner, &words, index), [id]);
	}
	assert!(candidates(root, &["unclaim", prefix, "--workspace=absent"], 2).is_empty());
	cli(root, &["unclaim", id], 0);
	assert!(candidates(root, &["unclaim", prefix], 2).is_empty());
}
