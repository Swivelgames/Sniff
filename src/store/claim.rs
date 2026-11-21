use super::{Store, document::Document};
use crate::{
	claims::Claim,
	paths::{self, Requirement, Target},
	problem::{Code, Problem},
};
use std::{fs::TryLockError, thread, time::Duration};
use trine::{Cancellation, NativePath};

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
}

impl Store {
	pub fn claim(
		&self,
		request: ClaimRequest,
		cancellation: &Cancellation,
	) -> Result<Claim, Problem> {
		self.claim_with_wait(request, cancellation, || {
			thread::sleep(Duration::from_millis(15))
		})
	}

	fn claim_with_wait(
		&self,
		mut request: ClaimRequest,
		cancellation: &Cancellation,
		mut wait: impl FnMut(),
	) -> Result<Claim, Problem> {
		let gate = self.lock_file(true)?;
		loop {
			check_cancellation(cancellation)?;
			match gate.try_lock() {
				Ok(()) => break,
				Err(TryLockError::WouldBlock) => wait(),
				Err(TryLockError::Error(e)) => {
					return Err(Problem::at(
						Code::StoreUnavailable,
						&self.0.join("store.lock"),
						e,
					));
				}
			}
		}

		let path = self.0.join("claims.json");
		let mut doc = Document::read(&path)?;
		let paths = request.resolve_paths()?;
		let (claim, changed) = doc
			.claims
			.acquire(paths, request.id.as_deref(), request.owner)?;
		check_cancellation(cancellation)?;
		if changed {
			doc.publish(&path, Some(&claim.claim_id))?;
		}
		Ok(claim)
	}
}

fn check_cancellation(cancellation: &Cancellation) -> Result<(), Problem> {
	if cancellation.is_cancelled() {
		return Err(Problem::new(Code::Cancelled, "Claim cancelled."));
	}
	Ok(())
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
			.claim_with_wait(request, &Cancellation::new(), || {
				assert!(!waited);
				waited = true;
				std::fs::remove_file(&path).unwrap();
				gate.unlock().unwrap();
			})
			.unwrap_err();

		assert!(waited);
		assert!(matches!(problem.code, Code::PathUnresolved));
		let extra = paths::resolve(
			paths::native(fixture.path().join("extra")),
			Requirement::MayBeMissing,
		)
		.unwrap();
		assert!(store.overlaps(&extra).unwrap().is_empty());
	}
}
