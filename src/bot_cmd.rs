use ft_api::{
    locations::{self, FtApiCampusLocationsRequest},
    AuthInfo, FtApiToken, FtCampusId, FtClient, FtClientReqwestConnector, FtFilterField,
    FtFilterOption, GS_CAMPUS_ID,
};
use regex::Regex;
use slack_morphism::prelude::*;
use std::{collections::VecDeque, sync::Arc, vec};

#[derive(Debug)]
pub struct BotTask {
    pub message_context: SlackMessageContext,
}

#[derive(Debug)]
pub enum Error {
    InvalidCommand(crate::Error),
}

impl From<String> for Error {
    fn from(src: String) -> Error {
        Error::InvalidCommand(src.into())
    }
}

#[derive(Debug)]
pub enum SubCommand {
    Reset,
    Close,
    Help,
}

#[derive(Debug)]
pub enum GsctlCommand {
    Reboot(ft_api::FtHost),
    Home(Option<SubCommand>),
    Goinfre(Option<SubCommand>),
    Help,
}

fn check_hostname(raw_text: &str) -> bool {
    let re = Regex::new(r"c[1-3]{1}r\d{1,2}s\d{1,2}").unwrap();

    re.is_match(raw_text)
}

impl GsctlCommand {
    pub async fn from(
        context: &SlackMessageContext,
        ft_client: Arc<FtClient<FtClientReqwestConnector>>,
    ) -> Self {
        let mut token = context.text.split_whitespace();

        match token.next() {
            Some(subcommand) => match subcommand {
                "reboot" => {
                    let location = match token.next() {
                        Some(location) if check_hostname(location) => {
                            ft_api::FtHost(location.to_string())
                        }
                        Some(_) => return GsctlCommand::Help,
                        None => {
                            let info = AuthInfo::build_from_env().unwrap();
                            let token = FtApiToken::try_get(info).await.unwrap();
                            let session = ft_client.open_session(&token);

                            let res = session
                                .campus_id_locations(
                                    FtApiCampusLocationsRequest::new(FtCampusId::new(GS_CAMPUS_ID))
                                        .with_filter(vec![FtFilterOption::new(
                                            FtFilterField::Active,
                                            vec!["true".to_string()],
                                        )]),
                                )
                                .await
                                .unwrap();

                            res.location
                                .into_iter()
                                .find(|lo| {
                                    if let Some(name) = &lo.user.login {
                                        name.to_string() == context.real_name
                                    } else {
                                        false
                                    }
                                })
                                .unwrap()
                                .host
                        }
                    };

                    GsctlCommand::Reboot(location)
                }
                "home" => {
                    let subcommand = match token.next() {
                        Some("reset") => Some(SubCommand::Reset),
                        Some("close") => Some(SubCommand::Close),
                        _ => Some(SubCommand::Help),
                    };
                    GsctlCommand::Home(subcommand)
                }
                "goinfre" => {
                    let subcommand = match token.next() {
                        Some("reset") => Some(SubCommand::Reset),
                        _ => Some(SubCommand::Help),
                    };
                    GsctlCommand::Goinfre(subcommand)
                }
                _ => Self::Help,
            },
            None => Self::Help,
        }
    }
}

#[derive(Debug)]
pub struct SlackMessageContext {
    pub channel: SlackChannelId,
    pub ts: SlackTs,
    pub thread_ts: Option<SlackTs>,
    pub real_name: String,
    pub is_admin: bool,
    pub text: String,
}
