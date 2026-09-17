"""Robot keyword that exercises local-lfs through a real Git LFS client.

The bare Git remote in this test stores commits and pointers. The local-lfs
process stores the object bytes, just as it would with an external Git host.
"""

import hashlib
import http.client
import os
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SERVER_BINARY = PROJECT_ROOT / "target" / "debug" / "local-lfs"
OBJECTS = {
    "small.bin": b"real Git LFS push and fetch\x00\xff\n",
    "middle.bin": bytes(range(256)) * 2048 + b"different object",
    # A body above the 16 MiB batch/echo limit proves object PUT streams.
    "large.bin": bytes(range(256)) * (17 * 4096) + b"last bytes",
}


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


def _start_server(port, store):
    """Start the server against one persistent store and wait for readiness."""
    process = subprocess.Popen(
        [str(SERVER_BINARY), "--port", str(port), "--store", str(store)],
        cwd=PROJECT_ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        _wait_for_server(process, port)
    except Exception:
        _stop_server(process)
        raise
    return process


def _stop_server(process):
    """Never leave a background listener behind after an assertion fails."""
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _check_other_clients_while_upload_stalls(port, store):
    """A client withholding its PUT body must not block a separate GET."""
    slow = socket.create_connection(("127.0.0.1", port), timeout=2)
    try:
        oid = hashlib.sha256(b"unfinished upload").hexdigest()
        slow.sendall(
            f"PUT /objects/{oid} HTTP/1.1\r\nContent-Length: 17\r\n\r\n".encode()
        )
        # Wait for the temporary upload file: this proves the server entered
        # the slow request before the second client tries to connect.
        deadline = time.monotonic() + 2
        while not any(store.rglob(".upload-*")) and time.monotonic() < deadline:
            time.sleep(0.01)
        if not any(store.rglob(".upload-*")):
            raise AssertionError("server never entered the stalled upload")
        # Another worker should return the ordinary health route without
        # waiting for the first worker's read timeout.
        fast = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
        try:
            fast.request("GET", "/")
            response = fast.getresponse()
            if response.status != 200 or response.read() != b"hello world":
                raise AssertionError("stalled upload delayed another client")
        finally:
            fast.close()
    finally:
        slow.close()
    # Closing the partial client body must also remove the unpublished temp.
    deadline = time.monotonic() + 2
    while any(store.rglob(".upload-*")) and time.monotonic() < deadline:
        time.sleep(0.01)
    if any(store.rglob(".upload-*")) or any(store.rglob(oid)):
        raise AssertionError("partial upload left an object or temporary file")


def _check_failed_upload_does_not_publish(port, store):
    """A wrong SHA-256 claim must fail without leaving an object in storage."""
    expected_oid = hashlib.sha256(b"correct bytes").hexdigest()
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.request("PUT", f"/objects/{expected_oid}", body=b"wrong bytes")
        response = connection.getresponse()
        if response.status != 422:
            raise AssertionError(
                f"wrong-hash upload returned {response.status}, not 422"
            )
        response.read()
    finally:
        connection.close()
    if any(store.rglob(expected_oid)):
        raise AssertionError("wrong-hash upload published an object")


def _check_expect_continue_upload(port, store):
    """An upload waiting on 100 Continue should finish and persist its bytes."""
    content = b"send only after the interim response"
    oid = hashlib.sha256(content).hexdigest()
    with socket.create_connection(("127.0.0.1", port), timeout=2) as client:
        client.sendall(
            f"PUT /objects/{oid} HTTP/1.1\r\n"
            f"Expect: 100-continue\r\nContent-Length: {len(content)}\r\n\r\n".encode()
        )
        interim = b"HTTP/1.1 100 Continue\r\n\r\n"
        received = b""
        while len(received) < len(interim):
            part = client.recv(len(interim) - len(received))
            if not part:
                raise AssertionError("server closed before 100 Continue")
            received += part
        if received != interim:
            raise AssertionError(f"unexpected interim response: {received!r}")
        client.sendall(content)
        client.shutdown(socket.SHUT_WR)
        final = b""
        while b"\r\n" not in final:
            part = client.recv(1024)
            if not part:
                break
            final += part
        if not final.startswith(b"HTTP/1.1 200 OK\r\n"):
            raise AssertionError(f"continued upload failed: {final!r}")
    stored = next(store.rglob(oid), None)
    if stored is None or stored.read_bytes() != content:
        raise AssertionError("continued upload did not persist its bytes")


def server_stays_responsive_to_bad_clients():
    """Check stalled, invalid, and 100-continue requests on a live server."""
    _run("cargo", "build", "--quiet", cwd=PROJECT_ROOT)
    with tempfile.TemporaryDirectory(prefix="local-lfs-http-") as temp:
        store = Path(temp) / "objects"
        port = _free_loopback_port()
        server = _start_server(port, store)
        try:
            _check_other_clients_while_upload_stalls(port, store)
            _check_failed_upload_does_not_publish(port, store)
            _check_expect_continue_upload(port, store)
        finally:
            _stop_server(server)


def git_lfs_push_and_fetch():
    """Push several LFS files, restart, then fetch into an empty LFS cache."""
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

        server = _start_server(port, store)
        try:
            _run("git", "init", "--bare", str(remote), env=git_env)
            _run(
                "git",
                "symbolic-ref",
                "HEAD",
                "refs/heads/main",
                cwd=remote,
                env=git_env,
            )
            _run("git", "init", "-b", "main", str(source), env=git_env)
            _run(
                "git", "config", "user.name", "Local LFS Test", cwd=source, env=git_env
            )
            _run(
                "git",
                "config",
                "user.email",
                "local-lfs@example.invalid",
                cwd=source,
                env=git_env,
            )
            _run("git", "config", "lfs.url", endpoint, cwd=source, env=git_env)
            _run("git", "config", "lfs.locksverify", "false", cwd=source, env=git_env)
            _run("git", "lfs", "install", "--local", cwd=source, env=git_env)
            _run("git", "lfs", "track", "*.bin", cwd=source, env=git_env)

            for name, content in OBJECTS.items():
                (source / name).write_bytes(content)
            _run("git", "add", ".gitattributes", *OBJECTS, cwd=source, env=git_env)
            _run(
                "git", "commit", "-m", "Track binary payloads", cwd=source, env=git_env
            )
            for name, content in OBJECTS.items():
                oid = hashlib.sha256(content).hexdigest()
                pointer = _run(
                    "git", "show", f"HEAD:{name}", cwd=source, env=git_env
                ).stdout
                if f"oid sha256:{oid}" not in pointer:
                    raise AssertionError(
                        f"Git committed {name} as bytes, not an LFS pointer"
                    )
            _run("git", "remote", "add", "origin", str(remote), cwd=source, env=git_env)
            # The Git pre-push hook invokes the actual LFS batch and PUT flow.
            _run("git", "push", "-u", "origin", "main", cwd=source, env=git_env)

            for name, content in OBJECTS.items():
                oid = hashlib.sha256(content).hexdigest()
                stored = next(store.rglob(oid), None)
                if stored is None or stored.read_bytes() != content:
                    raise AssertionError(f"Git push did not publish {name}")

            # Download after a restart, proving the actions come from the
            # persistent store rather than the previous process's memory.
            _stop_server(server)
            server = _start_server(port, store)

            # Clone without smudging so this second repo has no cached bytes.
            # Its explicit LFS pull must use batch download and GET actions.
            clone_env = git_env.copy()
            clone_env["GIT_LFS_SKIP_SMUDGE"] = "1"
            _run("git", "clone", str(remote), str(clone), env=clone_env)
            _run("git", "config", "lfs.url", endpoint, cwd=clone, env=git_env)
            _run("git", "config", "lfs.locksverify", "false", cwd=clone, env=git_env)
            _run("git", "lfs", "install", "--local", cwd=clone, env=git_env)
            for name, content in OBJECTS.items():
                if (clone / name).read_bytes() == content:
                    raise AssertionError(f"clone unexpectedly has {name} before pull")
            _run("git", "lfs", "pull", "origin", cwd=clone, env=git_env)
            for name, content in OBJECTS.items():
                if (clone / name).read_bytes() != content:
                    raise AssertionError(f"Git LFS pull did not restore {name}")
        finally:
            _stop_server(server)
