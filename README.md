# Coaly


A Rust library for logging and tracing.
There are various libraries for that purpose around, but especially in situations where direct debugging
isn't possible or spurious errors must be tracked down, Coaly can be worth a try
due to some unique features:

- event based output mode for log and trace messages. Output mode means filtering of messages according to their
  associated level (e.g. error or warning). Usually, the output mode is defined once upon application start and on a
  per-module basis. In Coaly, the default output mode is set upon application start and may change whenever
  configurable events like a certain function call or structure instantiation occur.
- configurable formatting of log and trace messages
- support for output resource types file, memory mapped file, console and network
- file based resources may be level-, thread-, process- or application speficic
- built-in rollover of file resources based either on file size or time

Documentation:

-   [API reference (master branch)](https://github.com/FrankSommer-64/coaly)
-   [API reference (docs.rs)](https://docs.rs/crate/coaly/0.1.1)


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
coaly = "0.1.1"
```

To get started using Coaly, view the demo application in the examples folder.  
The doc folder contains a configuration file with all available parameters.


## Versions

Coaly is still under construction.

Current Coaly versions are:

-   Version 0.1.1 - feature complete, but still widely untested

A detailed [changelog](CHANGELOG.md) is available for releases.  
For planned releases check roadmap.pdf in the doc folder.


### Rust version requirements

Coaly complies to the 2021 Rust standard and requires **Rustc version 1.36 or greater**.

## Crate Features

Coaly is built with this features enabled by default:

-   `core` enables functionality without network support

Optional, the following features can be added:

-   `compression` enables compression of older log files, implied by `all`
-   `net` enables network functionality including a dedicated logging server, implied by `all`

# License

Coaly is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.
