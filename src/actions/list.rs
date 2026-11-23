//! List active reservations, their owner labels and the roots each ID covers.

use super::{
	SwissEntry, interfaces,
	terminal::{HumanOutput, Render},
};
use crate::{claims::Claim, problem::Problem, store::Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trine::{Action, NativePath, RegistrationError, cli::Bindings};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
	/// Use the store at this exact directory.
	workspace: Option<NativePath>,
}

#[derive(Serialize)]
pub(super) struct Listing {
	claims: Vec<Claim>,
}

impl HumanOutput for Listing {
	fn render(&self, output: &mut Render) {
		if self.claims.is_empty() {
			output.heading("No active claims.");
			return;
		}

		output.heading("Active claims");

		for claim in &self.claims {
			let label = output.claim_label(&claim.claim_id, claim.owner.as_deref());
			output.separate();
			output.detail(format_args!("{label}"));
			for path in &claim.paths {
				output.nested_detail(format_args!("{}", output.path(path.as_path())));
			}
		}
	}
}

fn list(input: Input) -> Result<Listing, Problem> {
	let store = Store::discover(input.workspace.as_ref())?;
	let claims = store.snapshot()?.into_records();
	Ok(Listing { claims })
}

pub(super) fn entry() -> Result<SwissEntry<Input, Listing, Problem>, RegistrationError> {
	let action = Action::new("list", "List active claims and their paths", |input, _| {
		smol::unblock(move || list(input))
	})?;
	let bindings = Bindings::new()
		.option("workspace", "workspace")
		.native_path("workspace");
	Ok(interfaces(action, bindings).mcp_observation())
}
