use ft_api::{
    locations::FtApiCampusLocationsRequest, AuthInfo, FtApiToken, FtCampusId, FtClient,
    FtClientReqwestConnector, FtFilterField, FtFilterOption, GS_CAMPUS_ID,
};
use regex::Regex;
use slack_morphism::prelude::*;
use std::sync::Arc;

use crate::WAKEUP_WORD;

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
    Reset(ft_api::FtLoginId),
    Close(ft_api::FtLoginId, String),
    Help,
}

#[derive(Debug)]
pub enum GsctlCommand {
    Reboot(ft_api::FtHost),
    Home(Option<SubCommand>),
    Goinfre(Option<SubCommand>),
}

#[derive(Debug)]
pub enum GsctlError {
    Help,
    NotACommand,
    Error(String),
}

fn check_hostname(raw_text: &str) -> bool {
    let re = Regex::new(r"c[1-3]{1}(r\d{1,2})?(s\d{1,2})?").unwrap();

    re.is_match(raw_text)
}

impl GsctlCommand {
    pub async fn from(
        context: &SlackMessageContext,
        ft_client: Arc<FtClient<FtClientReqwestConnector>>,
    ) -> Result<Self, GsctlError> {
        let mut token = context.text.split_whitespace();

        if let Some(WAKEUP_WORD) = token.next() {
            match token.next() {
                Some(subcommand) => {
                    match subcommand {
                        "reboot" => {
                            let location = match token.next() {
                                Some(location) if check_hostname(location) && context.is_admin => {
                                    ft_api::FtHost(location.to_string())
                                }
                                Some(_) => return Err(GsctlError::Help),
                                None => {
                                    let info = AuthInfo::build_from_env().unwrap();
                                    let token = FtApiToken::try_get(info).await.unwrap();
                                    let session = ft_client.open_session(&token);

                                    let res = session
                                        .campus_id_locations(
                                            FtApiCampusLocationsRequest::new(FtCampusId::new(
                                                GS_CAMPUS_ID,
                                            ))
                                            .with_filter(vec![FtFilterOption::new(
                                                FtFilterField::Active,
                                                vec!["true".to_string()],
                                            )]),
                                        )
                                        .await
                                        .unwrap();

                                    match res.location.into_iter().find(|lo| {
                                        if let Some(name) = &lo.user.login {
                                            name.to_string() == context.real_name
                                        } else {
                                            false
                                        }
                                    }) {
                                        Some(location) => location.host,
                                        None => {
                                            return Err(GsctlError::Error(
                                                "Location not found!".to_string(),
                                            ))
                                        }
                                    }
                                }
                            };
                            Ok(GsctlCommand::Reboot(location))
                        }
                        "home" => {
                            let subcommand = match token.next() {
                                Some("reset") => Some(SubCommand::Reset(ft_api::FtLoginId(
                                    context.real_name.clone(),
                                ))),
                                Some("close") => {
                                    let info = AuthInfo::build_from_env().unwrap();
                                    let token = FtApiToken::try_get(info).await.unwrap();
                                    let session = ft_client.open_session(&token);

                                    let res = session
                                        .campus_id_locations(
                                            FtApiCampusLocationsRequest::new(FtCampusId::new(
                                                GS_CAMPUS_ID,
                                            ))
                                            .with_filter(vec![FtFilterOption::new(
                                                FtFilterField::Active,
                                                vec!["true".to_string()],
                                            )]),
                                        )
                                        .await
                                        .unwrap();

                                    let location = match res.location.into_iter().find(|lo| {
                                        if let Some(name) = &lo.user.login {
                                            name.to_string() == context.real_name
                                        } else {
                                            false
                                        }
                                    }) {
                                        Some(location) => format!("iqn.fr.42:{}", location.host),
                                        None => {
                                            return Err(GsctlError::Error(
                                                "location not found!".to_string(),
                                            ))
                                        }
                                    };

                                    Some(SubCommand::Close(
                                        ft_api::FtLoginId(context.real_name.clone()),
                                        location,
                                    ))
                                }
                                _ => Some(SubCommand::Help),
                            };
                            Ok(GsctlCommand::Home(subcommand))
                        }
                        "goinfre" => {
                            let subcommand = match token.next() {
                                Some("reset") => Some(SubCommand::Reset(ft_api::FtLoginId(
                                    context.real_name.clone(),
                                ))),
                                _ => Some(SubCommand::Help),
                            };
                            Ok(GsctlCommand::Goinfre(subcommand))
                        }
                        _ => Err(GsctlError::Help),
                    }
                }
                None => Err(GsctlError::Help),
            }
        } else {
            Err(GsctlError::NotACommand)
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
