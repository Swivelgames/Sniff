//! Initialize a workspace and return its resolved coordination root.

mod skill;

use super::{
	SniffEntry, interfaces,
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
}

#[derive(Serialize)]
pub(super) struct Ready {
	workspace: NativePath,
}

impl HumanOutput for Ready {
	fn render(&self, output: &mut Render) {
		let workspace = output.path(self.workspace.as_path());
		output.status("Workspace ready", Tone::Success, Some(&workspace));
		output.advice("Check a path from your workspace", "sniff check <path>");
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

			skill::install(store.root()).map_err(|error| {
				Problem::at(
					Code::SkillInstallFailed,
					&store.root().join(".agents/skills/sniff/SKILL.md"),
					error,
				)
			})?;

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
