//! Classify failures for all hosts and retain actionable conflict evidence.

use crate::claims::Overlap;
use serde::Serialize;
use std::{
	fmt::Display,
	path::{Path, PathBuf},
};

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

	/// Native path to display in the human diagnostic.
	#[serde(skip)]
	pub location: Option<PathBuf>,
	/// An explicitly requested existing path has an ordinary missing component.
	#[serde(skip)]
	pub missing_target: bool,
}

impl Problem {
	pub fn new(code: Code, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
			conflicts: vec![],
			claim_id: None,
			location: None,
			missing_target: false,
		}
	}

	pub fn at(code: Code, path: &Path, cause: impl Display) -> Self {
		let mut problem = Self::new(code, format!("{}: {cause}", path.display()));
		problem.location = Some(path.to_owned());
		problem
	}

	/// Retain the requested path for display and the failing component for diagnostics.
	pub fn missing_path(requested: &Path, missing_component: &Path) -> Self {
		let mut problem = Self::at(
			Code::PathUnresolved,
			missing_component,
			"path does not exist; check its spelling or use --reserve",
		);
		problem.location = Some(requested.to_owned());
		problem.missing_target = true;
		problem
	}
}
