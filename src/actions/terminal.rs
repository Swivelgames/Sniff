//! Render human-readable outcomes while keeping machine outputs structured.

use crate::problem::Problem;
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};

pub(super) trait HumanOutput {
	fn text(&self) -> String;
}

/// Keep one caller-supplied value on one terminal line.
pub(super) fn field(value: &str) -> String {
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

impl HumanOutput for Problem {
	fn text(&self) -> String {
		let mut text = self.message.clone();
		for overlap in &self.conflicts {
			let owner = overlap
				.owner
				.as_deref()
				.map(|s| format!(" ({s})"))
				.unwrap_or_default();
			write!(
				text,
				"\n    {} overlaps {} · {}{owner}",
				overlap.requested_path.as_path().display(),
				overlap.claimed_path.as_path().display(),
				overlap.claim_id
			)
			.expect("string output");
		}
		text
	}
}

pub(super) struct Terminal(pub CliViewContext);

impl<O, E> View<O, E, ()> for Terminal
where
	O: HumanOutput + Sync,
	E: HumanOutput + Sync,
{
	type Error = io::Error;
	type Completion = ();

	async fn progress(&mut self, (): ()) -> io::Result<()> {
		Ok(())
	}

	async fn finish(&mut self, result: &Result<O, E>) -> io::Result<()> {
		let (stream, text) = match result {
			Ok(value) => (&self.0.stdout, value.text()),
			Err(error) => (&self.0.stderr, error.text()),
		};
		stream.write_all(format!("{text}\n").as_bytes())
	}
}
