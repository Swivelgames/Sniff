//! Coverage and conflict rules for one store. Paths have already been resolved.
mod overlap;

use crate::paths::{
	Target,
	names::{self, Coverage},
};
use crate::problem::{Code, Problem};
pub use overlap::{Overlap, Relation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use trine::NativePath;
use uuid::Uuid;

/// A reservation identified by its ID, with a minimal set of covered roots.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Claim {
	/// ID used to extend or release this reservation.
	pub claim_id: String,
	/// Optional display label supplied by the caller.
	pub owner: Option<String>,
	/// Resolved roots covered by this reservation, including their descendants.
	pub paths: Vec<NativePath>,
}

/// The set of active claims in one workspace snapshot.
#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Claims(Vec<Claim>);

impl Claims {
	/// Consume the snapshot to enumerate its claim records.
	pub fn into_records(self) -> Vec<Claim> {
		self.0
	}

	/// Remove every root held by the ID. Return false if the ID is already absent.
	pub fn release(&mut self, id: &str) -> bool {
		let Some(index) = self.position(id) else {
			return false;
		};
		self.0.remove(index);
		true
	}

	/// Check stored IDs, absolute paths and component-wise overlap before trusting a snapshot.
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

	/// Find claims covering or contained by the request, optionally excluding the claim being extended.
	pub fn overlaps(
		&self,
		requested: &Target,
		except: Option<&str>,
	) -> Result<Vec<Overlap>, Problem> {
		let mut overlaps = vec![];
		let mut uncertain = None;
		for claim in &self.0 {
			if Some(claim.claim_id.as_str()) == except {
				continue;
			}
			for path in &claim.paths {
				match Overlap::between(requested, claim, path) {
					Ok(Some(hit)) => overlaps.push(hit),
					Ok(None) => {}
					Err(problem) => uncertain = Some(problem),
				}
			}
		}
		// A definite reservation answers a check even if another location cannot
		// be classified. Only report "clear" when every comparison succeeded.
		if overlaps.is_empty()
			&& let Some(problem) = uncertain
		{
			return Err(problem);
		}
		Ok(overlaps)
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

		let mut conflicts = Vec::new();
		let mut uncertain = None;
		for path in &paths {
			match self.overlaps(path, id) {
				Ok(hits) => conflicts.extend(hits),
				Err(problem) => uncertain = Some(problem),
			}
		}
		if !conflicts.is_empty() {
			let mut problem = Problem::new(Code::Conflict, "Paths overlap an existing claim.");
			problem.conflicts = conflicts;
			return Err(problem);
		}

		if let Some(problem) = uncertain {
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
			changed |= claim.include(target.resolved)?;
		}
		Ok((claim.clone(), changed))
	}

	fn find(&self, id: &str) -> Result<usize, Problem> {
		self.position(id)
			.ok_or_else(|| Problem::new(Code::ClaimNotFound, format!("Claim {id} was not found.")))
	}

	fn position(&self, id: &str) -> Option<usize> {
		self.0.iter().position(|claim| claim.claim_id == id)
	}
}

impl Claim {
	/// Fold redundant descendants into their covering root and report whether coverage changed.
	fn include(&mut self, path: NativePath) -> Result<bool, Problem> {
		let mut superseded = Vec::new();
		let mut uncertain = None;
		for (index, root) in self.paths.iter().enumerate() {
			match names::compare(path.as_path(), root.as_path()) {
				Ok(Coverage::Exact | Coverage::Covers) => return Ok(false),
				Ok(Coverage::Descendant) => superseded.push(index),
				Ok(Coverage::Disjoint) => {}
				Err(problem) => uncertain = Some(problem),
			}
		}
		if let Some(problem) = uncertain {
			return Err(problem);
		}

		for index in superseded.into_iter().rev() {
			self.paths.remove(index);
		}
		self.paths.push(path);
		Ok(true)
	}
}
