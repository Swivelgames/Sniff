use super::{SniffEntry, interfaces, terminal::HumanOutput};
use crate::{
	claims::Claim,
	problem::Problem,
	store::{ClaimRequest, Store},
};
use schemars::JsonSchema;
use serde::Deserialize;
use trine::{Action, InvocationContext, NativePath, RegistrationError, cli::Bindings};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
	/// Claim existing files or directories together.
	#[serde(default)]
	paths: Vec<NativePath>,
	/// Also reserve an existing or anticipated location. Repeat for each path.
	#[serde(default)]
	reserve: Vec<NativePath>,
	/// Extend the active claim with this ID.
	claim_id: Option<String>,
	/// Display label for a new claim.
	owner: Option<String>,
	/// Use the store at this exact directory.
	workspace: Option<NativePath>,
}

impl HumanOutput for Claim {
	fn text(&self) -> String {
		let count = self.paths.len();
		let unit = if count == 1 { "path" } else { "paths" };
		format!("Claim {} · {count} {unit}", self.claim_id)
	}
}

fn acquire(input: Input, context: InvocationContext<()>) -> Result<Claim, Problem> {
	let store = Store::discover(input.workspace.as_ref())?;

	let request = ClaimRequest {
		existing: input.paths,
		reservations: input.reserve,
		id: input.claim_id,
		owner: input.owner,
	};
	store.claim(request, context.cancellation())
}

pub(super) fn entry() -> Result<SniffEntry<Input, Claim, Problem>, RegistrationError> {
	let action = Action::new(
		"claim",
		"Reserve paths or extend an active claim",
		|input, context| smol::unblock(move || acquire(input, context)),
	)?;
	let bindings = Bindings::new()
		.positional("paths")
		.native_path("paths")
		.option("reserve", "reserve")
		.native_path("reserve")
		.option("claim_id", "claim-id")
		.option("owner", "owner")
		.option("workspace", "workspace")
		.native_path("workspace");
	Ok(interfaces(action, bindings))
}
