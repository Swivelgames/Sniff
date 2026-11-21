//! Shell queries describe possible input without preparing an action.
use crate::{claims::Claim, paths, selectors::Selectors, store::Store};
use clap::{Arg, Command, ValueHint};
use std::ffi::{OsStr, OsString};
use trine::cli::completion::{self, Candidate, Context, OptionValue};

pub(super) fn reply(command: Command, args: Vec<OsString>) -> Vec<u8> {
	let Ok(cwd) = std::env::current_dir() else {
		return Vec::new();
	};

	// At the root the user may type either a subcommand or a shorthand check.
	let command = if follows_shorthand(&command, &args) {
		let globals = command
			.get_arguments()
			.filter(|arg| arg.is_global_set())
			.cloned();
		Command::new("sniff").args(globals)
	} else {
		command
	};
	let command = command
		.arg(Arg::new("path").value_hint(ValueHint::AnyPath))
		.arg(
			Arg::new("workspace")
				.long("workspace")
				.value_hint(ValueHint::DirPath),
		)
		.mut_subcommands(|command| {
			let init = command.get_name() == "init";
			command.mut_args(|arg| {
				if arg.get_long() == Some("workspace") || (init && arg.is_positional()) {
					return arg.value_hint(ValueHint::DirPath);
				}
				arg
			})
		});
	completion::respond(command, args, &cwd).unwrap_or_default()
}

/// Registration scripts pass the shell words after the protocol separator.
fn follows_shorthand(command: &Command, args: &[OsString]) -> bool {
	let [_, separator, words @ ..] = args else {
		return false;
	};
	if separator != "--" {
		return false;
	}
	let index = Selectors::read(words).action_index;
	let cursor = std::env::var("_CLAP_COMPLETE_INDEX")
		.ok()
		.and_then(|s| s.parse::<usize>().ok());
	let Some(word) = words
		.get(index)
		.filter(|_| cursor.is_some_and(|cursor| index < cursor))
	else {
		return false;
	};
	// Readline may split an option assignment at '='. In that case the raw
	// shell words cannot establish a shorthand target after a leading selector.
	if words[..index].iter().any(|w| w == "=") || word == "=" {
		return false;
	}
	word == "--"
		|| (!word.as_encoded_bytes().starts_with(b"-")
			&& word
				.to_str()
				.is_none_or(|name| command.find_subcommand(name).is_none() && name != "help"))
}

pub(crate) fn claim_ids(context: &Context, prefix: &OsStr) -> Vec<Candidate> {
	let Some(store) = selected_store(context) else {
		return Vec::new();
	};
	let Ok(claims) = store.snapshot() else {
		return Vec::new();
	};

	claims
		.into_records()
		.into_iter()
		.filter(|claim| {
			claim
				.claim_id
				.as_bytes()
				.starts_with(prefix.as_encoded_bytes())
		})
		.map(|claim| claim_candidate(claim, store.root()))
		.collect()
}

fn claim_candidate(claim: Claim, root: &std::path::Path) -> Candidate {
	let candidate = Candidate::new(claim.claim_id);
	let Some((first, rest)) = claim.paths.split_first() else {
		return candidate;
	};

	let path = first.as_path();
	let relative = path.strip_prefix(root).unwrap_or(path);
	let mut description = if relative.as_os_str().is_empty() {
		".".to_owned()
	} else {
		relative.display().to_string()
	};
	if !rest.is_empty() {
		description.push_str(&format!(" (+{} more)", rest.len()));
	}
	if let Some(owner) = claim.owner.filter(|owner| !owner.is_empty()) {
		description = format!("{owner} · {description}");
	}
	candidate.description(description)
}

fn selected_store(context: &Context) -> Option<Store> {
	// Trine resolves fields within the action. Sniff also accepts leading selectors.
	let leading = Selectors::read(&context.arguments).workspace;
	let workspace = match (leading, context.field("workspace")) {
		(OptionValue::Absent, OptionValue::Absent) => None,
		(OptionValue::Present(path), OptionValue::Absent)
		| (OptionValue::Absent, OptionValue::Present(path))
			if !path.is_empty() =>
		{
			Some(path)
		}
		_ => return None,
	};
	let path = workspace.map(|value| paths::native(context.cwd.join(value)));
	Store::discover(path.as_ref()).ok()
}
