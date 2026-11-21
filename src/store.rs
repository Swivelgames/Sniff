mod claim;
mod document;
mod mutation;

pub use claim::ClaimRequest;

use crate::{
	claims::{Claims, Overlap},
	paths::Target,
	problem::{Code, Problem},
};
use document::Document;
use std::{
	fs::{self, File, OpenOptions},
	io,
	path::{Path, PathBuf},
};
use trine::NativePath;

const MARKER: &str = ".sniff";

pub struct Store(PathBuf);

impl Store {
	pub fn initialize(directory: &Path) -> Result<Self, Problem> {
		if directory.as_os_str().is_empty() {
			return Err(Problem::new(
				Code::InvalidInput,
				"Supply a non-empty workspace directory.",
			));
		}
		fs::create_dir_all(directory)
			.map_err(|e| Problem::at(Code::StoreUnavailable, directory, e))?;
		let root = directory
			.canonicalize()
			.map_err(|e| Problem::at(Code::StoreUnavailable, directory, e))?;
		let store = Self(root.join(MARKER));
		if store.present()? {
			store.snapshot()?;
			return Ok(store);
		}

		// Publish a complete marker. A concurrent initializer can only observe the
		// old absence or the fully populated winner, never half-initialized state.
		let staging = tempfile::Builder::new()
			.prefix(".sniff-init-")
			.tempdir_in(&root)
			.map_err(|e| Problem::at(Code::StoreUnavailable, &root, e))?;
		let lock = staging.path().join("store.lock");
		File::create(&lock).map_err(|e| Problem::at(Code::StoreUnavailable, &lock, e))?;
		Document::empty().publish(&staging.path().join("claims.json"), None)?;
		if let Err(error) = fs::rename(staging.path(), &store.0) {
			if !store.present()? {
				return Err(Problem::at(Code::StoreUnavailable, &store.0, error));
			}
			store.snapshot()?;
		}
		Ok(store)
	}

	pub fn discover(workspace: Option<&NativePath>) -> Result<Self, Problem> {
		let cwd;
		let root = match workspace {
			Some(path) => path.as_path(),
			None => {
				cwd = std::env::current_dir()
					.map_err(|e| Problem::new(Code::StoreUnavailable, e.to_string()))?;
				&cwd
			}
		};
		if root.as_os_str().is_empty() {
			return Err(Problem::new(
				Code::InvalidInput,
				"Supply a non-empty workspace directory.",
			));
		}
		for directory in root.ancestors() {
			let store = Self(directory.join(MARKER));
			if store.present()? {
				return Ok(store);
			}
			if workspace.is_some() {
				break;
			}
		}
		Err(Problem::at(
			Code::WorkspaceNotFound,
			root,
			"no Sniff workspace found; run sniff init",
		))
	}

	fn present(&self) -> Result<bool, Problem> {
		match fs::symlink_metadata(&self.0) {
			Ok(metadata) if metadata.is_dir() => Ok(true),
			Ok(_) => Err(Problem::at(
				Code::InvalidStore,
				&self.0,
				"expected a .sniff directory",
			)),
			Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
			Err(e) => Err(Problem::at(Code::StoreUnavailable, &self.0, e)),
		}
	}

	pub fn root(&self) -> &Path {
		self.0.parent().expect("marker under workspace")
	}

	fn lock_file(&self, writable: bool) -> Result<File, Problem> {
		let path = self.0.join("store.lock");
		require_file(&path)?;
		OpenOptions::new()
			.read(true)
			.write(writable)
			.open(&path)
			.map_err(|e| Problem::at(Code::StoreUnavailable, &path, e))
	}

	pub fn snapshot(&self) -> Result<Claims, Problem> {
		self.lock_file(false)?;
		Ok(Document::read(&self.0.join("claims.json"))?.claims)
	}

	pub fn overlaps(&self, requested: &Target) -> Result<Vec<Overlap>, Problem> {
		self.snapshot()?.overlaps(requested, None)
	}
}

fn require_file(path: &Path) -> Result<(), Problem> {
	let metadata =
		fs::symlink_metadata(path).map_err(|e| Problem::at(Code::StoreUnavailable, path, e))?;
	if metadata.is_file() {
		return Ok(());
	}
	Err(Problem::at(
		Code::InvalidStore,
		path,
		"expected a regular state file",
	))
}
