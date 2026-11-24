//! Turn classified failures into local next steps while retaining structured details.

use super::{HumanOutput, Render, Tone, field};
use crate::problem::{Code, Problem};

impl HumanOutput for Problem {
	fn render(&self, output: &mut Render) {
		self.render_with_heading(output, None);
	}
}

impl Problem {
	pub(in crate::actions) fn render_with_heading(
		&self,
		output: &mut Render,
		heading: Option<&str>,
	) {
		let name = crate::identity::Identity::name();
		let summary = match self.code {
			Code::InvalidInput | Code::ClaimNotFound | Code::Cancelled => field(&self.message),
			Code::Conflict => "Paths overlap an existing claim.".into(),
			Code::PathUnresolved => "Could not resolve the path.".into(),
			Code::WorkspaceNotFound => {
				format!("No {name} workspace found.")
			}
			Code::StoreUnavailable => format!("Could not access the {name} store."),
			Code::SkillInstallFailed => {
				"Workspace is ready. Could not install the agent skill.".into()
			}
			Code::InvalidStore => format!("The {name} store could not be read."),
			Code::PublicationUncertain => "Could not confirm whether the claim changed.".into(),
		};

		if let Some(heading) = heading {
			output.status(heading, Tone::Attention, None);
			output.detail(format_args!("{summary}"));
		} else {
			output.status(&summary, Tone::Attention, None);
		}

		if let Some(path) = &self.location {
			output.detail(format_args!("{}", output.path(path)));
		}
		if let Some(id) = &self.claim_id {
			output.detail(format_args!("Claim {}", output.claim_id(id)));
		}
		for hit in &self.conflicts {
			let requested = output.path(hit.requested_path.as_path());
			let claimed = output.path(hit.claimed_path.as_path());
			let claim = output.claim_label(&hit.claim_id, hit.owner.as_deref());
			output.detail(format_args!("{requested} overlaps {claimed} · {claim}"));
		}

		match self.code {
			Code::WorkspaceNotFound if crate::identity::Identity::managed() => {
				output.advice("Retry to prepare the claims workspace", "list");
			}
			Code::WorkspaceNotFound => {
				output.advice("From the intended workspace directory, run", "init");
			}
			Code::PathUnresolved if self.missing_target => {
				output.advice(
					"Check the spelling. To claim a future path, use",
					"claim --reserve <path>",
				);
			}
			Code::PublicationUncertain => {
				output.separate();
				output.detail(format_args!(
					"Before retrying, run {} list with the same workspace selection.",
					crate::identity::Identity::prefix()
				));
			}
			Code::SkillInstallFailed if crate::identity::Identity::managed() => {
				output.advice("Check the skill path, then retry", "list");
			}
			Code::SkillInstallFailed => {
				output.separate();
				output.detail(format_args!(
					"Check the skill path and rerun swiss init for the same workspace."
				));
			}
			Code::PathUnresolved | Code::StoreUnavailable | Code::InvalidStore => {
				output.separate();
				output.detail(format_args!("Rerun with --json for details."));
			}
			_ => {}
		}
	}
}
