mod check;
mod claim;
mod init;
mod list;
mod terminal;
mod unclaim;

use serde::{Serialize, de::DeserializeOwned};
use terminal::{HumanOutput, Terminal};
use trine::{
	Action, RegistrationError,
	application::{Application, CliBinding, Entry},
	cli::Bindings,
	structured::StructuredBinding,
};

type SniffEntry<I, O, E> = Entry<I, O, E, ()>;

pub fn application() -> Result<Application, RegistrationError> {
	let mut app = Application::new("sniff", env!("CARGO_PKG_VERSION"))?;
	app.register(init::entry()?)?;
	app.register(check::entry()?)?;
	app.register(claim::entry()?)?;
	app.register(list::entry()?)?;
	app.register(unclaim::entry()?)?;
	Ok(app)
}

fn interfaces<I, O, E>(action: Action<I, O, E, ()>, bindings: Bindings) -> SniffEntry<I, O, E>
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
