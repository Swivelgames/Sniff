//! Render human outcomes on the selected CLI stream.

mod problem;
mod style;

use self::style::Style;
pub(super) use self::style::Tone;
use std::{
	fmt::{self, Write},
	io,
	path::{Path, PathBuf},
};
use trine::{application::CliViewContext, view::View};

pub(super) trait HumanOutput {
	fn render(&self, output: &mut Render);
}

/// Escape caller-supplied text before adding indentation or styles.
fn field(value: &str) -> String {
	let mut text = String::with_capacity(value.len());
	for ch in value.chars() {
		if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
			text.extend(ch.escape_default());
		} else {
			text.push(ch);
		}
	}
	text
}

/// A small buffer that shares indentation, path labels and styles across outcomes.
pub(super) struct Render {
	text: String,
	style: Style,
	cwd: Option<PathBuf>,
	canonical_cwd: Option<PathBuf>,
}

impl Render {
	fn new(stream: &trine::application::Output) -> Self {
		let cwd = std::env::current_dir().ok();
		let canonical_cwd = cwd.as_ref().and_then(|path| path.canonicalize().ok());

		Self {
			text: String::new(),
			style: Style::for_stream(stream),
			cwd,
			canonical_cwd,
		}
	}

	pub(super) fn path(&self, path: &Path) -> String {
		let relative = self
			.cwd
			.iter()
			.chain(&self.canonical_cwd)
			.find_map(|cwd| path.strip_prefix(cwd).ok());
		let path = match relative {
			Some(path) if path.as_os_str().is_empty() => Path::new("."),
			Some(path) => path,
			None => path,
		};
		let label = path
			.to_str()
			.map(field)
			.unwrap_or_else(|| field(&format!("{:?}", path.as_os_str())));

		self.style.paint(&label, Tone::Path).into_owned()
	}

	pub(super) fn claim_id(&self, id: &str) -> String {
		self.style.paint(&field(id), Tone::Claim).into_owned()
	}

	pub(super) fn claim_label(&self, id: &str, owner: Option<&str>) -> String {
		let id = self.claim_id(id);
		let Some(owner) = owner else { return id };

		let owner = field(owner);
		format!("{id} ({})", self.style.paint(&owner, Tone::Owner))
	}

	pub(super) fn heading(&mut self, text: &str) {
		self.status(text, Tone::Heading, None);
	}

	pub(super) fn status(&mut self, text: &str, tone: Tone, subject: Option<&str>) {
		write!(self.text, "  {}", self.style.paint(text, tone)).expect("string output");
		if let Some(subject) = subject {
			write!(self.text, " · {subject}").expect("string output");
		}
		self.text.push('\n');
	}

	pub(super) fn detail(&mut self, content: fmt::Arguments<'_>) {
		writeln!(self.text, "    {content}").expect("string output");
	}

	pub(super) fn nested_detail(&mut self, content: fmt::Arguments<'_>) {
		self.detail(format_args!("  {content}"));
	}

	pub(super) fn separate(&mut self) {
		self.text.push('\n');
	}

	pub(super) fn advice(&mut self, label: &str, command: &str) {
		self.separate();
		self.heading(label);
		self.detail(format_args!("{}", self.style.paint(command, Tone::Command)));
	}
}

pub(crate) fn heading(stream: &trine::application::Output) -> String {
	let title = concat!("🧀 Sniff (v", env!("CARGO_PKG_VERSION"), ")");
	format!(
		"\n  {}\n",
		Style::for_stream(stream).paint(title, Tone::Heading)
	)
}

pub(super) struct Terminal(pub CliViewContext);

impl<O, E> View<O, E, ()> for Terminal
where
	O: HumanOutput + Sync,
	E: HumanOutput + Sync,
{
	type Error = io::Error;
	type Completion = ();

	async fn finish(&mut self, result: &Result<O, E>) -> io::Result<()> {
		let stream = if result.is_ok() {
			&self.0.stdout
		} else {
			&self.0.stderr
		};
		let mut output = Render::new(stream);

		match result {
			Ok(value) => value.render(&mut output),
			Err(error) => error.render(&mut output),
		}

		stream.write_all(output.text.as_bytes())
	}
}
