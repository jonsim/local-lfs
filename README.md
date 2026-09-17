# local-lfs

[![Build Status](https://travis-ci.org/jonsim/local-lfs.svg?branch=master)](https://travis-ci.org/jonsim/local-lfs)
[![codecov](https://codecov.io/gh/jonsim/local-lfs/branch/master/graph/badge.svg)](https://codecov.io/gh/jonsim/local-lfs)

An implementation of a [git-lfs](https://git-lfs.github.com/) server which can
be hosted locally and is designed to hold its file objects in a highly
compressed form suitable for cloud storage platforms

This allows using traditional Git repository hosting services (e.g. Github,
Bitbucket) without being bound by their binary file store size limits, instead
storing versioned binary files on a separate service.


## Why use git-lfs
Git Large File Storage is a Git extension to allow versioning large binary files
in Git (something traditionally Git is not well suited for). This still uses the
the Git client so allows uninterrupted use of Git workflows and provides an
attractive alternative to traditional, commercial binary repository management
solutions (e.g. Perforce, Plastic SCM).


## Why use local-lfs
git-lfs does not come with a ready-to-use server, instead relying on third-party
hosting services to build support into their platforms. While several such
platforms support the git-lfs protocol (e.g.
[Github](https://help.github.com/articles/configuring-git-large-file-storage/)
and
[Bitbucket](https://confluence.atlassian.com/bitbucket/git-large-file-storage-in-bitbucket-829078514.html)),
their policies (at the time of writing) restrict free repositories to 1 GB total
storage (including version history) and their pricing structures for increasing
the storage limit are not competitive when compared to cloud hosting services.
This is problematic for individuals or small teams with large numbers of binary
assets.

local-lfs aims to solve this by offering a server, trivially hosted locally,
which stores the binary assets. It effectively splits the repository into a
regular git repository, which can be hosted either locally or on any hosting
platform, and a locally hosted binary store. This store is designed to offer
very high levels of compression and be backed by (or served directly from) a
cloud storage platform (e.g. Google Drive, OneDrive, DropBox, Amazon S3 etc.)
which typically offer larger free storage limits and *much* more competitive
pricing structures (by several orders of magnitude) than commercial git-lfs
servers.
This provides all the benefits of using a standard third-party repository host
(e.g. visualisation / workflow tools) without being bound by their lfs pricing
model.



## Features

Still in development, not ready for external use.

### Current Status

The project now implements Git LFS batch negotiation and basic HTTP object
transfers against a persistent local store. The protocol flow is covered by
loopback tests and a real Git LFS client push and fetch system test.

Implemented:

- The CLI accepts `--help`, `--port` (default `9090`), and `--store`. The server
  listens on `127.0.0.1` at the selected port.
- HTTP building blocks parse and format request and response lines, headers,
  status codes, and fixed-length bodies. A message builder assembles requests
  and responses. The Rust suite includes unit tests and loopback TCP tests.
- The live server uses four connection workers and routes `GET /` to a
  `hello world` response and `POST /echo` to a binary echo response. It reads
  bodies using `Content-Length`, sends framed responses, and returns HTTP errors
  for bad requests, unsupported methods, and unknown paths. A client error no
  longer stops the listener. A bounded queue limits waiting sockets, and
  `Expect: 100-continue` works for fixed-length uploads. The Robot suite checks
  the CLI help output and the workflows below.
- `--store` selects a persistent local object directory (default
  `./lfo-store`). `PUT /objects/{oid}` streams an upload to a temporary file,
  checks its `Content-Length` and SHA-256 ID, then publishes it. `GET
  /objects/{oid}` streams the stored bytes back. Invalid or incomplete uploads
  leave no published object.
- `POST /objects/batch` accepts Git LFS JSON for upload and download requests,
  selects the basic transfer adapter, and returns action URLs for missing
  uploads or available downloads. Already stored uploads need no action;
  missing downloads and invalid object claims produce per-object errors.
- The system test creates a local bare Git remote, pushes three LFS-tracked
  binaries (including one over 16 MiB), restarts the server, then pulls into a
  fresh clone and compares the recovered bytes. Another test checks that a
  stalled upload does not delay another client, a wrong-hash upload publishes
  nothing, and a `100 Continue` upload completes. The Git workflow requires
  `git-lfs` on `PATH`.

Still missing:

- Optional verification actions are not implemented.
- Objects are stored as raw bytes. Compression and cloud-backed storage are not
  implemented.
- Non-object request bodies still have a temporary 16 MiB in-memory limit.
  Object transfers stream, but chunked transfer encoding and persistent HTTP
  connections are not supported.



## Getting started

Install a stable Rust toolchain and [`git-lfs`](https://git-lfs.com/). Install
[uv](https://docs.astral.sh/uv/) if you want to run the development checks.
`rustup` will add the `rustfmt` and `clippy` components from
`rust-toolchain.toml`.

Install the development tools:

```sh
uv sync
```

### Build and run

Build the `local-lfs` executable:

```sh
cargo build
```

Run it through Cargo:

```sh
cargo run
```

Or install and run the executable directly:

```sh
cargo install --path .
local-lfs
```

### Use with a Git repository

Run the server from this project directory and leave it running while you push
or fetch LFS files. The store directory holds the object bytes and can be reused
when the server restarts. This example uses the default loopback port and an
ignored directory inside this checkout:

```sh
cargo run -- --port 9090 --store ./lfo-store
```

In another terminal, go to an existing Git repository with an `origin` remote.
Configure Git LFS to send its object requests to this server, then track and
commit a file. Replace `my-file.bin` with a file that exists in your repository;
change the `*.bin` pattern if needed. Run `git lfs track` before adding the file.

```sh
git lfs install --local
git config lfs.url http://127.0.0.1:9090
git config lfs.locksverify false
git lfs track "*.bin"
git add .gitattributes my-file.bin
git commit -m "Track binary file with Git LFS"
git push origin HEAD
```

The Git remote receives the commit and LFS pointer; `./lfo-store` receives the
file bytes. The local `lfs.url` setting stays in this repository's Git config.
It is not committed with the project files. Lock checks are disabled because
this server does not implement the Git LFS locks API. See the
[Git LFS configuration reference](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-config.adoc)
for these settings.

To fetch into a fresh clone on the same computer, skip the automatic LFS
download during clone so you can set the local endpoint first. Replace
`YOUR_GIT_REMOTE_URL` with the URL or path of your Git remote:

```sh
GIT_LFS_SKIP_SMUDGE=1 git clone YOUR_GIT_REMOTE_URL new-checkout
cd new-checkout
git lfs install --local
git config lfs.url http://127.0.0.1:9090
git config lfs.locksverify false
git lfs pull origin
```

The server listens only on `127.0.0.1`, so these commands must run on the same
computer as `local-lfs`. Keep the same store directory when restarting it;
the Git remote contains pointers rather than copies of the LFS file bytes.

### Run the tests

Run the Rust unit tests:

```sh
cargo test
```

Run the CLI smoke, Git LFS push/fetch, and HTTP hardening system tests:

```sh
uv run robot --outputdir target/robot test
```

Format and lint the Python system-test fixture:

```sh
uv run ruff format test
uv run ruff check test
```

### Run the checks

Install the Git hooks once after cloning the project:

```sh
uv run pre-commit install
```

Run every check over the repository whenever needed:

```sh
uv run pre-commit run --all-files
```


## License
All files are licensed under the MIT license.

&copy; Copyright 2018-2026 Jonathan Simmonds
