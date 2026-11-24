//! Coordinate access to paths through cooperative, workspace-local claims.
//!
//! All three hosts use the same actions. A claim reserves each recorded root
//! and its descendants until someone releases its ID. See [`claims`] for
//! conflict rules and [`store`] for how concurrent writers publish coverage.
#![deny(missing_docs)]

mod actions;
mod claims;
mod completion;
mod host;
mod identity;
mod paths;
mod problem;
mod selectors;
mod store;

use std::process::ExitCode;

fn main() -> ExitCode {
	match host::run(std::env::args_os().collect()) {
		Ok(code) => ExitCode::from(code),
		Err(error) => {
			eprintln!("{error}");
			ExitCode::FAILURE
		}
	}
}
