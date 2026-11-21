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

	fn initialize_mcp(&mut self) {
		let handshake = self.request(
			"initialize",
			json!({
				"protocolVersion":"2025-11-25",
				"capabilities":{},
				"clientInfo":{"name":"sniff-tests","version":"1"}
			}),
		);
		assert_eq!(handshake["result"]["protocolVersion"], "2025-11-25");
		self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
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

	mcp.initialize_mcp();

	let discovery = mcp.request("tools/list", json!({}));
	let mut names: Vec<_> = discovery["result"]["tools"]
		.as_array()
		.unwrap()
		.iter()
		.map(|tool| tool["name"].as_str().unwrap())
		.collect();
	names.sort();
	assert_eq!(names, ["check", "claim", "init", "list", "unclaim"]);

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

#[test]
fn list_and_unclaim_share_state_across_all_interfaces() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let created = cli(root, &["claim", "--reserve", "one", "--reserve", "two"], 0);
	let mut rpc = Connection::open(root, "rpc");
	let mut mcp = Connection::open(root, "mcp");
	mcp.initialize_mcp();

	let scope = json!({ "workspace":root });
	let expected = json!({"claims":[created]});
	assert_eq!(cli(root, &["list"], 0), expected);
	assert_eq!(rpc.rpc("list", scope.clone())["result"], expected);
	assert_eq!(mcp.mcp("list", scope)["structuredContent"], expected);

	let release = json!({"workspace":root, "claim_id":created["claim_id"]});
	let released = mcp.mcp("unclaim", release.clone());
	assert_eq!(
		released["structuredContent"],
		json!({"claim_id":created["claim_id"],"released":true})
	);
	let repeated = rpc.rpc("unclaim", release);
	assert_eq!(
		repeated["result"],
		json!({"claim_id":created["claim_id"],"released":false})
	);
	assert_eq!(cli(root, &["list"], 0), json!({"claims":[]}));
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

#[test]
fn cancelling_an_rpc_release_while_the_store_is_locked_preserves_coverage() {
	let fixture = tempdir().unwrap();
	let root = fixture.path();
	cli(root, &["init"], 0);
	let claim = cli(root, &["claim", "--reserve", "mine"], 0);
	let gate = std::fs::File::options()
		.read(true)
		.write(true)
		.open(root.join(".sniff/store.lock"))
		.unwrap();
	gate.lock().unwrap();

	let mut rpc = Connection::open(root, "rpc");
	rpc.send(json!({"jsonrpc":"2.0","id":42,"method":"invoke",
		"params":{"action":["unclaim"],"input":{"claim_id":claim["claim_id"]}}}));
	// A round trip while the store is locked proves the host processed the invoke.
	let listing = rpc.rpc("list", json!({}));
	assert_eq!(listing["result"], json!({"claims":[claim]}));
	rpc.send(json!({"jsonrpc":"2.0","method":"cancel","params":{"id":42}}));
	let terminal = rpc
		.responses
		.recv_timeout(Duration::from_secs(10))
		.expect("cancelled release");
	let response: Value = serde_json::from_str(&terminal).unwrap();
	assert_eq!(response["id"], 42);
	assert_eq!(response["error"]["data"]["code"], "cancelled", "{response}");

	gate.unlock().unwrap();
	assert_eq!(cli(root, &["list"], 0), json!({"claims":[claim]}));
}
