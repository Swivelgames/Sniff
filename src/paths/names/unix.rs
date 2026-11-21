use std::{ffi::OsStr, fs, io, os::unix::fs::MetadataExt, path::Path};

fn identity(path: &Path) -> io::Result<Option<(u64, u64)>> {
	match fs::symlink_metadata(path) {
		Ok(metadata) => Ok(Some((metadata.dev(), metadata.ino()))),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
		Err(error) => Err(error),
	}
}

// For unusual spellings use actual lookups when possible. Inode equality alone
// cannot distinguish an alias for one directory entry from separate hard links.
pub(super) fn lookup(parent: &Path, a: &OsStr, b: &OsStr) -> io::Result<Option<bool>> {
	let first_path = parent.join(a);
	let left = identity(&first_path)?;
	let right = identity(&parent.join(b))?;
	if left != right {
		// Creation or removal between these two lookups can mimic distinct entries.
		return Ok((identity(&first_path)? == left).then_some(false));
	}
	if left.is_none() {
		return Ok(None);
	}

	let mut entries = 0;
	let mut literal = 0;
	for entry in fs::read_dir(parent)? {
		let entry = entry?;
		if identity(&entry.path())? == left {
			entries += 1;
			if entry.file_name() == a || entry.file_name() == b {
				literal += 1;
			}
		}
	}
	match (entries, literal) {
		(1, _) => Ok(Some(true)),
		(_, 2) => Ok(Some(false)),
		_ => Ok(None),
	}
}
