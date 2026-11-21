mod actions;
mod claims;
mod completion;
mod host;
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
