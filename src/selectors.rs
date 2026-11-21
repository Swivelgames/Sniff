//! Leading selectors are accepted before an action name or shorthand path.
use clap_lex::OsStrExt;
use std::ffi::{OsStr, OsString};
use trine::cli::completion::OptionValue;

pub(crate) struct Selectors<'a> {
	pub action_index: usize,
	pub workspace: OptionValue<'a>,
}

impl<'a> Selectors<'a> {
	pub fn read(args: &'a [OsString]) -> Self {
		let mut index = 1;
		let mut workspace = OptionValue::Absent;
		while let Some(arg) = args.get(index) {
			let value = if arg == "--workspace" {
				index += 1;
				args.get(index).map(OsString::as_os_str)
			} else if let Some(value) = arg.as_os_str().strip_prefix("--workspace=") {
				Some(value)
			} else if arg == "--json" {
				index += 1;
				continue;
			} else {
				break;
			};

			workspace = match (workspace, value) {
				(OptionValue::Absent, Some(value)) if !value.is_empty() => {
					OptionValue::Present(value)
				}
				(OptionValue::Absent, _) => OptionValue::Incomplete,
				_ => OptionValue::Ambiguous,
			};
			index += 1;
		}

		Self {
			action_index: index,
			workspace,
		}
	}
}

/// These words select a command or a display request. Other first positionals
/// are shorthand check targets, including a file whose name is a single dash.
pub(crate) fn is_explicit_action(word: &OsStr) -> bool {
	matches!(
		word.to_str(),
		Some(
			"init"
				| "check" | "claim"
				| "list" | "unclaim"
				| "completion"
				| "rpc" | "mcp"
				| "help" | "-h"
				| "--help" | "-V"
				| "--version"
		)
	)
}
