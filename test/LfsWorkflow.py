"""Robot keyword that exercises local-lfs through a real Git LFS client.

The bare Git remote in this test stores commits and pointers. The local-lfs
process stores the object bytes, just as it would with an external Git host.
"""

import hashlib
import os
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent.parent
SERVER_BINARY = PROJECT_ROOT / "target" / "debug" / "local-lfs"
OBJECT_BYTES = b"real Git LFS push and fetch\x00\xff\n"


def _run(*args, cwd=None, env=None):
    """Run a command and retain its output in assertion failures."""
    result = subprocess.run(
        args, cwd=cwd, env=env, capture_output=True, text=True, timeout=60, check=False
    )
    if result.returncode:
        raise AssertionError(
            f"{' '.join(args)} failed ({result.returncode})\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def _free_loopback_port():
    """Pick a temporary port for the subprocess-based server."""
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _wait_for_server(process, port):
    """Wait for the listener, reporting startup errors instead of hanging."""
    for _ in range(100):
        if process.poll() is not None:
            stdout, stderr = process.communicate()
            raise AssertionError(f"local-lfs exited early\n{stdout}\n{stderr}")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return
        except OSError:
            time.sleep(0.05)
    raise AssertionError("local-lfs did not start within five seconds")


def git_lfs_push_and_fetch():
    """Push an LFS file, then fetch it into a clone with an empty LFS cache."""
    if shutil.which("git-lfs") is None:
        raise AssertionError("git-lfs must be installed and available on PATH")

    _run("cargo", "build", "--quiet", cwd=PROJECT_ROOT)
    # Ignore machine-wide Git settings so credentials, hooks, and user identity
    # from a developer's own repositories cannot affect this local test.
    git_env = os.environ.copy()
    git_env["GIT_CONFIG_NOSYSTEM"] = "1"
    git_env["GIT_CONFIG_GLOBAL"] = os.devnull

    with tempfile.TemporaryDirectory(prefix="local-lfs-workflow-") as temp:
        root = Path(temp)
        remote = root / "remote.git"
        source = root / "source"
        clone = root / "clone"
        store = root / "objects"
        port = _free_loopback_port()
        endpoint = f"http://127.0.0.1:{port}"

        server = subprocess.Popen(
            [str(SERVER_BINARY), "--port", str(port), "--store", str(store)],
            cwd=PROJECT_ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            _wait_for_server(server, port)

            _run("git", "init", "--bare", str(remote), env=git_env)
            _run("git", "symbolic-ref", "HEAD", "refs/heads/main", cwd=remote, env=git_env)
            _run("git", "init", "-b", "main", str(source), env=git_env)
            _run("git", "config", "user.name", "Local LFS Test", cwd=source, env=git_env)
            _run("git", "config", "user.email", "local-lfs@example.invalid", cwd=source, env=git_env)
            _run("git", "config", "lfs.url", endpoint, cwd=source, env=git_env)
            _run("git", "config", "lfs.locksverify", "false", cwd=source, env=git_env)
            _run("git", "lfs", "install", "--local", cwd=source, env=git_env)
            _run("git", "lfs", "track", "*.bin", cwd=source, env=git_env)

            (source / "payload.bin").write_bytes(OBJECT_BYTES)
            _run("git", "add", ".gitattributes", "payload.bin", cwd=source, env=git_env)
            _run("git", "commit", "-m", "Track binary payload", cwd=source, env=git_env)
            oid = hashlib.sha256(OBJECT_BYTES).hexdigest()
            pointer = _run("git", "show", "HEAD:payload.bin", cwd=source, env=git_env).stdout
            if f"oid sha256:{oid}" not in pointer:
                raise AssertionError("Git committed bytes instead of an LFS pointer")
            _run("git", "remote", "add", "origin", str(remote), cwd=source, env=git_env)
            # The Git pre-push hook invokes the actual LFS batch and PUT flow.
            _run("git", "push", "-u", "origin", "main", cwd=source, env=git_env)

            stored = next(store.rglob(oid), None)
            if stored is None or stored.read_bytes() != OBJECT_BYTES:
                raise AssertionError("Git push did not publish the expected LFS object")

            # Clone without smudging so this second repo has no cached bytes.
            # Its explicit LFS pull must use batch download and GET actions.
            clone_env = git_env.copy()
            clone_env["GIT_LFS_SKIP_SMUDGE"] = "1"
            _run("git", "clone", str(remote), str(clone), env=clone_env)
            _run("git", "config", "lfs.url", endpoint, cwd=clone, env=git_env)
            _run("git", "config", "lfs.locksverify", "false", cwd=clone, env=git_env)
            _run("git", "lfs", "install", "--local", cwd=clone, env=git_env)
            if (clone / "payload.bin").read_bytes() == OBJECT_BYTES:
                raise AssertionError("clone unexpectedly has the LFS object before pull")
            _run("git", "lfs", "pull", "origin", cwd=clone, env=git_env)
            if (clone / "payload.bin").read_bytes() != OBJECT_BYTES:
                raise AssertionError("Git LFS pull did not restore the original bytes")
        finally:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
