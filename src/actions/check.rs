use super::{SniffEntry, interfaces, terminal::HumanOutput};
use crate::{
	claims::{Overlap, Relation},
	paths::{self, Requirement},
	problem::Problem,
	store::Store,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
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
	fn text(&self) -> String {
		format!("Undetermined · {}", self.problem.text())
	}
}

impl HumanOutput for Check {
	fn text(&self) -> String {
		match self {
			Self::Clear { path } => format!("Clear · {}", path.as_path().display()),
			Self::Claimed { path, overlaps } => {
				let mut text = format!("Claimed · {}", path.as_path().display());
				for hit in overlaps {
					let relation = match hit.relation {
						Relation::Exact => "exact",
						Relation::Covers => "covered by",
						Relation::Descendant => "contains",
					};
					write!(
						text,
						"\n    {relation} {} · {}",
						hit.claimed_path.as_path().display(),
						hit.claim_id
					)
					.expect("string output");
					if let Some(owner) = &hit.owner {
						write!(text, " ({owner})").expect("string output");
					}
				}
				text
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

pub(super) fn entry() -> Result<SniffEntry<Input, Check, Undetermined>, RegistrationError> {
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
