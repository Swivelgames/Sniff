//! Read coverage for one path, including a path that does not yet exist.

use super::{
	SwissEntry, interfaces,
	terminal::{HumanOutput, Render, Tone},
};
use crate::{
	claims::{Overlap, Relation},
	paths::{self, Requirement},
	problem::Problem,
	store::Store,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trine::{Action, NativePath, RegistrationError, cli::Bindings};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
	/// Existing or anticipated path to inspect.
	path: NativePath,
	/// Use the store at this exact directory.
	workspace: Option<NativePath>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum Check {
	Clear {
		path: NativePath,
	},
	Claimed {
		path: NativePath,
		overlaps: Vec<Overlap>,
	},
}

#[derive(Serialize)]
pub(super) struct Undetermined {
	status: &'static str,
	problem: Problem,
}

impl HumanOutput for Undetermined {
	fn render(&self, output: &mut Render) {
		self.problem
			.render_with_heading(output, Some("Undetermined"));
	}
}

impl HumanOutput for Check {
	fn render(&self, output: &mut Render) {
		match self {
			Self::Clear { path } => {
				let path = output.path(path.as_path());
				output.status("Clear", Tone::Success, Some(&path));
			}
			Self::Claimed { path, overlaps } => {
				let path = output.path(path.as_path());
				output.status("Claimed", Tone::Attention, Some(&path));

				for hit in overlaps {
					let relation = match hit.relation {
						Relation::Exact => "Exact match",
						Relation::Covers => "Covered by",
						Relation::Descendant => "Contains claim on",
					};
					let path = output.path(hit.claimed_path.as_path());
					let claim = output.claim_label(&hit.claim_id, hit.owner.as_deref());
					output.detail(format_args!("{relation} {path} · {claim}"));
				}
			}
		}
	}
}

fn inspect(input: Input) -> Result<Check, Problem> {
	let store = Store::discover(input.workspace.as_ref())?;
	let target = paths::resolve(input.path, Requirement::MayBeMissing)?;
	let overlaps = store.overlaps(&target)?;
	let path = target.resolved;
	if overlaps.is_empty() {
		return Ok(Check::Clear { path });
	}
	Ok(Check::Claimed { path, overlaps })
}

pub(super) fn entry() -> Result<SwissEntry<Input, Check, Undetermined>, RegistrationError> {
	let action = Action::new(
		"check",
		"Check a path for overlapping claims",
		|input, _| {
			smol::unblock(move || {
				inspect(input).map_err(|problem| Undetermined {
					status: "undetermined",
					problem,
				})
			})
		},
	)?;
	let bindings = Bindings::new()
		.positional("path")
		.native_path("path")
		.option("workspace", "workspace")
		.native_path("workspace");
	Ok(interfaces(action, bindings).mcp_observation())
}
