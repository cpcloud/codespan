# codespan

[![Continuous integration][actions-badge]][actions-url]
[![Crates.io][crate-badge]][crate-url]
[![Docs.rs][docs-badge]][docs-url]
[![Gitter][gitter-badge]][gitter-lobby]

[actions-badge]: https://img.shields.io/github/workflow/status/brendanzab/codespan/Continuous%20integration
[actions-url]: https://github.com/brendanzab/codespan/actions
[crate-url]: https://crates.io/crates/codespan
[crate-badge]: https://img.shields.io/crates/v/codespan.svg
[docs-url]: https://docs.rs/codespan
[docs-badge]: https://docs.rs/codespan/badge.svg
[gitter-badge]: https://badges.gitter.im/codespan-rs/codespan.svg
[gitter-lobby]: https://gitter.im/codespan-rs/Lobby

Utilities for dealing with source code locations.

## Supporting crates

Codespan also allows you to easily set up pretty diagnostic formatting for
command line interfaces via the [`codespan-reporting`][codespan-reporting]
crate. This will give you output that looks like the following:

![screenshot](./codespan-reporting/assets/screenshot.png)

[Rustdoc][codespan-reporting-docs]

In the future we would also like to make it easy for language developers to set
up language server protocols and interface with browser-embedded editors like
Ace or Monaco.

[codespan-reporting]: https://crates.io/crates/codespan-reporting
[codespan-reporting-docs]: https://docs.rs/codespan-reporting

## Codespan in use

Codespan is used in the following projects:

- [Gluon](https://github.com/gluon-lang/gluon)
- [Pikelet](https://github.com/pikelet-lang/pikelet)
- [Gleam](https://github.com/lpil/gleam/)
- [Arret](https://arret-lang.org)

## Acknowledgments

Inspired by [rustc's error reporting infrastructure][libsyntax], the [codemap][codemap]
crate, and [language-reporting][language-reporting] (a fork of codespan).

[libsyntax]: https://github.com/rust-lang/rust/tree/master/src/libsyntax
[codemap]: https://crates.io/crates/codemap
[language-reporting]: https://crates.io/crates/language-reporting
