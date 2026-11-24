//! Initialize a workspace and return its resolved coordination root.

mod skill;

use super::{
	SwissEntry, interfaces,
	terminal::{HumanOutput, Render, Tone},
};
use crate::{
	paths,
	problem::{Code, Problem},
	store::Store,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use trine::{Action, NativePath, RegistrationError, cli::Bindings};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
	/// Coordination root. Defaults to the current directory.
	directory: Option<NativePath>,
	/// Optional heading for an embedding application.
	pub display_name: Option<String>,
	/// Display-only command prefix for help and suggestions.
	pub invocation_prefix: Option<String>,
}

#[derive(Serialize)]
pub(super) struct Ready {
	workspace: NativePath,
}

impl HumanOutput for Ready {
	fn render(&self, output: &mut Render) {
		let workspace = output.path(self.workspace.as_path());
		output.status("Workspace ready", Tone::Success, Some(&workspace));
		output.advice("Check a path from your workspace", "check <path>");
	}
}

pub(super) fn entry() -> Result<SwissEntry<Input, Ready, Problem>, RegistrationError> {
	let action = Action::new("init", "Initialize a Swiss workspace", |input: Input, _| {
		smol::unblock(move || {
			let identity = crate::identity::Identity {
				display_name: input.display_name,
				invocation_prefix: input.invocation_prefix,
			};
			identity
				.validate()
				.map_err(|error| Problem::new(Code::InvalidInput, error.to_string()))?;

			let root = input
				.directory
				.as_ref()
				.map_or(Path::new("."), NativePath::as_path);

			let store = Store::initialize(root)?;

			identity
				.save(store.root())
				.map_err(|error| Problem::at(Code::StoreUnavailable, store.root(), error))?;

			skill::install(store.root()).map_err(|error| {
				Problem::at(
					Code::SkillInstallFailed,
					&store.root().join(".agents/skills/swiss/SKILL.md"),
					error,
				)
			})?;

			Ok(Ready {
				workspace: paths::native(store.root().to_owned()),
			})
		})
	})?;
	let bindings = Bindings::new()
		.option("display_name", "display-name")
		.option("invocation_prefix", "invocation-prefix")
		.positional("directory")
		.native_path("directory");
	Ok(interfaces(action, bindings))
}
