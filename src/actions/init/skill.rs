//! Install the bundled workspace skill without replacing local content.

use atomicwrites::{AtomicFile, DisallowOverwrite};
use std::{
	fs::{self, Metadata},
	io::{self, Write},
	path::Path,
};

const SKILL: &[u8] = include_bytes!("../../../skills/sniff/SKILL.md");

pub(super) fn install(workspace: &Path) -> io::Result<()> {
	let mut directory = workspace.to_path_buf();
	for name in [".agents", "skills", "sniff"] {
		directory.push(name);
		ensure_directory(&directory)?;
	}

	let path = directory.join("SKILL.md");
	if exists_as(&path, Metadata::is_file)? {
		return Ok(());
	}

	let result = AtomicFile::new(&path, DisallowOverwrite).write(|file| file.write_all(SKILL));
	match result.map_err(io::Error::from) {
		// Another initializer may have published the skill since the lookup.
		Err(error)
			if error.kind() == io::ErrorKind::AlreadyExists
				&& exists_as(&path, Metadata::is_file)? =>
		{
			Ok(())
		}
		result => result,
	}
}

fn ensure_directory(path: &Path) -> io::Result<()> {
	if exists_as(path, Metadata::is_dir)? {
		return Ok(());
	}

	match fs::create_dir(path) {
		Err(error)
			if error.kind() == io::ErrorKind::AlreadyExists
				&& exists_as(path, Metadata::is_dir)? =>
		{
			Ok(())
		}
		result => result,
	}
}

fn exists_as(path: &Path, matches: fn(&Metadata) -> bool) -> io::Result<bool> {
	match fs::symlink_metadata(path) {
		Ok(metadata) if matches(&metadata) => Ok(true),
		Ok(_) => Err(io::Error::new(
			io::ErrorKind::AlreadyExists,
			format!("Unexpected entry at {}.", path.display()),
		)),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
		Err(error) => Err(error),
	}
}
