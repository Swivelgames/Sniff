//! Serialize changes from reading current coverage through publishing its replacement.
use super::{
	Store,
	document::{Change, Document},
};
use crate::{
	claims::Claims,
	problem::{Code, Problem},
};
use std::{fs::TryLockError, thread, time::Duration};
use trine::Cancellation;

impl Store {
	pub(super) fn mutate<T>(
		&self,
		cancellation: &Cancellation,
		apply: impl FnOnce(&mut Claims) -> Result<(T, Option<Change>), Problem>,
	) -> Result<T, Problem> {
		self.mutate_with_wait(cancellation, apply, || {
			thread::sleep(Duration::from_millis(15))
		})
	}

	pub(super) fn mutate_with_wait<T>(
		&self,
		cancellation: &Cancellation,
		apply: impl FnOnce(&mut Claims) -> Result<(T, Option<Change>), Problem>,
		mut wait: impl FnMut(),
	) -> Result<T, Problem> {
		let gate = self.lock_file(true)?;
		loop {
			check_cancellation(cancellation)?;
			match gate.try_lock() {
				Ok(()) => break,
				Err(TryLockError::WouldBlock) => wait(),
				Err(TryLockError::Error(error)) => {
					return Err(Problem::at(
						Code::StoreUnavailable,
						&self.0.join("store.lock"),
						error,
					));
				}
			}
		}

		let path = self.0.join("claims.json");
		let mut document = Document::read(&path)?;
		let (result, change) = apply(&mut document.claims)?;
		check_cancellation(cancellation)?;
		if let Some(change) = change {
			document.publish(&path, Some(&change))?;
		}
		Ok(result)
	}
}

fn check_cancellation(cancellation: &Cancellation) -> Result<(), Problem> {
	if cancellation.is_cancelled() {
		return Err(Problem::new(Code::Cancelled, "Operation cancelled."));
	}
	Ok(())
}
