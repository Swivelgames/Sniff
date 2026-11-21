//! Claims record resolved locations. A future path may have an ordinary missing tail.
pub mod names;

use crate::problem::{Code, Problem};
use std::{
	fs, io,
	path::{Component, PathBuf},
};
use trine::NativePath;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
	Existing,
	MayBeMissing,
}

pub struct Target {
	pub supplied: NativePath,
	pub resolved: NativePath,
}

pub fn resolve(input: NativePath, requirement: Requirement) -> Result<Target, Problem> {
	let path = input.as_path();
	if path.as_os_str().is_empty() {
		return Err(Problem::new(Code::InvalidInput, "Supply a non-empty path."));
	}

	let absolute = if path.is_absolute() {
		path.to_owned()
	} else {
		let cwd =
			std::env::current_dir().map_err(|e| Problem::at(Code::PathUnresolved, path, e))?;
		cwd.join(path)
	};
	if !absolute.is_absolute() {
		return Err(Problem::at(
			Code::PathUnresolved,
			path,
			"supply an absolute or working-directory-relative path",
		));
	}

	let mut resolved = PathBuf::new();
	for component in absolute.components() {
		match component {
			Component::Prefix(prefix) => {
				resolved.push(prefix.as_os_str());
				continue;
			}
			Component::CurDir => continue,
			Component::ParentDir => {
				// Let the OS check traversal (including permissions) through an
				// existing parent before collapsing ".." in the resolved location.
				let parent = resolved.join("..");
				match fs::metadata(&parent) {
					Ok(_) => {}
					Err(e)
						if requirement == Requirement::MayBeMissing
							&& e.kind() == io::ErrorKind::NotFound => {}
					Err(e) => return Err(Problem::at(Code::PathUnresolved, &parent, e)),
				}
				resolved.pop();
				continue;
			}
			other => resolved.push(other),
		}

		// lstat first so a dangling link cannot masquerade as an absent component.
		match fs::symlink_metadata(&resolved) {
			Ok(_) => {
				resolved = fs::canonicalize(&resolved)
					.map_err(|e| Problem::at(Code::PathUnresolved, &resolved, e))?
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				if requirement == Requirement::Existing {
					return Err(Problem::at(
						Code::PathUnresolved,
						&resolved,
						"path does not exist; check its spelling or use --reserve",
					));
				}
			}
			Err(e) => return Err(Problem::at(Code::PathUnresolved, &resolved, e)),
		}
	}
	// Path::components drops a final slash or ".". Ask the OS about the
	// original spelling too, so "file/" cannot acquire a regular file.
	match fs::metadata(&absolute) {
		Ok(_) => {}
		Err(e)
			if requirement == Requirement::MayBeMissing && e.kind() == io::ErrorKind::NotFound => {}
		Err(e) => return Err(Problem::at(Code::PathUnresolved, path, e)),
	}

	Ok(Target {
		supplied: input,
		resolved: native(resolved),
	})
}

pub fn native(path: PathBuf) -> NativePath {
	NativePath::new(path).expect("filesystem paths and validated input contain no NUL")
}
