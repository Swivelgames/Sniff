//! Host commands and the bare-path shorthand around Trine's action catalog.
use crate::{actions, completion, selectors::Selectors};
use clap::{Arg, Command, error::ErrorKind};
use std::{error::Error, ffi::OsString};
use trine::{
	Cancellation,
	application::{Application, CliViewContext, json::JsonResponse},
	cli::{Parsed, output_mode::json_requested},
	error::FailureKind,
	mcp, rpc,
};

/// Select completion, stdio serving or a command-line action from native arguments.
pub fn run(mut args: Vec<OsString>) -> Result<u8, Box<dyn Error>> {
	let mut app = actions::application()?;
	let output = CliViewContext::default();

	if trine::cli::completion::requested() {
		let bytes = completion::reply(command(&app), args);
		output.stdout.write_all(&bytes)?;
		return Ok(0);
	}

	if output.stdout.is_terminal() {
		app.cli_heading(actions::heading(&output.stdout));
	}

	if args.len() == 1 {
		args.push("--help".into());
	}
	select_action(&mut args);

	match args[1].to_str() {
		Some("help" | "-h" | "--help" | "-V" | "--version" | "rpc" | "mcp" | "completion") => {
			run_host(&app, &args, &output)
		}
		_ => run_action(&app, args, output),
	}
}

fn run_host(
	app: &Application,
	args: &[OsString],
	output: &CliViewContext,
) -> Result<u8, Box<dyn Error>> {
	let mut command = command(app);
	let matches = match command.clone().try_get_matches_from(args) {
		Ok(matches) => matches,
		Err(error)
			if matches!(
				error.kind(),
				ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
			) =>
		{
			output.stdout.write_all(error.to_string().as_bytes())?;
			return Ok(0);
		}
		Err(error) => return Err(error.into()),
	};

	if let Some(matches) = matches.subcommand_matches("completion") {
		if json_requested(args) {
			return Err("--json cannot be combined with completion".into());
		}
		let shell = matches.get_one::<String>("shell").expect("required shell");
		let script =
			trine::cli::completion::generate(&mut command, shell.parse().expect("validated shell"));
		output.stdout.write_all(&script)?;
		return Ok(0);
	}

	serve(app, args)
}

fn command(app: &Application) -> Command {
	let shell =
		Arg::new("shell")
			.required(true)
			.value_parser(["bash", "zsh", "fish", "powershell"]);
	app.cli_command()
		.about("Coordinate filesystem claims. Use sniff <path> to check a path.")
		.subcommand(Command::new("rpc").about("Serve JSON-RPC on stdio"))
		.subcommand(Command::new("mcp").about("Serve MCP tools on stdio"))
		.subcommand(
			Command::new("completion")
				.about("Print shell completion registration")
				.arg(shell),
		)
}

fn select_action(args: &mut Vec<OsString>) {
	let index = Selectors::read(args).action_index;

	if args
		.get(index)
		.is_some_and(|word| crate::selectors::is_explicit_action(word))
	{
		let action = args.remove(index);
		args.insert(1, action);
	} else {
		args.insert(1, "check".into());
	}
}

fn run_action(
	app: &Application,
	args: Vec<OsString>,
	output: CliViewContext,
) -> Result<u8, Box<dyn Error>> {
	let json = json_requested(&args);
	let parsed = match app.parse_cli(args) {
		Ok(parsed) => parsed,
		Err(error) if json => {
			JsonResponse::failure(FailureKind::InvalidInput, error.to_string())
				.write(&output.stdout)?;
			return Ok(1);
		}
		Err(error) => return Err(error.into()),
	};
	match parsed {
		Parsed::Display(text) => {
			output.stdout.write_all(text.as_bytes())?;
			Ok(0)
		}
		Parsed::Input(prepared) => {
			let report = prepared.run(output, Cancellation::new())?;
			Ok(report.exit_code())
		}
	}
}

fn serve(app: &Application, args: &[OsString]) -> Result<u8, Box<dyn Error>> {
	if args.len() != 2 {
		return Err(format!("{} takes no arguments", args[1].to_string_lossy()).into());
	}
	let shutdown = Cancellation::new();
	if args[1] == "mcp" {
		mcp::run_stdio(app, shutdown)?;
	} else {
		rpc::run_stdio(app, shutdown)?;
	}
	Ok(0)
}
