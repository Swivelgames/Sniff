use super::Fixture;
use serde_json::json;
use std::{
	ffi::{OsStr, OsString},
	fs,
	os::windows::ffi::{OsStrExt, OsStringExt},
	path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
	GetShortPathNameW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
};

fn wide(path: &Path) -> Vec<u16> {
	path.as_os_str().encode_wide().chain([0]).collect()
}

fn terminated_path(buffer: &[u16]) -> PathBuf {
	let len = buffer.iter().position(|unit| *unit == 0).unwrap();
	OsString::from_wide(&buffer[..len]).into()
}

// Represent a reservation recorded before the alternate spelling became a real
// entry. This also forces comparison with the persisted spelling after reopening.
fn record_spelling(fixture: &Fixture, spelling: &Path) -> String {
	let id = uuid::Uuid::new_v4().to_string();
	let record = json!({"version":1,"claims":[{"claim_id":id,"paths":[spelling]}]});
	fs::write(
		fixture.store.path().join(".sniff/claims.json"),
		record.to_string(),
	)
	.unwrap();
	id
}

#[test]
fn materialized_short_name_and_long_name_identify_the_same_claim() {
	let fixture = Fixture::new();
	let target = fixture.target.path().canonicalize().unwrap();
	let name = "A long test directory name for Sniff";
	let full = target.join(name);
	fs::create_dir(&full).unwrap();

	let path = wide(&full);
	let mut short = vec![0u16; 32768];
	// SAFETY: path is terminated and short is a writable UTF-16 path buffer.
	let len = unsafe { GetShortPathNameW(path.as_ptr(), short.as_mut_ptr(), short.len() as u32) };
	assert!(
		len > 0 && (len as usize) < short.len(),
		"{}",
		std::io::Error::last_os_error()
	);
	let short_path = terminated_path(&short);
	let alias = short_path.file_name().unwrap();
	if alias == OsStr::new(name) {
		eprintln!(
			"GetShortPathNameW returned the long name; this target has no materialized 8.3 alias"
		);
		return;
	}
	let stored = target.join(alias);
	assert!(stored.is_dir());
	let id = record_spelling(&fixture, &stored);
	let child = format!("{name}/child");
	let check = fixture.call(&["check", &child], 0);
	assert_eq!(check["overlaps"][0]["claim_id"], id);
	assert_eq!(check["overlaps"][0]["relation"], "covers");
	let refusal = fixture.call(&["claim", "--reserve", &child], 1);
	assert_eq!(refusal["conflicts"][0]["claim_id"], id);
}

#[test]
fn volume_guid_and_drive_letter_paths_share_future_coverage() {
	let fixture = Fixture::new();
	let target = fixture.target.path();
	let path = wide(target);
	let mut mount = vec![0u16; 32768];
	// SAFETY: each call receives a terminated input path and a writable buffer.
	assert_ne!(
		unsafe { GetVolumePathNameW(path.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) },
		0,
		"local drive path required: {}",
		std::io::Error::last_os_error()
	);
	let mut guid = vec![0u16; 64];
	assert_ne!(
		unsafe {
			GetVolumeNameForVolumeMountPointW(mount.as_ptr(), guid.as_mut_ptr(), guid.len() as u32)
		},
		0,
		"local volume GUID required: {}",
		std::io::Error::last_os_error()
	);
	let drive_root = terminated_path(&mount).canonicalize().unwrap();
	let target = target.canonicalize().unwrap();
	let relative = target.strip_prefix(&drive_root).unwrap();
	let alternate = terminated_path(&guid).join(relative);
	assert!(alternate.is_dir());
	let id = record_spelling(&fixture, &alternate.join("Future"));
	assert_eq!(
		fixture.call(&["check", "Future/child"], 0)["overlaps"][0]["claim_id"],
		id
	);
	let refusal = fixture.call(&["claim", "--reserve", "Future/child"], 1);
	assert_eq!(refusal["conflicts"][0]["claim_id"], id);
}
