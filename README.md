# mtcp – Metal TCP

[![Crates.io][crates-badge]][crates-url]
[![Unlicense][unlicense-badge]][unlicense-url]
![Timeout][timeout-badge]
![Cancellation][cancellation-badge]

[crates-badge]: https://img.shields.io/crates/v/mtcp-rs.svg
[crates-url]: https://crates.io/crates/mtcp-rs
[unlicense-badge]: https://img.shields.io/badge/license-Unlicense-blue.svg
[unlicense-url]: LICENSE
[timeout-badge]: https://img.shields.io/badge/timeout-✔-brightgreen.svg
[cancellation-badge]: https://img.shields.io/badge/cancellation-✔-brightgreen.svg

**mtcp** provides a “blocking” implementation of `TcpListener` and
`TcpStream` with proper ***timeout*** and ***cancellation*** support. The
"blocking" I/O operations in **mtcp** are emulated via *non-blocking*
operations, using the [**`mio`**](https://github.com/tokio-rs/mio) library.

**Crates.io:**  
https://crates.io/crates/mtcp-rs

**API Documentation:**  
https://docs.rs/mtcp-rs/latest/index.html

**Examples:**  
https://github.com/dEajL3kA/mtcp/tree/master/examples

**Discuss:**  
https://users.rust-lang.org/t/87983
