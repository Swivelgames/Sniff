//! Leading selectors are accepted before an action name or shorthand path.
use clap_lex::OsStrExt;
use std::ffi::OsString;
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
