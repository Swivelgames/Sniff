use super::Claim;
use crate::paths::Target;
use serde::Serialize;
use trine::NativePath;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
	Exact,
	Covers,
	Descendant,
}

#[derive(Debug, Serialize)]
pub struct Overlap {
	pub requested_path: NativePath,
	pub claimed_path: NativePath,
	pub relation: Relation,
	pub claim_id: String,
	pub owner: Option<String>,
}

impl Overlap {
	pub(super) fn between(request: &Target, claim: &Claim, path: &NativePath) -> Option<Self> {
		let requested = request.resolved.as_path();
		let relation = if requested == path.as_path() {
			Relation::Exact
		} else if requested.starts_with(path) {
			Relation::Covers
		} else if path.as_path().starts_with(requested) {
			Relation::Descendant
		} else {
			return None;
		};

		Some(Self {
			requested_path: request.supplied.clone(),
			claimed_path: path.clone(),
			relation,
			claim_id: claim.claim_id.clone(),
			owner: claim.owner.clone(),
		})
	}
}
