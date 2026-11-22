//! Resolve each requested path after writer admission, then acquire or release coverage.

use super::{Store, document::Change};
use crate::{
	claims::{Claim, Claims},
	paths::{self, Requirement, Target},
	problem::Problem,
};
use trine::{Cancellation, NativePath};

/// Requested coverage, with separate resolution policies for ordinary and reserved paths.
pub struct ClaimRequest {
	pub existing: Vec<NativePath>,
	pub reservations: Vec<NativePath>,
	pub id: Option<String>,
	pub owner: Option<String>,
}

impl ClaimRequest {
	fn resolve_paths(&mut self) -> Result<Vec<Target>, Problem> {
		let mut paths = Vec::new();
		for path in self.existing.drain(..) {
			paths.push(paths::resolve(path, Requirement::Existing)?);
		}
		for path in self.reservations.drain(..) {
			paths.push(paths::resolve(path, Requirement::MayBeMissing)?);
		}
		Ok(paths)
	}

	fn acquire(mut self, claims: &mut Claims) -> Result<(Claim, Option<Change>), Problem> {
		let paths = self.resolve_paths()?;
		let (claim, changed) = claims.acquire(paths, self.id.as_deref(), self.owner)?;
		let change = changed.then(|| Change::Claim(claim.claim_id.clone()));
		Ok((claim, change))
	}
}

impl Store {
	/// Resolve and acquire all requested paths together while holding the store lock.
	pub fn claim(
		&self,
		request: ClaimRequest,
		cancellation: &Cancellation,
	) -> Result<Claim, Problem> {
		self.mutate(cancellation, |claims| request.acquire(claims))
	}

	/// Release all paths of one ID and report whether the store changed.
	pub fn unclaim(&self, id: &str, cancellation: &Cancellation) -> Result<bool, Problem> {
		self.mutate(cancellation, |claims| {
			let released = claims.release(id);
			let change = released.then(|| Change::Release(id.to_owned()));
			Ok((released, change))
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn waiting_claim_rechecks_its_existing_paths_after_lock_admission() {
		let fixture = tempfile::tempdir().unwrap();
		let store = Store::initialize(fixture.path()).unwrap();
		let gate = store.lock_file(true).unwrap();
		gate.lock().unwrap();
		let path = fixture.path().join("vanishing");
		std::fs::write(&path, "temporary").unwrap();
		let request = ClaimRequest {
			existing: vec![paths::native(path.clone())],
			reservations: vec![paths::native(fixture.path().join("extra"))],
			id: None,
			owner: None,
		};

		let mut waited = false;
		let problem = store
			.mutate_with_wait(
				&Cancellation::new(),
				|claims| request.acquire(claims),
				|| {
					assert!(!waited);
					waited = true;
					std::fs::remove_file(&path).unwrap();
					gate.unlock().unwrap();
				},
			)
			.unwrap_err();

		assert!(waited);
		assert!(matches!(problem.code, crate::problem::Code::PathUnresolved));
		let extra = paths::resolve(
			paths::native(fixture.path().join("extra")),
			Requirement::MayBeMissing,
		)
		.unwrap();
		assert!(store.overlaps(&extra).unwrap().is_empty());
	}
}
