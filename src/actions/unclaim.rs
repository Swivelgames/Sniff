//! Release a complete claim by ID. Repeating the release succeeds.

use super::{
	SniffEntry, interfaces,
	terminal::{HumanOutput, Render, Tone},
};
use crate::{problem::Problem, store::Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trine::{Action, InvocationContext, NativePath, RegistrationError, cli::Bindings};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
	/// ID of the claim to release.
	claim_id: String,
	/// Use the store at this exact directory.
	workspace: Option<NativePath>,
}

#[derive(Serialize)]
pub(super) struct Release {
	claim_id: String,
	released: bool,
}

impl HumanOutput for Release {
	fn render(&self, output: &mut Render) {
		let id = output.claim_id(&self.claim_id);
		if self.released {
			output.status("Released claim", Tone::Success, Some(&id));
			return;
		}

		output.status("Claim already absent", Tone::Heading, Some(&id));
	}
}

fn release(input: Input, context: InvocationContext<()>) -> Result<Release, Problem> {
	let store = Store::discover(input.workspace.as_ref())?;
	let released = store.unclaim(&input.claim_id, context.cancellation())?;
	Ok(Release {
		claim_id: input.claim_id,
		released,
	})
}

pub(super) fn entry() -> Result<SniffEntry<Input, Release, Problem>, RegistrationError> {
	let action = Action::new("unclaim", "Release a claim by ID", |input, context| {
		smol::unblock(move || release(input, context))
	})?;
	let bindings = Bindings::new()
		.positional("claim_id")
		.option("workspace", "workspace")
		.native_path("workspace")
		.complete_candidates(
			"claim_id",
			Some("Active claims"),
			crate::completion::claim_ids,
		);
	Ok(interfaces(action, bindings))
}
