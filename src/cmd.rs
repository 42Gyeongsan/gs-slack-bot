use ft_api::FtHost;
use http_body_util::BodyExt;
use rsb_derive::Builder;
use serde::{Deserialize, Serialize};
use std::process::{Command, Output};
use std::{io, os::unix::process::CommandExt};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GstCoreCommand {
    Reboot,
    Home,
    Goinfre,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebootSubCommand {
    Host(String),
    Help,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeSubCommand {
    Reset,
    Close,
    Help,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoinfreSubCommand {
    Reset,
    Help,
}

#[derive(Debug, Builder)]
pub struct SshExcutor<'b, 'r> {
    pub ssh_pub_key: Option<&'b str>,
    address: &'b str,
    pub port: Option<u16>,
    pub remote_cmd: Option<RawCommand<'r>>,
}

#[derive(Debug, Builder)]
pub struct RawCommand<'a> {
    cmd: &'a str,
    args: Vec<&'a str>,
}

impl<'a> RawCommand<'a> {
    pub fn build_reboot(location_hostname: &'a FtHost) -> Self {
        RawCommand {
            cmd: "ansible-playbook",
            args: vec!["-l", location_hostname.0.as_str(), "reboot.yml"],
        }
    }

    pub fn into_string(self) -> String {
        format!("{} {}", self.cmd, self.args.join(" ").as_str())
    }
}

impl<'b, 'r> SshExcutor<'b, 'r> {
    pub fn new_ansible() -> Self {
        SshExcutor::new("ansible@ansiblecluster")
    }

    pub fn execute(self) -> io::Result<Output> {
        let mut command = Command::new("ssh");

        if let Some(key) = self.ssh_pub_key {
            command.arg("-i").arg(key);
        }

        if let Some(port) = self.port {
            command.arg("-p").arg(port.to_string());
        }

        if let Some(key) = self.ssh_pub_key {
            command.arg("-i").arg(key);
        }

        command.arg(self.address);

        if let Some(remote_cmd) = self.remote_cmd {
            command.arg(format!("sudo su -l root -c '{}'", remote_cmd.into_string()));
        }

        command.output()
    }
}

#[test]
fn excute_ansible() {
    let location_hostname = FtHost::new("c1r2s3".into());
    let ansible = SshExcutor::new_ansible()
        .with_port(4222)
        .with_remote_cmd(RawCommand::build_reboot(&location_hostname));

    let output = ansible.execute();

    assert!(output.is_ok());
}
