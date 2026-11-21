use super::Claim;
use crate::{
	paths::{
		Target,
		names::{self, Coverage},
	},
	problem::Problem,
};
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
	pub(super) fn between(
		request: &Target,
		claim: &Claim,
		path: &NativePath,
	) -> Result<Option<Self>, Problem> {
		let relation = match names::compare(request.resolved.as_path(), path.as_path())? {
			Coverage::Exact => Relation::Exact,
			Coverage::Covers => Relation::Covers,
			Coverage::Descendant => Relation::Descendant,
			Coverage::Disjoint => return Ok(None),
		};

		Ok(Some(Self {
			requested_path: request.supplied.clone(),
			claimed_path: path.clone(),
			relation,
			claim_id: claim.claim_id.clone(),
			owner: claim.owner.clone(),
		}))
	}
}
