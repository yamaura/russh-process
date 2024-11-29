use russh::client;
use russh_process::Command;
use std::sync::Arc;
use std::time::Duration;

struct Client {}

#[async_trait::async_trait]
impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv = std::env::args().collect::<Vec<String>>();
    let server = argv.get(1).expect("server address is required");
    let username = argv.get(2).expect("username is required");
    let password = argv.get(3).expect("password is required");
    let command = argv.get(4).expect("command is required");

    let config = client::Config {
        inactivity_timeout: Some(Duration::from_secs(5)),
        ..<_>::default()
    };
    let config = Arc::new(config);

    let mut ssh = client::connect(config, server, Client {}).await?;
    ssh.authenticate_password(username, password).await?;

    let channel = ssh.channel_open_session().await?;
    let channel = Command::from_channel(channel, command.clone().into_bytes());

    let mut output = channel.spawn().await?;

    let mut stdout = output.stdout.take().unwrap();

    let mut buf = vec![];
    use tokio::io::AsyncReadExt;
    stdout.read_to_end(&mut buf).await?;

    println!("{}", String::from_utf8_lossy(&buf));

    let status = output.wait().await?;

    println!("exit code: {:?}", status.code());
    //println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    //println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}
