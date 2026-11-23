mod support;

use std::fs;
use support::cli;

const INSTALLED_SKILL: &str = ".agents/skills/swiss/SKILL.md";
const BUNDLED_SKILL: &[u8] = include_bytes!("../skills/swiss/SKILL.md");

#[test]
fn init_installs_the_bundled_skill_and_preserves_local_edits() {
	let fixture = tempfile::tempdir().unwrap();
	let parent = fixture.path();
	let workspace = parent.join("workspace");
	let sibling = workspace.join(".agents/skills/workyard/SKILL.md");
	fs::create_dir_all(sibling.parent().unwrap()).unwrap();
	fs::write(&sibling, b"other product's skill").unwrap();

	cli(parent, &["init", "workspace"], 0);
	let installed = workspace.join(INSTALLED_SKILL);
	assert_eq!(fs::read(&installed).unwrap(), BUNDLED_SKILL);
	assert!(!parent.join(".agents").exists());
	assert!(!workspace.join(".agent").exists());

	fs::write(&installed, b"customized skill").unwrap();
	cli(&workspace, &["init"], 0);
	assert_eq!(fs::read(&installed).unwrap(), b"customized skill");
	assert_eq!(fs::read(&sibling).unwrap(), b"other product's skill");

	fs::remove_file(&installed).unwrap();
	cli(&workspace, &["init"], 0);
	assert_eq!(fs::read(&installed).unwrap(), BUNDLED_SKILL);
}

#[test]
fn blocked_skill_install_preserves_the_workspace_and_can_be_retried() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	let blocker = workspace.join(".agents");
	fs::write(&blocker, b"existing file").unwrap();

	let failure = cli(workspace, &["init"], 1);
	assert_eq!(failure["code"], "skill_install_failed");
	assert_eq!(fs::read(&blocker).unwrap(), b"existing file");
	cli(workspace, &["list"], 0);

	fs::remove_file(&blocker).unwrap();
	cli(workspace, &["init"], 0);
	assert_eq!(
		fs::read(workspace.join(INSTALLED_SKILL)).unwrap(),
		BUNDLED_SKILL
	);
}

#[cfg(unix)]
#[test]
fn init_never_installs_through_a_linked_skill_directory() {
	use std::os::unix::fs::symlink;

	let workspace = tempfile::tempdir().unwrap();
	let outside = tempfile::tempdir().unwrap();
	symlink(outside.path(), workspace.path().join(".agents")).unwrap();

	cli(workspace.path(), &["init"], 1);
	assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}
