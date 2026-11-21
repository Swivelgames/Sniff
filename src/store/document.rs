//! Publish the whole claim set with one same-filesystem replacement.
use crate::{
	claims::Claims,
	problem::{Code, Problem},
};
use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Deserialize, Serialize};
use std::{
	fs,
	io::{self, Write},
	path::Path,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
	version: u32,
	pub claims: Claims,
}

impl Document {
	pub fn empty() -> Self {
		Self {
			version: 1,
			claims: Claims::default(),
		}
	}

	pub fn read(path: &Path) -> Result<Self, Problem> {
		super::require_file(path)?;
		let bytes = fs::read(path).map_err(|e| Problem::at(Code::StoreUnavailable, path, e))?;
		let doc: Self =
			serde_json::from_slice(&bytes).map_err(|e| Problem::at(Code::InvalidStore, path, e))?;
		if doc.version != 1 {
			return Err(Problem::at(
				Code::InvalidStore,
				path,
				"unsupported claim store version",
			));
		}
		doc.claims
			.validate()
			.map_err(|e| Problem::at(Code::InvalidStore, path, e))?;
		Ok(doc)
	}

	pub fn publish(&self, path: &Path, claim_id: Option<&str>) -> Result<(), Problem> {
		let mut bytes = Vec::new();
		let format = serde_json::ser::PrettyFormatter::with_indent(b"\t");
		let mut json = serde_json::Serializer::with_formatter(&mut bytes, format);
		self.serialize(&mut json)
			.map_err(|e| Problem::at(Code::StoreUnavailable, path, e))?;
		bytes.push(b'\n');

		let written = AtomicFile::new(path, AllowOverwrite).write(|file| file.write_all(&bytes));
		written.map_err(|cause| publication_error(path, &bytes, claim_id, cause))
	}
}

// Atomic replacement can succeed before the containing directory reports a sync
// failure. The mutation lock is still held, so the visible bytes identify our write.
fn publication_error(
	path: &Path,
	intended: &[u8],
	claim_id: Option<&str>,
	cause: impl std::fmt::Display,
) -> Problem {
	let Some(id) = claim_id else {
		return Problem::at(Code::StoreUnavailable, path, cause);
	};

	let detail = match fs::read(path) {
		Ok(visible) if visible == intended => "is visible, but durability could not be confirmed",
		Ok(_) => return Problem::at(Code::StoreUnavailable, path, cause),
		Err(error) if error.kind() == io::ErrorKind::NotFound => {
			return Problem::at(Code::StoreUnavailable, path, cause);
		}
		Err(_) => "could not be verified after the write",
	};
	let mut problem = Problem::at(
		Code::PublicationUncertain,
		path,
		format!("claim {id} {detail}: {cause}"),
	);
	problem.claim_id = Some(id.to_owned());
	problem
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn publication_failure_reports_the_claim_id_when_the_replacement_is_visible() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("claims.json");
		fs::write(&path, "previous").unwrap();
		let before = publication_error(&path, b"updated", Some("id"), "write failed");
		assert!(matches!(before.code, Code::StoreUnavailable));
		assert!(before.claim_id.is_none());

		fs::write(&path, "updated").unwrap();
		let after = publication_error(&path, b"updated", Some("id"), "directory sync failed");
		assert!(matches!(after.code, Code::PublicationUncertain));
		assert_eq!(after.claim_id.as_deref(), Some("id"));
		assert!(after.message.contains("is visible"));
	}
}
