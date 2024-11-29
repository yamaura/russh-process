
use core::pin::Pin;
use core::task::{Context, Poll};
use russh::{Channel, ChannelId, ChannelMsg};
use std::io;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, SimplexStream, WriteHalf};
use tokio::task::JoinHandle;
use crate::{ExitStatus, ExitStatusImp};

/// Represents the standard input (stdin) of a child process.
/// Implements `AsyncWrite` for non-blocking I/O.
#[derive(Debug)]
pub struct ChildStdin {
    pub(crate) inner: WriteHalf<SimplexStream>,
}

impl AsyncWrite for ChildStdin {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

/// Represents the standard output (stdout) of a child process.
/// Implements `AsyncRead` for non-blocking I/O.
#[derive(Debug)]
pub struct ChildStdout {
    pub(crate) inner: ReadHalf<SimplexStream>,
}

impl AsyncRead for ChildStdout {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

/// Represents the standard error (stderr) of a child process.
/// Implements `AsyncRead` for non-blocking I/O.
#[derive(Debug)]
pub struct ChildStderr {
    pub(crate) inner: ReadHalf<SimplexStream>,
}

impl AsyncRead for ChildStderr {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

/// A running child process, providing access to its standard I/O streams
/// and the ability to wait for its completion.
#[derive(Debug)]
pub struct Child {
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
    pub(crate) handle: JoinHandle<Result<ExitStatus, io::Error>>,
}

#[derive(Debug)]
pub(crate) struct ChildImp<S>
where
    S: From<(ChannelId, ChannelMsg)> + Send + Sync + 'static,
{
    pub(crate) channel: Channel<S>,
    pub(crate) stdin_rx: ReadHalf<SimplexStream>,
    pub(crate) stdout_tx: WriteHalf<SimplexStream>,
    pub(crate) stderr_tx: WriteHalf<SimplexStream>,
}

impl<S> ChildImp<S>
where
    S: From<(ChannelId, ChannelMsg)> + Send + Sync + 'static,
{
    pub async fn wait(mut self) -> Result<ExitStatus, io::Error> {
        use tokio::io::AsyncWriteExt;

        let mut code = ExitStatusImp::Processing;

        let mut writer = self.channel.make_writer_ext(None);
        let mut stdin_rx = self.stdin_rx;
        tokio::spawn(async move {
            let _ = tokio::io::copy(&mut stdin_rx, &mut writer).await; // TODO: handle error
        });

        loop {
            let Some(msg) = self.channel.wait().await else {
                break;
            };
            match msg {
                ChannelMsg::ExitStatus { exit_status } => {
                    // Do not return here, we need to read all the data
                    code = ExitStatusImp::Code(exit_status);
                }
                ChannelMsg::Data { ref data } => {
                    self.stdout_tx.write_all(data).await?;
                }
                ChannelMsg::ExtendedData { ref data, ext: 1 } => {
                    self.stderr_tx.write_all(data).await?;
                }
                _ => {}
            }
        }
        tokio::try_join!(self.stdout_tx.shutdown(), self.stderr_tx.shutdown())?;
        Ok(ExitStatus { inner: code })
    }
}

impl Child {
    /// Waits for the child process to exit, returning the exit status.
    pub async fn wait(self) -> Result<ExitStatus, io::Error> {
        self.handle.await?
    }

    /// Waits for the child process to exit, returning the exit status, stdout, and stderr.
    pub async fn wait_with_output(mut self) -> Result<Output, io::Error> {
        async fn read_to_end<A: AsyncRead + Unpin>(io: &mut Option<A>) -> io::Result<Vec<u8>> {
            use tokio::io::AsyncReadExt;
            let mut vec = Vec::new();
            if let Some(io) = io.as_mut() {
                io.read_to_end(&mut vec).await?;
            }
            Ok(vec)
        }

        let mut stdout_pipe = self.stdout.take();
        let mut stderr_pipe = self.stderr.take();

        let stdout_fut = read_to_end(&mut stdout_pipe);
        let stderr_fut = read_to_end(&mut stderr_pipe);

        let (status, stdout, stderr) = tokio::try_join!(self.wait(), stdout_fut, stderr_fut)?;

        drop(stdout_pipe);
        drop(stderr_pipe);

        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}

/// The result of a completed command, including exit status,
/// standard output, and standard error.
#[derive(Debug)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
