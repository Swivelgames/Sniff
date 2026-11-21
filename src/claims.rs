//! Coverage and conflict rules for one store. Paths have already been resolved.
mod overlap;

use crate::paths::Target;
use crate::problem::{Code, Problem};
pub use overlap::{Overlap, Relation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use trine::NativePath;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Claim {
	pub claim_id: String,
	pub owner: Option<String>,
	pub paths: Vec<NativePath>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Claims(Vec<Claim>);

impl Claims {
	pub fn validate(&self) -> Result<(), &'static str> {
		let mut ids = HashSet::new();
		let mut paths = Vec::new();
		for claim in &self.0 {
			if Uuid::parse_str(&claim.claim_id).is_err() || !ids.insert(&claim.claim_id) {
				return Err("invalid or duplicate claim ID");
			}
			if claim.paths.is_empty() {
				return Err("claim has no paths");
			}
			for location in &claim.paths {
				let path = location.as_path();
				if !path.is_absolute()
					|| path
						.components()
						.any(|c| c == std::path::Component::ParentDir)
				{
					return Err("claim contains an unresolved path");
				}
				paths.push(path);
			}
		}

		// Component ordering puts a parent immediately before its first descendant.
		paths.sort_unstable();
		if paths.windows(2).any(|pair| pair[1].starts_with(pair[0])) {
			return Err("claim store contains overlapping coverage");
		}
		Ok(())
	}

	pub fn overlaps(&self, requested: &Target, except: Option<&str>) -> Vec<Overlap> {
		let mut overlaps = vec![];
		for claim in &self.0 {
			if Some(claim.claim_id.as_str()) == except {
				continue;
			}
			for path in &claim.paths {
				if let Some(hit) = Overlap::between(requested, claim, path) {
					overlaps.push(hit);
				}
			}
		}
		overlaps
	}

	/// Return the complete reservation plus whether it requires publication.
	pub fn acquire(
		&mut self,
		paths: Vec<Target>,
		id: Option<&str>,
		owner: Option<String>,
	) -> Result<(Claim, bool), Problem> {
		if paths.is_empty() {
			return Err(Problem::new(
				Code::InvalidInput,
				"Supply a path or use --reserve.",
			));
		}

		let existing = id.map(|id| self.find(id)).transpose()?;

		let conflicts: Vec<_> = paths
			.iter()
			.flat_map(|path| self.overlaps(path, id))
			.collect();
		if !conflicts.is_empty() {
			let mut problem = Problem::new(Code::Conflict, "Paths overlap an existing claim.");
			problem.conflicts = conflicts;
			return Err(problem);
		}

		let claim = match existing {
			Some(index) => &mut self.0[index],
			None => {
				self.0.push(Claim {
					claim_id: Uuid::new_v4().to_string(),
					owner,
					paths: vec![],
				});
				self.0.last_mut().expect("just inserted claim")
			}
		};

		let mut changed = false;
		for target in paths {
			let path = target.resolved;
			if claim
				.paths
				.iter()
				.any(|root| path.as_path().starts_with(root))
			{
				continue;
			}
			claim
				.paths
				.retain(|root| !root.as_path().starts_with(&path));
			claim.paths.push(path);
			changed = true;
		}
		Ok((claim.clone(), changed))
	}

	fn find(&self, id: &str) -> Result<usize, Problem> {
		self.0
			.iter()
			.position(|claim| claim.claim_id == id)
			.ok_or_else(|| Problem::new(Code::ClaimNotFound, format!("Claim {id} was not found.")))
	}
}
