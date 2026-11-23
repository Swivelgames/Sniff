//! Register claim operations once and bind them to terminal, JSON-RPC and MCP views.

mod check;
mod claim;
mod init;
mod list;
mod terminal;
mod unclaim;

pub(crate) use terminal::heading;

use serde::{Serialize, de::DeserializeOwned};
use terminal::{HumanOutput, Terminal};
use trine::{
	Action, RegistrationError,
	application::{Application, CliBinding, Entry},
	cli::Bindings,
	structured::StructuredBinding,
};

type SwissEntry<I, O, E> = Entry<I, O, E, ()>;

/// Build the shared action registry with the input bindings and views for each host.
pub fn application() -> Result<Application, RegistrationError> {
	let mut app = Application::new("swiss", env!("CARGO_PKG_VERSION"))?;
	app.register(init::entry()?)?;
	app.register(check::entry()?)?;
	app.register(claim::entry()?)?;
	app.register(list::entry()?)?;
	app.register(unclaim::entry()?)?;

	Ok(app)
}

fn interfaces<I, O, E>(action: Action<I, O, E, ()>, bindings: Bindings) -> SwissEntry<I, O, E>
where
	I: DeserializeOwned + Send + 'static,
	O: HumanOutput + Serialize + Sync + Send + 'static,
	E: HumanOutput + Serialize + Sync + Send + 'static,
{
	let cli = CliBinding::new(bindings).capacity(None);

	Entry::new(action)
		.cli(
			cli,
			|context| Ok(Terminal(context)),
			|result| u8::from(result.is_err()),
		)
		.rpc(StructuredBinding::serde())
		.mcp(StructuredBinding::serde())
}
