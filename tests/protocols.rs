mod support;

use serde_json::{Value, json};
use std::{
	io::{BufRead, BufReader, Write},
	path::Path,
	process::{Child, ChildStdin, Stdio},
	sync::mpsc::{Receiver, channel},
	thread,
	time::Duration,
};
use support::{cli, command};
use tempfile::tempdir;

struct Connection {
	child: Child,
	input: ChildStdin,
	responses: Receiver<String>,
	next_id: u64,
}

impl Connection {
	fn open(root: &Path, host: &str) -> Self {
		let mut child = command(root)
			.arg(host)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.unwrap();
		let input = child.stdin.take().unwrap();
		let output = child.stdout.take().unwrap();
		let (send, responses) = channel();
		thread::spawn(move || {
			for line in BufReader::new(output).lines().map_while(Result::ok) {
				if send.send(line).is_err() {
					break;
				}
			}
		});
		Self {
			child,
			input,
			responses,
			next_id: 1,
		}
	}

	fn send(&mut self, message: Value) {
		writeln!(self.input, "{message}").unwrap();
		self.input.flush().unwrap();
	}

	fn request(&mut self, method: &str, params: Value) -> Value {
		let id = self.next_id;
		self.next_id += 1;
		self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
		let line = self
			.responses
			.recv_timeout(Duration::from_secs(10))
			.expect("live host response");
		let response: Value = serde_json::from_str(&line).unwrap();
		assert_eq!(response["id"], id, "{response}");
		response
	}

	fn rpc(&mut self, action: &str, input: Value) -> Value {
		self.request("invoke", json!({ "action":[action], "input":input }))
	}

	fn mcp(&mut self, name: &str, arguments: Value) -> Value {
		let response = self.request("tools/call", json!({ "name":name, "arguments":arguments }));
		assert!(response["error"].is_null(), "{response}");
		response["result"].clone()
	}
}

impl Drop for Connection {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

#[test]
fn independent_hosts_observe_and_extend_the_same_cli_claim() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	let ready = cli(root, &["init"], 0);
	let mut rpc = Connection::open(root, "rpc");
	let mut mcp = Connection::open(root, "mcp");

	let handshake = mcp.request(
		"initialize",
		json!({
			"protocolVersion":"2025-11-25",
			"capabilities":{},
			"clientInfo":{"name":"sniff-tests","version":"1"}
		}),
	);
	assert_eq!(handshake["result"]["protocolVersion"], "2025-11-25");
	mcp.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));

	let discovery = mcp.request("tools/list", json!({}));
	let mut names: Vec<_> = discovery["result"]["tools"]
		.as_array()
		.unwrap()
		.iter()
		.map(|tool| tool["name"].as_str().unwrap())
		.collect();
	names.sort();
	assert_eq!(names, ["check", "claim", "init"]);

	let workspace = json!({ "directory":root });
	assert_eq!(rpc.rpc("init", workspace.clone())["result"], ready);
	assert_eq!(mcp.mcp("init", workspace)["structuredContent"], ready);

	let created = cli(
		root,
		&["claim", "--reserve", "src/payments", "--owner", "one"],
		0,
	);
	let extension = json!({"workspace":root, "claim_id":created["claim_id"], "reserve":["docs"]});
	let extended = rpc.rpc("claim", extension.clone())["result"].clone();
	assert_eq!(extended["claim_id"], created["claim_id"]);
	assert_eq!(extended["paths"].as_array().unwrap().len(), 2);
	let repeated = mcp.mcp("claim", extension);
	assert_eq!(repeated["structuredContent"], extended);

	let path = "src/payments/new.rs";
	let check = json!({"path":path,"workspace":root});
	let checked = cli(root, &[path], 0);
	assert_eq!(checked["overlaps"][0]["claim_id"], created["claim_id"]);
	assert_eq!(rpc.rpc("check", check.clone())["result"], checked);
	assert_eq!(mcp.mcp("check", check)["structuredContent"], checked);

	let input = json!({"reserve":[path],"owner":"one"});
	let conflict = cli(root, &["claim", "--reserve", path, "--owner", "one"], 1);
	assert_eq!(rpc.rpc("claim", input.clone())["error"]["data"], conflict);
	let refused = mcp.mcp("claim", input);
	assert_eq!(refused["isError"], true);
	assert_eq!(refused["structuredContent"], conflict);
}

#[cfg(unix)]
#[test]
fn absolute_targets_in_an_explicit_workspace_survive_removal_of_the_host_cwd() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let cwd = root.join("ephemeral");
	std::fs::create_dir(&cwd).unwrap();
	let mut rpc = Connection::open(&cwd, "rpc");

	let target = root.join("future");
	let check = json!({ "workspace":root, "path":target });
	assert_eq!(rpc.rpc("check", check.clone())["result"]["status"], "clear");
	std::fs::remove_dir(&cwd).unwrap();

	let claim = rpc.rpc("claim", json!({ "workspace":root, "reserve":[target] }));
	let id = claim["result"]["claim_id"]
		.as_str()
		.expect("claim succeeds with absolute paths");
	assert_eq!(
		rpc.rpc("check", check)["result"]["overlaps"][0]["claim_id"],
		id
	);
}
