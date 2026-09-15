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

The project is an HTTP server prototype, not yet a usable Git LFS server. It
builds and runs, but a Git LFS client cannot upload or download objects through
it.

Implemented:

- The CLI accepts `--help`, `--port` (default `9090`), and `--store`. The server
  listens on `127.0.0.1` at the selected port.
- HTTP building blocks parse and format request and response lines, headers,
  status codes, and fixed-length bodies. A message builder assembles requests
  and responses. The Rust suite includes unit tests and loopback TCP tests.
- The live server accepts one connection at a time and routes `GET /` to a
  `hello world` response and `POST /echo` to a binary echo response. It reads
  bodies using `Content-Length`, sends framed responses, and returns HTTP errors
  for bad requests, unsupported methods, and unknown paths. A client error no
  longer stops the listener. The Robot system test still checks only that
  `--help` exits successfully.

Still missing:

- The `--store` path is parsed but never used. There is no object store,
  compression, hashing, or persistence.
- No Git LFS protocol endpoints or operations exist: batch negotiation, object
  upload and download, and verification are all absent.
- The handler still processes connections sequentially and holds request bodies
  in memory, with a temporary 16 MiB limit. Chunked transfer encoding and
  `Expect` requests are not supported. Object uploads will need a streaming
  path.
- End-to-end tests need to exercise Git LFS client workflows beyond the
  current CLI smoke test and HTTP loopback tests.



## Getting started

Install a stable Rust toolchain and [uv](https://docs.astral.sh/uv/). `rustup`
will add the `rustfmt` and `clippy` components from `rust-toolchain.toml`.

Install the development tools:

```sh
uv sync
```

### Build and run

Build the `example-app` executable:

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
example-app
```

### Run the tests

Run the Rust unit tests:

```sh
cargo test
```

Run the command-line smoke test:

```sh
uv run robot --outputdir target/robot test
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
