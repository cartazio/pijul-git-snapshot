# Pijul

Pijul is a version control system based on *patches*, that can mimic
the behaviour and workflows of both [Git](https://git-scm.org) and
[Darcs](http://darcs.net), but contrarily to those systems, Pijul is
based on a **mathematically sound theory of patches**.


Pijul was started out of frustration that no version control system
was at the same time fast and sound:

- Git has non-associative merges, which might lead to security problems. Concretely, this means that the commits you merge might not be the same as the ones you review and test. [More details here](https://nest.pijul.com/help/patches.html).

- Handling of conflicts: Pijul has an explicit internal representation of conflicts, a rock-solid theory of how they behave, and super-fast data structures to handle them.

- Speed! The complexity of Pijul is low in all cases, whereas previous attempts to build a mathematically sound distributed version control system had huge worst-case complexities. The use of [Rust](//www.rust-lang.org) additionally yields a blazingly fast implementation.


## License

The license is GPL2, or any later version at your convenience. This was changed from the time when Pijul was still a prototype, and had another license.

## Getting Started

Pijul depends on [libsodium](https://libsodium.org) and [openssl](https://www.openssl.org/).

You can find Pijul on [crates.io](https://crates.io/crates/pijul). The easiest way to install it is to use cargo:

```
cargo install --force pijul
```

The `--force` flag is used so you can upgrade Pijul if you have a previous version already installed. Once the command has been executed, you can find the `pijul` binary in `~/.cargo/bin/`. You might want to add an alias in your shell profile.

Pijul looks for its configuration in `$PIJUL_CONFIG_DIR` first, then in `$HOME/.pijulconfig` if the former is not set.

## Contributing

We welcome contributions, even if you understand nothing of patch theory.
Currently, the main areas where Pijul needs improvements are:

- Portable handling of SSH keys (Windows and Linux).
- Error messages. There are very few useful messages at the moment.
- HTTP Redirects and errors.

The first step towards contributing is to *clone the repositories*. Pijul depends on a number of packages maintained by the same team, the two largest ones being [Sanakirja](/pijul_org/sanakirja) and [Thrussh](/pijul_org/thrussh).
Here is how to build and install the pijul repositories:

```
$ pijul clone https://nest.pijul.com/pijul_org/pijul
$ cd pijul
$ cargo build
```

If you want to replace the version installed by Cargo with you own build, it is as simple as:

```
$ cargo install
```

By contributing, you agree to make all your contributions GPL2+.

Moreover, the main platform for contributing is [the Nest](//nest.pijul.com/pijul_org/pijul), which is still experimental. Therefore, even though we do our best to avoid it, our repository might be reset, causing the patches of all contributors to be merged. Feel free to add your name in CONTRIBUTORS.md.

If you want to propose a change, you should proceed as follows:

1. Create a [new discussion on the Nest](https://nest.pijul.com/pijul_org/pijul/discussions), to gather feedback on your proposal.
2. Make your change, record a patch (by using `pijul record`).
3. Push it to the Nest. You do not need to create a fork on the Nest, as you would on GitHub for instance, to propose a change. You can actually “push your change” directly to the discussion. When you created your discussion, it got assigned a number. If this number is, for instance, 271, then you can propose a change by pushing to the branch `#272`, just like that:

```
pijul push <user>@nest.pijul.com:pijul_org/pijul --to-branch #271
```

Be aware that you might need to prefix the `#` by `\`, for instance if you use zsh.

We use `rustfmt` to enforce a coding style on pijul source code. You can have a look at [the `rustfmt` repository](https://github.com/rust-lang-nursery/rustfmt) for how to install it. To be sure not to forget to run `rustfmt` before recording your change, you can use the `pre-hook` hook, by creating an executable file at `.pijul/hooks/pre-record`, with the following content:

```
#!/usr/bin/bash

cargo fmt
```

Please make sure to comply with the rustfmt coding style before submitting your patches!
