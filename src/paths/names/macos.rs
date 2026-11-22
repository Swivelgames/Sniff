//! Read the volume case-sensitivity policy for a macOS directory.

use super::Rules;
use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};

pub(super) fn rules(directory: &Path) -> io::Result<Rules> {
	let path = CString::new(directory.as_os_str().as_bytes())?;
	// SAFETY: path is a live, NUL-terminated filesystem path.
	let sensitive = unsafe { libc::pathconf(path.as_ptr(), libc::_PC_CASE_SENSITIVE) };
	Ok(match sensitive {
		0 => Rules::AsciiInsensitive,
		1 => Rules::AsciiSensitive,
		_ => Rules::Unknown,
	})
}
