use super::super::{Coverage, compare_components};
use crate::problem::{Code, Problem};
use std::{
	ffi::{OsStr, OsString},
	fs::OpenOptions,
	io,
	os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
	path::{Component, Path, PathBuf, Prefix},
};
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

// GetFinalPathNameByHandle (also used by std's canonicalize) may retain distinct
// DOS prefixes for one volume. Translate only the root, so a later-created
// symlink in the recorded suffix cannot move an existing reservation.
pub(in crate::paths::names) fn compare_roots(
	request: &Path,
	recorded: &Path,
) -> Result<Coverage, Problem> {
	let a_root = unc_share(request);
	let b_root = unc_share(recorded);
	if let (Some((a_host, a_share)), Some((b_host, b_share))) = (a_root, b_root)
		&& a_host.eq_ignore_ascii_case(b_host)
		&& a_share.eq_ignore_ascii_case(b_share)
	{
		// Keep the request prefix for native lookup along the shared SMB namespace.
		let root = request.ancestors().last().expect("resolved absolute path");
		let other_root = recorded.ancestors().last().expect("resolved absolute path");
		let suffix = recorded.strip_prefix(other_root).expect("ancestor of path");
		return compare_components(request, &root.join(suffix));
	}

	let request = resolve_volume(request)?;
	let recorded = resolve_volume(recorded)?;
	compare_components(&request, &recorded)
}

fn unc_share(path: &Path) -> Option<(&OsStr, &OsStr)> {
	let Component::Prefix(prefix) = path.components().next()? else {
		return None;
	};
	match prefix.kind() {
		Prefix::UNC(host, share) | Prefix::VerbatimUNC(host, share) => Some((host, share)),
		_ => None,
	}
}

fn resolve_volume(path: &Path) -> Result<PathBuf, Problem> {
	volume_location(path).map_err(|error| {
		Problem::at(
			Code::PathUnresolved,
			path,
			format!(
				"cannot identify this volume: {error}. Verify access or use the reserved namespace"
			),
		)
	})
}

fn volume_location(path: &Path) -> io::Result<PathBuf> {
	use windows_sys::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, VOLUME_NAME_GUID};
	let root = path
		.ancestors()
		.last()
		.ok_or_else(|| io::Error::other("missing volume root"))?;
	let handle = OpenOptions::new()
		.access_mode(0)
		.custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
		.open(root)?;
	let mut buffer = vec![0u16; 260];
	loop {
		// SAFETY: the directory handle stays open and capacity matches the writable buffer.
		let count = unsafe {
			GetFinalPathNameByHandleW(
				handle.as_raw_handle(),
				buffer.as_mut_ptr(),
				buffer.len() as u32,
				VOLUME_NAME_GUID,
			)
		} as usize;
		if count == 0 {
			return Err(io::Error::last_os_error());
		}
		if count < buffer.len() {
			let volume = PathBuf::from(OsString::from_wide(&buffer[..count]));
			return Ok(volume.join(path.strip_prefix(root).expect("ancestor of path")));
		}
		buffer.resize(count + 1, 0);
	}
}
