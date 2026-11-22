//! Consult Windows directory policies and resolve materialized filename aliases.

mod namespace;
pub(super) use namespace::compare_roots;

use super::Rules;
use std::{
	ffi::{OsStr, OsString},
	fs::OpenOptions,
	io,
	os::windows::{
		ffi::{OsStrExt, OsStringExt},
		fs::OpenOptionsExt,
		io::AsRawHandle,
	},
	path::Path,
};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::{
	FILE_CASE_SENSITIVE_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
	FileCaseSensitiveInfo, FindClose, FindFirstFileW, GetFileInformationByHandleEx,
	WIN32_FIND_DATAW,
};

pub(super) fn rules(directory: &Path) -> io::Result<Rules> {
	let handle = OpenOptions::new()
		.access_mode(FILE_READ_ATTRIBUTES)
		.custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
		.open(directory)?;
	let mut info = FILE_CASE_SENSITIVE_INFO::default();
	// SAFETY: the live directory handle and the class-specific buffer cover this call.
	let ok = unsafe {
		GetFileInformationByHandleEx(
			handle.as_raw_handle(),
			FileCaseSensitiveInfo,
			(&raw mut info).cast(),
			size_of::<FILE_CASE_SENSITIVE_INFO>() as u32,
		)
	};
	Ok(match (ok, info.Flags & 1) {
		(0, _) => Rules::Unknown,
		(_, 0) => Rules::AsciiInsensitive,
		_ => Rules::ByteExact,
	})
}

pub(super) fn lookup(parent: &Path, a: &OsStr, b: &OsStr) -> io::Result<Option<bool>> {
	let left = entry_name(parent, a)?;
	let right = entry_name(parent, b)?;
	if left != right && entry_name(parent, a)? != left {
		return Err(io::Error::other(
			"directory entry changed during lookup; retry",
		));
	}
	Ok(match (left, right) {
		(None, None) => None,
		(left, right) => Some(left == right),
	})
}

fn entry_name(parent: &Path, name: &OsStr) -> io::Result<Option<OsString>> {
	if name
		.as_encoded_bytes()
		.iter()
		.any(|b| matches!(b, b'?' | b'*'))
	{
		return Err(io::Error::other("wildcards do not identify a single entry"));
	}
	let pattern: Vec<u16> = parent
		.join(name)
		.as_os_str()
		.encode_wide()
		.chain([0])
		.collect();
	let mut data = WIN32_FIND_DATAW::default();
	// SAFETY: pattern is NUL-terminated and data is a writable result buffer.
	let handle = unsafe { FindFirstFileW(pattern.as_ptr(), &raw mut data) };
	if handle == INVALID_HANDLE_VALUE {
		let error = io::Error::last_os_error();
		return if error.kind() == io::ErrorKind::NotFound {
			Ok(None)
		} else {
			Err(error)
		};
	}
	// SAFETY: FindFirstFileW returned an owned, valid search handle.
	unsafe { FindClose(handle) };
	let len = data
		.cFileName
		.iter()
		.position(|unit| *unit == 0)
		.unwrap_or(data.cFileName.len());
	Ok(Some(OsString::from_wide(&data.cFileName[..len])))
}
