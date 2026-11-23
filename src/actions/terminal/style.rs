//! Keep terminal emphasis local to the stream receiving it.

use std::borrow::Cow;
use trine::application::Output;

pub(in crate::actions) enum Tone {
	Heading,
	Success,
	Attention,
	Command,
	Claim,
	Owner,
	Path,
}

pub(super) struct Style {
	color: bool,
}

impl Style {
	pub(super) fn for_stream(stream: &Output) -> Self {
		let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
		let dumb = std::env::var_os("TERM").is_some_and(|value| value == "dumb");

		Self {
			color: stream.is_terminal() && !no_color && !dumb,
		}
	}

	pub(super) fn paint<'a>(&self, text: &'a str, tone: Tone) -> Cow<'a, str> {
		if !self.color {
			return Cow::Borrowed(text);
		}

		let sgr = match tone {
			Tone::Heading => "1",
			Tone::Success => "1;32",
			Tone::Attention => "1;33",
			Tone::Command => "36",
			Tone::Claim => "1;36",
			Tone::Owner => "35",
			Tone::Path => "94",
		};
		format!("\x1b[{sgr}m{text}\x1b[0m").into()
	}
}
