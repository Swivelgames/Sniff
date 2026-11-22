//! Shell queries describe possible input without preparing an action.
use crate::{claims::Claim, paths, selectors::Selectors, store::Store};
use clap::{Arg, Command, ValueHint};
use std::ffi::{OsStr, OsString};
use trine::cli::completion::{self, Candidate, Context, OptionValue};

pub(super) fn reply(command: Command, args: Vec<OsString>) -> Vec<u8> {
	let Ok(cwd) = std::env::current_dir() else {
		return Vec::new();
	};

	completion::respond_with(args, &cwd, |words, cursor| {
		completion_command(command, words, cursor)
	})
	.unwrap_or_default()
}

fn completion_command(command: Command, words: &[OsString], cursor: usize) -> Command {
	let command = command.mut_subcommands(|command| {
		let init = command.get_name() == "init";
		command.mut_args(|arg| {
			if arg.get_long() == Some("workspace") || (init && arg.is_positional()) {
				return arg.value_hint(ValueHint::DirPath);
			}
			arg
		})
	});

	if follows_shorthand(&words[..cursor]) {
		let check = command
			.find_subcommand("check")
			.expect("registered check action");
		let globals = command
			.get_arguments()
			.filter(|arg| arg.is_global_set())
			.cloned();
		return check.clone().args(globals);
	}

	// Until a target is supplied, both a path and an action name are valid.
	command
		.arg(Arg::new("path").value_hint(ValueHint::AnyPath))
		.arg(
			Arg::new("workspace")
				.long("workspace")
				.value_hint(ValueHint::DirPath),
		)
}

fn follows_shorthand(before_cursor: &[OsString]) -> bool {
	let index = Selectors::read(before_cursor).action_index;
	before_cursor
		.get(index)
		.is_some_and(|word| !crate::selectors::is_explicit_action(word))
}

/// Suggest active claim IDs from the selected workspace and attach display context.
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
