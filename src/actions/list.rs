use super::{
	SniffEntry, interfaces,
	terminal::{HumanOutput, field},
};
use crate::{claims::Claim, problem::Problem, store::Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
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
	fn text(&self) -> String {
		if self.claims.is_empty() {
			return "No active claims.".into();
		}

		let mut text = String::new();
		for claim in &self.claims {
			if !text.is_empty() {
				text.push('\n');
			}
			text.push_str(&claim.claim_id);
			if let Some(owner) = &claim.owner {
				write!(text, " ({})", field(owner)).expect("string output");
			}
			for path in &claim.paths {
				write!(text, "\n    {}", field(&path.as_path().to_string_lossy()))
					.expect("string output");
			}
		}
		text
	}
}

fn list(input: Input) -> Result<Listing, Problem> {
	let store = Store::discover(input.workspace.as_ref())?;
	let claims = store.snapshot()?.into_records();
	Ok(Listing { claims })
}

pub(super) fn entry() -> Result<SniffEntry<Input, Listing, Problem>, RegistrationError> {
	let action = Action::new("list", "List active claims and their paths", |input, _| {
		smol::unblock(move || list(input))
	})?;
	let bindings = Bindings::new()
		.option("workspace", "workspace")
		.native_path("workspace");
	Ok(interfaces(action, bindings).mcp_observation())
}
