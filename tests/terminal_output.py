"""Check Swiss's human presentation with terminal and redirected output on Unix."""

import errno
import json
import os
import pty
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def terminal(binary, root, args, env):
	reader, writer = pty.openpty()
	try:
		with os.fdopen(writer, "wb") as tty:
			result = subprocess.run(
				[binary, *args], cwd=root, env=env, stdout=tty, stderr=tty, timeout=10
			)
		chunks = []
		while True:
			try:
				chunk = os.read(reader, 4096)
			except OSError as error:
				if error.errno != errno.EIO:
					raise
				break
			if not chunk:
				break
			chunks.append(chunk)

		return result.returncode, b"".join(chunks).replace(b"\r\n", b"\n")
	finally:
		os.close(reader)


def main():
	binary = Path(sys.argv[1]).resolve()
	env = dict(os.environ, TERM="xterm-256color")
	env.pop("NO_COLOR", None)

	with tempfile.TemporaryDirectory() as root:
		code, output = terminal(binary, root, ["check", "file"], env)
		assert code == 1
		assert b"\x1b[36mswiss init\x1b[0m" in output
		plain = re.sub(rb"\x1b\[[0-9;]*m", b"", output)
		assert re.match(r"\n  🧀 Swiss \(v[^\n]+\)\n\n  Undetermined\n", plain.decode())

		for settings in (dict(env, NO_COLOR="1"), dict(env, TERM="dumb")):
			assert terminal(binary, root, ["check", "file"], settings) == (1, plain)

		code, output = terminal(binary, root, ["--json", "check", "file"], env)
		assert code == 1
		assert json.loads(output)["result"]["status"] == "undetermined"
		assert b"\x1b" not in output

		code, output = terminal(binary, root, ["init"], env)
		assert code == 0
		assert b"\x1b[1;32mWorkspace ready\x1b[0m" in output

		claim = subprocess.run(
			[binary, "--json", "claim", "--reserve", "file", "--owner", "writer"],
			cwd=root,
			env=env,
			capture_output=True,
			check=True,
			timeout=10,
		)
		claim_id = json.loads(claim.stdout)["result"]["claim_id"].encode()

		for args in (["list"], ["check", "file"]):
			code, output = terminal(binary, root, args, env)
			assert code == 0
			assert b"\x1b[1;36m" + claim_id + b"\x1b[0m" in output
			assert b"\x1b[35mwriter\x1b[0m" in output
			assert b"\x1b[94mfile\x1b[0m" in output

			plain = re.sub(rb"\x1b\[[0-9;]*m", b"", output)
			assert terminal(binary, root, args, dict(env, NO_COLOR="1")) == (0, plain)

			piped = subprocess.run(
				[binary, *args], cwd=root, env=env, capture_output=True,
				check=True, timeout=10,
			)
			assert piped.stdout == plain.split(b"\n\n", 1)[1]
			assert not piped.stderr


if __name__ == "__main__":
	main()
