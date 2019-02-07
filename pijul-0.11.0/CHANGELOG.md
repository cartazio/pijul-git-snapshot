# CHANGELOG

## pijul-0.10.0

This version is focused on bugfixes, after the difficult landing of pijul-0.9.0.
It has also been the occasion to dramatically increase the code coverage of the
pijul tests. This effort should worth the long await for this version, because
we can hope for more stable releases in the future.

## pijul-0.9.0

This entry also contains the changes from `pijul-0.8.1` to `pijul-0.8.3`.

### Breaking Change

This version of pijul introduces a new patch format, with no backward
compatibility. Its main benefit is to simplify *a lot* the algorithms of Pijul.
To avoid history losses, we have written a converter. It does not produce a
perfectly equivalent repository. Because it replays the repository history, one
patch at a time, it does not care about conflicts. As a consequence, the
conflict markers will be recorded as-is.

### Enhancements

* It is now possible to write a patch name and description using an external
  editor
* There is now a new hook, called `post-record`
* `pijul checkout` will now abort if there is pending changes to prevent loosing
  them; a new flag (`--force`) relaxes this behaviour
* `pijul pull` will not report progress where it has nothing to do
* Several unit and integration tests have been written, in an attempt to prevent
  regressions
* `pijul` can now be used behind a proxy
* `pijul record` gets a new flag (`-n`) to add untracked files
* `pijul changes` is renamed `pijul log` and has a new option (`--grep`)
* `pijul` now uses the base58 encoding in place of base64 to print its hashes

### Bug Fixes

* Two similar patches could produce unwanted conflict markers after one was
  unrecorded
* The `TAG` flag was not set by `pijul tag`
* A change in rust std led to incorrect permission settings by `pijul`
* `pijul push` to a private server was failing
* `pijul show-dependencies` could produce incorrect an incorrect dot files
* Insertions were lost when unrecording and re-recording a given patch multiples
  time
* `pijul clone` was not setting the current branch properly when cloning to a branch
  other than master
* Some files were getting deleted when pulling patches that deleted, and then
  added them
* `pijul show-dependencies --depth` output was incorrect

## pijul-0.8.0

### Enhancements

* Add a new flag `--recursive` to `pijul add` to add a directory and its content
  recursively
* Add support for hooks (`pre-record` and `patch-name`)
* Manually add patch dependencies when recording a new patch
* Pijul can now deal with cyclic conflicts when unrecording a patch
* Add the patch ID in `pijul changes` output
* Conditionally use liner or rustyline to be more portable
* Download patches into a temporary directory and rename them later to perform
  “atomic downloads”
* Improve the conflict markers for conflicts targeting zombie lines
* Show a cursor for changes while recording a patch (in the form of “x/y”)
* Use a pager for `pijul changes`, `pijul blame` and `pijul diff`
* Check if a patch hash given as argument of pijul commands is a valid base64
  representation before using it
* Pijul does not need stdin to be a pty anymore
* Report an error when dropping a SSH session
* `pijul dist` can now create an archive of a subset of the repository
* `pijul show-dependencies` can now take several patches as arguments
* `pijul clone` and `pijul pull` report their progression while downloading and
  applying patches

### Bug Fixes

* Moving a file then reverting was removing the file
* An incorrect unsafe call to Sanakirja was introducing a bug in patch ordering
* Pijul was incorrectly complaining about missing dependencies when applying
  some patches
* Pijul needed two patches to move a tracked file into an untracked directory
* `pijul apply` was waiting for something from stdin for no valid reason
* `pijul unrecord` was introducing wrong conflicts when unrecording only one
  side of the concurrent deletion of some edges
* `pijul checkout <branch>` was sometimes failing with a “not enough space”
  error message
* Fix several inconsistencies or errors in the pijul UI, including command
  arguments and outputs
* Pijul had issues to handle the removal of a tracked file not yet recorded
  (that is, `pijul add <file>; rm <file>; pijul record` failed)
* Recursion limit was too low for Windows
* Handle Ctrl-C correctly in `pijul record`

### Dependencies

* getch: 0.1.1 -> 0.2.0
* libc is no longer a dependency
* liner 1.0 is a new dependency for unix
* pager 0.12 is a new dependency
* termion is a new dependency for unix
* username is a new dependency
* isatty is a new dependency

## pijul-7.3 and before

This ChangeLog entry is unfortunately incomplete.

### Features

* `pijul status`: print the list of staging changes
* `.ignore` and `.pijul/local/ignore` files can be used to filter the `pijul
  status` output.

### Enhancement

* Add a per-user config file
* Improve test coverage

### Bug Fixes

* Fixing a stack overflow in `pijul apply`

### Dependencies

* app_dirs 1.1 is a new dependency
* bitflags: 0.8 -> 0.9
* hyper-rustly: 0.5 -> 0.6

## pijul-0.6.0

### Features

* `pijul keys` to deal with ssh and signing keys
* Add support for signed patches

## pijul-0.5.13

### Enhancement

* Improve the look and feel of the `changes` command

### Dependencies

* `rustc_serialize` is no longer a dependency of pijul, as it has been
  deprecated in favor of serde by the Rust team

## pijul-0.5.11

### Features

* Add the notion of *remotes* in pijul so it is possible to save a list of know
  remote repositories with a unique name
* It is possible to set a default remote for pulling patches

### Enhancement

* Improve the error message when pijul cannot load a private key while pulling
  patches from a remote repository

### Bug Fixes

* The `dist` command only adds the `.tar.gz` extension if missing in its argument

### Dependencies

* All pijul dependencies now use `ring-0.9`
