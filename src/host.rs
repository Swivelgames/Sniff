//! Host commands and the bare-path shorthand around Trine's action catalog.
use crate::actions;
use clap::{Command, error::ErrorKind};
use std::{error::Error, ffi::OsString, io};
use trine::{
	Cancellation,
	application::{Application, CliViewContext, json::JsonResponse},
	cli::{Parsed, output_mode::json_requested},
	error::FailureKind,
	mcp, rpc,
};

pub fn run(mut args: Vec<OsString>) -> Result<u8, Box<dyn Error>> {
	let app = actions::application()?;
	let output = CliViewContext::default();
	if args.len() == 1 {
		args.push("--help".into());
	}
	select_action(&mut args);

	match args[1].to_str() {
		Some("help" | "-h" | "--help" | "-V" | "--version" | "rpc" | "mcp") => {
			let command = app
				.cli_command()
				.about("Check and claim paths. Use sniff <path> to check a path.")
				.subcommand(Command::new("rpc").about("Serve JSON-RPC on stdio"))
				.subcommand(Command::new("mcp").about("Serve MCP tools on stdio"));
			match command.try_get_matches_from(&args) {
				Ok(_) => serve(&app, &args),
				Err(e)
					if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) =>
				{
					output.stdout.write_all(e.to_string().as_bytes())?;
					Ok(0)
				}
				Err(e) => Err(e.into()),
			}
		}
		_ => run_action(&app, args, output),
	}
}

fn select_action(args: &mut Vec<OsString>) {
	// Keep global-looking selectors usable before either an action or a bare path.
	let mut index = 1;
	while let Some(arg) = args.get(index) {
		if arg == "--workspace" {
			index += 2;
		} else if arg == "--json" || arg.as_encoded_bytes().starts_with(b"--workspace=") {
			index += 1;
		} else {
			break;
		}
	}

	match args.get(index).and_then(|arg| arg.to_str()) {
		Some(
			"init" | "check" | "claim" | "rpc" | "mcp" | "help" | "-h" | "--help" | "-V"
			| "--version",
		) => {
			let action = args.remove(index);
			args.insert(1, action);
		}
		_ => args.insert(1, "check".into()),
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
		let (input, output) = mcp::stdio::open()?;
		smol::block_on(mcp::serve(app, input, output, shutdown))?;
	} else {
		let (input, output) = rpc::stdio::open()?;
		let report = smol::block_on(rpc::serve(
			app,
			input,
			output,
			rpc::Config::default(),
			shutdown,
			|_| {},
		))?;
		if let Some(failure) = report.failure {
			return Err(io::Error::other(failure).into());
		}
	}
	Ok(0)
}
