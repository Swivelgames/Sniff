use super::{SniffEntry, interfaces, terminal::HumanOutput};
use crate::{paths, problem::Problem, store::Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use trine::{Action, NativePath, RegistrationError, cli::Bindings};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
	/// Coordination root. Defaults to the current directory.
	directory: Option<NativePath>,
}

#[derive(Serialize)]
pub(super) struct Ready {
	workspace: NativePath,
}

impl HumanOutput for Ready {
	fn text(&self) -> String {
		format!("Workspace ready at {}", self.workspace.as_path().display())
	}
}

pub(super) fn entry() -> Result<SniffEntry<Input, Ready, Problem>, RegistrationError> {
	let action = Action::new("init", "Initialize a Sniff workspace", |input: Input, _| {
		smol::unblock(move || {
			let root = input
				.directory
				.as_ref()
				.map_or(Path::new("."), NativePath::as_path);
			let store = Store::initialize(root)?;
			Ok(Ready {
				workspace: paths::native(store.root().to_owned()),
			})
		})
	})?;
	let bindings = Bindings::new()
		.positional("directory")
		.native_path("directory");
	Ok(interfaces(action, bindings))
}
