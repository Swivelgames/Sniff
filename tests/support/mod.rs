use serde_json::Value;
use std::{
	ffi::OsStr,
	path::Path,
	process::{Command, Output},
};

pub fn command(root: &Path) -> Command {
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_sniff"));
	cmd.current_dir(root);
	cmd
}

pub fn cli(root: &Path, arguments: &[impl AsRef<OsStr>], exit: i32) -> Value {
	let output = command(root)
		.arg("--json")
		.args(arguments)
		.output()
		.unwrap();
	domain(output, exit)
}

pub fn domain(output: Output, exit: i32) -> Value {
	let stderr = String::from_utf8_lossy(&output.stderr);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert_eq!(output.status.code(), Some(exit), "{stderr}\n{stdout}");

	let document: Value = serde_json::from_slice(&output.stdout).expect("one complete JSON result");
	assert_eq!(document["exit_code"], exit);
	assert!(document["error"].is_null(), "{document}");
	document["result"].clone()
}
