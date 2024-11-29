# Russh Process Command

This crate provides an interface similar to `std::process::Command` for [`russh`](https://docs.rs/russh) library.
It allows executing commands, streaming their output, and retrieving their exit status.

## Features

- Mimics the `std::process::Command` API.
- Provides asynchronous `stdin`, `stdout`, and `stderr` streams.
- Supports retrieving the command's `ExitStatus`.
- Provides `spawn` and `output` methods for more control over process handling.

## Example

* [examples/client_exec.rs](examples/client_exec.rs)
