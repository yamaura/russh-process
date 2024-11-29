use crate::{Command, Output, Error};
use russh::{Channel, ChannelId, ChannelMsg, client::Msg};
use core::future::Future;

pub trait HandleProcessExt {
    fn channel_open_exec_spawn(&self, command: Vec<u8>) -> impl Future<Output=Result<Command<Msg>, Error>> + Send;
    fn channel_open_exec_output(&self, command: Vec<u8>) -> impl Future<Output=Result<Output, Error>> + Send 
        where Self: Sync
    {
        Box::pin(async move {
            let command = self.channel_open_exec_spawn(command).await?;
            command.output().await
        })
    }
}

impl<H> HandleProcessExt for russh::client::Handle<H>
where H: russh::client::Handler
{
    fn channel_open_exec_spawn(&self, command: Vec<u8>) -> impl Future<Output=Result<Command<Msg>, Error>> + Send {
        async move {
            Ok(Command::from_channel(self.channel_open_session().await?, command))
        }
    }
}
