#![doc = include_str!("../README.md")]
use core::pin::Pin;
use core::task::{Context, Poll};
use russh::{Channel, ChannelId, ChannelMsg};
use std::io;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, SimplexStream, WriteHalf};
use tokio::task::JoinHandle;

mod child;
/// The extension for
/// [`russh::client`](https://docs.rs/russh/latest/russh/client/index.html)
pub mod client;
pub use crate::child::{Child, ChildStderr, ChildStdin, ChildStdout, Output};
use crate::child::ChildImp;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub enum Error {
    IoError(#[from] io::Error),
    Russh(#[from] russh::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    pub(crate) inner: ExitStatusImp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitStatusImp {
    Code(u32),
    Processing,
}

impl ExitStatus {
    /// Returns true if the command was successful.
    pub fn success(&self) -> bool {
        match &self.inner {
            ExitStatusImp::Code(code) => *code == 0,
            ExitStatusImp::Processing => false,
        }
    }

    /// Retrieves the exit code if available.
    /// Returns `None` if the process is still in progress.
    pub fn code(&self) -> Option<u32> {
        match &self.inner {
            ExitStatusImp::Code(code) => Some(*code),
            ExitStatusImp::Processing => None,
        }
    }
}

/// Represents a command to be executed over an SSH channel.
///
/// Mimics the behavior of [`std::process::Command`].
/// Encapsulates the SSH channel and the command to be executed.
#[derive(Debug)]
pub struct Command<S>
where
    S: From<(ChannelId, ChannelMsg)> + Send + Sync + 'static,
{
    inner: Channel<S>,
    command: Vec<u8>,
}

impl<S> Command<S>
where
    S: From<(ChannelId, ChannelMsg)> + Send + Sync + 'static,
{
    pub fn from_channel<A: Into<Vec<u8>>>(inner: Channel<S>, command: A) -> Self {
        let command = command.into();
        Self { inner, command }
    }
}


impl<S> Command<S>
where
    S: From<(ChannelId, ChannelMsg)> + Send + Sync + 'static,
{
    pub async fn spawn(self) -> Result<Child, Error> {
        self.inner.exec(true, self.command).await?;
        use tokio::io::simplex;
        let stdin_buf_size = 4096;
        let stdout_buf_size = 4096;
        let stderr_buf_size = 4096;

        let (stdin_rx, stdin_tx) = simplex(stdin_buf_size);
        let (stdout_rx, stdout_tx) = simplex(stdout_buf_size);
        let (stderr_rx, stderr_tx) = simplex(stderr_buf_size);
        let stdin = ChildStdin { inner: stdin_tx };
        let stdout = ChildStdout { inner: stdout_rx };
        let stderr = ChildStderr { inner: stderr_rx };

        let imp = ChildImp {
            channel: self.inner,
            stdin_rx,
            stdout_tx,
            stderr_tx,
        };

        let handle = tokio::spawn(async move { imp.wait().await });

        Ok(Child {
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr: Some(stderr),
            handle,
        })
    }

    pub async fn output(self) -> Result<Output, Error> {
        let child = self.spawn().await?;
        Ok(child.wait_with_output().await?)
    }
}
