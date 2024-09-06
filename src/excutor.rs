use ft_api::{FtHost, FtLoginId};
use rsb_derive::Builder;
use std::io;
use std::process::Output;
use tokio::process::Command;
use tracing::*;

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
    pub fn build_pc_reboot(location_hostname: &'a FtHost) -> Self {
        RawCommand {
            cmd: "ansible-playbook",
            args: vec!["-l", location_hostname.0.as_str(), "reboot.yml"],
        }
    }

    pub fn build_home_create(login: &'a FtLoginId, url: &'a str, secret: &'a str) -> Self {
        RawCommand {
            cmd: "homemakerctl",
            args: vec![
                "--url",
                url,
                "-t",
                secret,
                "homes",
                "-i",
                login.0.as_str(),
                "create",
            ],
        }
    }

    pub fn build_home_delete(login: &'a FtLoginId, url: &'a str, secret: &'a str) -> Self {
        RawCommand {
            cmd: "homemakerctl",
            args: vec![
                "--url",
                url,
                "-t",
                secret,
                "homes",
                "-i",
                login.0.as_str(),
                "delete",
            ],
        }
    }

    pub fn build_home_close(
        login: &'a FtLoginId,
        location_hostname: &'a String,
        url: &'a str,
        secret: &'a str,
    ) -> Self {
        RawCommand {
            cmd: "homemakerctl",
            args: vec![
                "--url",
                url,
                "-t",
                secret,
                "homes",
                "-i",
                login.0.as_str(),
                "-q",
                location_hostname,
                "-f",
                "close",
            ],
        }
    }

    pub fn into_string(self) -> String {
        format!("{} {}", self.cmd, self.args.join(" ").as_str())
    }
}

impl<'b, 'r> SshExcutor<'b, 'r> {
    pub fn new_ansible_cluster() -> Self {
        SshExcutor::new("ansible@ansiblecluster")
    }

    pub fn new_student_storage() -> Self {
        SshExcutor::new("root@student-storage")
    }

    pub async fn execute(self) -> io::Result<Output> {
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
            let args = format!("sudo su -l root -c \"{}\"", remote_cmd.into_string());
            debug!("{}", args);
            command.arg(args);
        }

        command.output().await
    }
}
