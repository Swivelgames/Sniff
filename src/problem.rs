//! Classify failures for all hosts and retain actionable conflict evidence.

use crate::claims::Overlap;
use serde::Serialize;
use std::{fmt::Display, path::Path};

/// Stable diagnostic categories shared by the terminal and structured hosts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
	InvalidInput,
	PathUnresolved,
	WorkspaceNotFound,
	StoreUnavailable,
	PublicationUncertain,
	InvalidStore,
	ClaimNotFound,
	Conflict,
	Cancelled,
}

/// Classified failure with optional claim ID and the roots involved in a conflict.
#[derive(Debug, Serialize)]
pub struct Problem {
	pub code: Code,
	pub message: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub conflicts: Vec<Overlap>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub claim_id: Option<String>,
}

impl Problem {
	pub fn new(code: Code, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
			conflicts: vec![],
			claim_id: None,
		}
	}

	pub fn at(code: Code, path: &Path, cause: impl Display) -> Self {
		Self::new(code, format!("{}: {cause}", path.display()))
	}
}
