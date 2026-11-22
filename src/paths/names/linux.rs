//! Read Linux filesystem and directory policies for comparing missing names.

use super::Rules;
use linux_raw_sys::general::{EXT4_SUPER_MAGIC, FS_CASEFOLD_FL, TMPFS_MAGIC};
use rustix::fs::{fstatfs, ioctl_getflags};
use std::{
	fs::{self, File},
	io,
	os::fd::AsRawFd,
	path::Path,
};

const ZFS_SUPER_MAGIC: u32 = 0x2fc12fc1;

pub(super) fn rules(directory: &Path) -> io::Result<Rules> {
	let dir = File::open(directory)?;
	match fstatfs(&dir)?.f_type as u32 {
		EXT4_SUPER_MAGIC | TMPFS_MAGIC => {
			// ext4 and newer tmpfs expose directory casefolding through FS_IOC_GETFLAGS.
			match ioctl_getflags(&dir) {
				Ok(flags) if flags.bits() & FS_CASEFOLD_FL != 0 => Ok(Rules::AsciiInsensitive),
				Ok(_) => Ok(Rules::ByteExact),
				Err(_) => Ok(Rules::Unknown),
			}
		}
		ZFS_SUPER_MAGIC => zfs_rules(&dir),
		_ => Ok(Rules::Unknown),
	}
}

// OpenZFS publishes the dataset's case policy in mountinfo. Select by mount ID,
// which also works for bind mounts and paths containing escaped separators.
fn zfs_rules(dir: &File) -> io::Result<Rules> {
	let fdinfo = fs::read_to_string(format!("/proc/self/fdinfo/{}", dir.as_raw_fd()))?;
	let id = fdinfo
		.lines()
		.find_map(|line| line.strip_prefix("mnt_id:"))
		.map(str::trim);
	let Some(id) = id else {
		return Ok(Rules::Unknown);
	};
	let mounts = fs::read("/proc/self/mountinfo")?;
	for line in mounts.split(|byte| *byte == b'\n') {
		let mut fields = line.split(|byte| *byte == b' ');
		if fields.next() != Some(id.as_bytes()) {
			continue;
		}
		for field in fields
			.next_back()
			.unwrap_or_default()
			.split(|byte| *byte == b',')
		{
			match field {
				b"casesensitive" => return Ok(Rules::AsciiSensitive),
				b"caseinsensitive" => return Ok(Rules::AsciiInsensitive),
				_ => {}
			}
		}
	}
	Ok(Rules::Unknown)
}
