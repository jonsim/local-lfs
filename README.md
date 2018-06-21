# local-lfs
An implementation of a [git-lfs](https://git-lfs.github.com/) server which can
be hosted locally and can echo git commits to a third party git server while
storing large file objects in a local store.

## Why use git-lfs
Git Large File Storage is a Git extension to allow versioning large binary files
in Git (something traditionally Git is not well suited for). This still uses the
the Git client so allows uninterrupted use of Git workflows.

## Why use local-lfs
git-lfs does not come with a ready-to-use server. While
[Github](https://help.github.com/articles/configuring-git-large-file-storage/)
and
[Bitbucket](https://confluence.atlassian.com/bitbucket/git-large-file-storage-in-bitbucket-829078514.html)
support the git-lfs protocol, their policies (at the time of writing) restrict
free repositories to 1 GB total storage and their pricing structures are not
competitive for larger repositories. This is problematic for individuals or
small teams with repositories larger than this.

local-lfs aims to solve this by offering a server, trivially hosted locally,
which forwards git commits to a separate git server (e.g. Github, Bitbucket etc)
and stores large files in a configurable local store. This store is designed to
offer high levels of compression and backed by (or served directly from) a cloud
storage platform (e.g. Google Drive (15 GB free), OneDrive, DropBox, Amazon S3
etc.) which typically offer much larger free storage limits and more competitive
pricing structures than commercial git-lfs servers.

## Features
TODO

## Getting started
TODO

## License
All files are licensed under the MIT license.

&copy; Copyright 2018 Jonathan Simmonds

