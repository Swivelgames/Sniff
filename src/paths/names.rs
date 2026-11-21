//! Compare directory entries where two resolved locations first differ in spelling.
//! Stored locations stay intact, including when their original entries disappear.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use crate::problem::{Code, Problem};
use std::{
	ffi::OsStr,
	fs, io,
	path::{Component, Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Coverage {
	Exact,
	Covers,
	Descendant,
	Disjoint,
}

pub fn compare(request: &Path, recorded: &Path) -> Result<Coverage, Problem> {
	#[cfg(windows)]
	if request.components().next() != recorded.components().next() {
		return windows::compare_roots(request, recorded);
	}
	compare_components(request, recorded)
}

fn compare_components(request: &Path, recorded: &Path) -> Result<Coverage, Problem> {
	let mut left = request.components();
	let mut right = recorded.components();
	let mut parent = PathBuf::new();
	loop {
		match (left.next(), right.next()) {
			(None, None) => return Ok(Coverage::Exact),
			(Some(_), None) => return Ok(Coverage::Covers),
			(None, Some(_)) => return Ok(Coverage::Descendant),
			(Some(a), Some(b)) if a == b => parent.push(a),
			(Some(Component::Normal(a)), Some(Component::Normal(b))) => {
				if !same_name(&parent, a, b)? {
					return Ok(Coverage::Disjoint);
				}
				parent.push(a);
			}
			// Windows normalizes differing root prefixes before this comparison.
			_ => return Ok(Coverage::Disjoint),
		}
	}
}

#[derive(Clone, Copy)]
#[allow(dead_code)] // Native queries return the modes supported on their platform.
enum Rules {
	ByteExact,
	AsciiSensitive,
	AsciiInsensitive,
	Unknown,
}

impl Rules {
	fn compare(self, a: &OsStr, b: &OsStr) -> Option<bool> {
		if matches!(self, Self::ByteExact) {
			return Some(a == b);
		}
		let (a, b) = (a.as_encoded_bytes(), b.as_encoded_bytes());
		if !a.is_ascii() || !b.is_ascii() {
			return None;
		}
		match self {
			Self::AsciiSensitive => Some(a == b),
			Self::AsciiInsensitive => Some(a.eq_ignore_ascii_case(b)),
			_ => None,
		}
	}
}

fn same_name(parent: &Path, a: &OsStr, b: &OsStr) -> Result<bool, Problem> {
	match lookup(parent, a, b) {
		Ok(Some(equal)) => Ok(equal),
		outcome => {
			let detail = outcome
				.err()
				.map(|error| error.to_string())
				.unwrap_or_else(|| "the filesystem's rules for these spellings are unknown".into());
			Err(Problem::at(
				Code::PathUnresolved,
				parent,
				format!(
					"cannot compare {a:?} and {b:?}: {detail}. Verify directory access or use the reserved spelling"
				),
			))
		}
	}
}

fn lookup(parent: &Path, a: &OsStr, b: &OsStr) -> io::Result<Option<bool>> {
	let directory = existing_directory(parent)?;
	#[cfg(windows)]
	{
		// A materialized DOS short name can alias a different ASCII long name.
		// Resolve real entries before using the directory policy for future names.
		if let Some(equal) = windows::lookup(parent, a, b)? {
			return Ok(Some(equal));
		}
		Ok(platform_rules(directory)?.compare(a, b))
	}

	#[cfg(unix)]
	{
		let policy = platform_rules(directory);
		if let Ok(rules) = policy
			&& let Some(equal) = rules.compare(a, b)
		{
			return Ok(Some(equal));
		}
		// Looking up an existing name may require less access than querying its parent.
		match unix::lookup(parent, a, b)? {
			Some(equal) => Ok(Some(equal)),
			None => policy.map(|_| None),
		}
	}
}

fn existing_directory(path: &Path) -> io::Result<&Path> {
	for directory in path.ancestors() {
		match fs::symlink_metadata(directory) {
			Ok(metadata) if metadata.is_dir() => return Ok(directory),
			Ok(_) => return Err(io::Error::other("expected a resolved directory")),
			Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
			Err(error) => return Err(error),
		}
	}
	Err(io::Error::other("no accessible ancestor directory"))
}

#[cfg(target_os = "linux")]
use linux::rules as platform_rules;
#[cfg(target_os = "macos")]
use macos::rules as platform_rules;
#[cfg(windows)]
use windows::rules as platform_rules;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn platform_rules(_: &Path) -> io::Result<Rules> {
	Ok(Rules::Unknown)
}
