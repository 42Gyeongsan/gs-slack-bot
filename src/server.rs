use crate::{
    bot_cmd::{BotTask, GsctlCommand, GsctlError, SubCommand},
    excutor::{RawCommand, SshExcutor},
    handler::*,
    WAKEUP_WORD_FOR_USER,
};
use ft_api::{config_env_var, FtClient, FtClientReqwestConnector};
use slack_morphism::prelude::*;

use tracing::{debug, *};

use axum::Extension;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::mpsc, task};

const DEFAULT_PORT: u16 = 22;

pub async fn run_slack_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let slack_client: Arc<SlackHyperClient> =
        Arc::new(SlackClient::new(SlackClientHyperConnector::new()?));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("Loading server: {}", addr);

    let oauth_listener_config = SlackOAuthListenerConfig::new(
        config_env_var("SLACK_CLIENT_ID")?.into(),
        config_env_var("SLACK_CLIENT_SECRET")?.into(),
        config_env_var("SLACK_BOT_SCOPE")?,
        config_env_var("SLACK_REDIRECT_HOST")?,
    );

    let listener_environment: Arc<SlackHyperListenerEnvironment> = Arc::new(
        SlackClientEventsListenerEnvironment::new(slack_client.clone())
            .with_error_handler(error_handler),
    );
    let signing_secret: SlackSigningSecret = config_env_var("SLACK_SIGNING_SECRET")?.into();

    let listener: SlackEventsAxumListener<SlackHyperHttpsConnector> =
        SlackEventsAxumListener::new(listener_environment.clone());

    let (sender, mut receiver) = mpsc::channel::<BotTask>(32);

    let ft_client = Arc::new(FtClient::new(FtClientReqwestConnector::with_connector(
        reqwest::Client::new(),
    )));

    // build our application route with OAuth nested router and Push/Command/Interaction events
    let app = axum::routing::Router::new()
        .nest(
            "/auth",
            listener.oauth_router("/auth", &oauth_listener_config, oauth_install_function),
        )
        .route(
            "/push",
            axum::routing::post(push_event)
                .layer(Extension(sender))
                .layer(
                    listener
                        .events_layer(&signing_secret)
                        .with_event_extractor(SlackEventsExtractors::push_event()),
                ),
        );

    task::spawn(async move {
        axum::serve(TcpListener::bind(&addr).await.unwrap(), app)
            .await
            .unwrap();
    });

    while let Some(task) = receiver.recv().await {
        let ft_client = ft_client.clone();
        let slack_client = slack_client.clone();

        task::spawn(async move {
            let token = SlackApiToken::new(config_env_var("SLACK_TOKEN").unwrap().into());
            let session = slack_client.open_session(&token);

            let result = match GsctlCommand::from(&task.message_context, ft_client).await {
                Ok(command) => {
                    let _ = session
                        .reactions_add(&SlackApiReactionsAddRequest::new(
                            task.message_context.channel.clone(),
                            SlackReactionName::new("gsroot-loading".to_owned()),
                            task.message_context.ts.clone(),
                        ))
                        .await;

                    match command {
                        GsctlCommand::Reboot(location) => {
                            let port: u16 = config_env_var("ANSIBLE_CLUSTER_SSH_PORT")
                                .ok()
                                .and_then(|port| port.parse().ok())
                                .unwrap_or(DEFAULT_PORT);
                            let output = SshExcutor::new_ansible_cluster()
                                .with_port(port)
                                .with_remote_cmd(RawCommand::build_pc_reboot(&location))
                                .execute()
                                .await
                                .unwrap();

                            let stdout = String::from_utf8(output.stdout).unwrap_or_default();

                            if output.status.success() {
                                debug!("Reboot done: {stdout}");
                                Ok(None)
                            } else {
                                debug!("Reboot failed with following error: {stdout}");
                                Err(Some("Reboot failed.".to_string()))
                            }
                        }
                        GsctlCommand::Home(subcommand) => {
                            if let Some(subcmd) = subcommand {
                                let host_url = config_env_var("STUDENT_STORAGE_API_URL").unwrap();
                                let secret_token =
                                    config_env_var("HOMEMAKER_SECRET_TOKEN").unwrap();
                                let port: u16 = config_env_var("STUDENT_STORAGE_SSH_PORT")
                                    .ok()
                                    .and_then(|port| port.parse().ok())
                                    .unwrap_or(DEFAULT_PORT);
                                match subcmd {
                                    SubCommand::Reset(login) => {
                                        let delete_output = SshExcutor::new_student_storage()
                                            .with_port(port)
                                            .with_remote_cmd(RawCommand::build_home_delete(
                                                &login.clone(),
                                                &host_url,
                                                &secret_token,
                                            ))
                                            .execute()
                                            .await
                                            .unwrap();

                                        let create_output = SshExcutor::new_student_storage()
                                            .with_port(port)
                                            .with_remote_cmd(RawCommand::build_home_create(
                                                &login.clone(),
                                                &host_url,
                                                &secret_token,
                                            ))
                                            .execute()
                                            .await
                                            .unwrap();

                                        if delete_output.status.success() {
                                            let create_stdout =
                                                String::from_utf8(create_output.stdout)
                                                    .unwrap_or_default();
                                            let delete_stdout =
                                                String::from_utf8(delete_output.stdout)
                                                    .unwrap_or_default();
                                            debug!("Home reset done.\ndelete:[{delete_stdout}]\ncreate:[{create_stdout}]");
                                            Ok(None)
                                        } else {
                                            let create_stderr =
                                                String::from_utf8(create_output.stderr)
                                                    .unwrap_or_default();
                                            let delete_stderr =
                                                String::from_utf8(delete_output.stderr)
                                                    .unwrap_or_default();
                                            debug!(
                                                "Home reset failed with following error: delete: {delete_stderr}, create: {create_stderr}"
                                            );
                                            Err(Some(
                                                "Home reset failed. please contact staff"
                                                    .to_string(),
                                            ))
                                        }
                                    }
                                    SubCommand::Close(login, location) => {
                                        let output = SshExcutor::new_student_storage()
                                            .with_remote_cmd(RawCommand::build_home_close(
                                                &login,
                                                &location,
                                                &host_url,
                                                &secret_token,
                                            ))
                                            .execute()
                                            .await
                                            .unwrap();

                                        let stdout =
                                            String::from_utf8(output.stdout).unwrap_or_default();

                                        if output.status.success() {
                                            debug!(
                                                "Home close on {location}, login: {login} done: {stdout}"
                                            );
                                            Ok(None)
                                        } else {
                                            debug!(
                                                "Home close on {location}, login: {login} failed with: {stdout}."
                                            );
                                            Err(Some("Home close failed.".to_string()))
                                        }
                                    }
                                    SubCommand::Help => todo!(),
                                }
                            } else {
                                Err(None)
                            }
                        }
                        GsctlCommand::Goinfre(subcommand) => unimplemented!(),
                        GsctlCommand::Update => {
                            let res = session
                                .conversations_members(
                                    &SlackApiConversationsMembersRequest::new()
                                        .with_limit(200)
                                        .with_channel(task.message_context.channel.clone()),
                                )
                                .await;

                            if res.is_ok() {
                                let members = res.unwrap().members;
                                let mut member_nickname_pair = vec![];

                                for member in members {
                                    let info = session
                                        .users_info(&SlackApiUsersInfoRequest::new(member.clone()))
                                        .await;

                                    if let SlackApiUsersInfoResponse {
                                        user:
                                            SlackUser {
                                                real_name: Some(name),
                                                ..
                                            },
                                    } = info.unwrap()
                                    {
                                        member_nickname_pair.push((member, name))
                                    }
                                }

                                Ok(None)
                            } else {
                                Err(Some("get member list failed.".to_string()))
                            }
                        }
                    }
                }
                Err(error) => match error {
                    GsctlError::Help => Err(Some(format!(
                        "```사용법: {WAKEUP_WORD_FOR_USER} [핵심 명령어] [하위 명령어]

핵심 명령어:
  reboot       시스템을 재부팅합니다.

  home         'home' 디렉토리와 관련된 작업을 관리합니다.
    하위 명령어:
      reset    home을 기본 상태로 재설정합니다.
      close    remote home과 pc의 연결을 끊습니다.

일반 옵션:
  -h, --help   이 도움말 메시지를 보여주고 종료합니다.

예제:
   {WAKEUP_WORD_FOR_USER} reboot
   {WAKEUP_WORD_FOR_USER} home reset

인식할 수 없는 명령어나 하위 명령어가 제공될 경우 이 도움말이 표시됩니다.```"
                    ))),
                    GsctlError::Error(msg) => {
                        let command = task.message_context.text.clone();
                        debug!("{} command error with: {msg}", command);
                        Err(Some(format!(
                            "Command cannot be executed for the following reasons: {msg}"
                        )))
                    }
                    GsctlError::NotACommand => Err(None),
                },
            };

            match result {
                Ok(res) => {
                    let _ = session
                        .reactions_remove(
                            &SlackApiReactionsRemoveRequest::new(SlackReactionName::new(
                                "gsroot-loading".to_owned(),
                            ))
                            .with_channel(task.message_context.channel.clone())
                            .with_timestamp(task.message_context.ts.clone()),
                        )
                        .await;
                    let _ = session
                        .reactions_add(&SlackApiReactionsAddRequest::new(
                            task.message_context.channel.clone(),
                            SlackReactionName::new("white_check_mark".to_owned()),
                            task.message_context.ts.clone(),
                        ))
                        .await;

                    if let Some(msg) = res {
                        let _ = session
                            .chat_post_message(
                                &SlackApiChatPostMessageRequest::new(
                                    task.message_context.channel,
                                    SlackMessageContent::new().with_text(msg),
                                )
                                .with_thread_ts(task.message_context.ts),
                            )
                            .await;
                    }
                }
                Err(Some(msg)) => {
                    let _ = session
                        .reactions_remove(
                            &SlackApiReactionsRemoveRequest::new(SlackReactionName::new(
                                "gsroot-loading".to_owned(),
                            ))
                            .with_channel(task.message_context.channel.clone())
                            .with_timestamp(task.message_context.ts.clone()),
                        )
                        .await;
                    let _ = session
                        .reactions_add(&SlackApiReactionsAddRequest::new(
                            task.message_context.channel.clone(),
                            SlackReactionName::new("x".to_owned()),
                            task.message_context.ts.clone(),
                        ))
                        .await;

                    let _ = session
                        .chat_post_message(
                            &SlackApiChatPostMessageRequest::new(
                                task.message_context.channel,
                                SlackMessageContent::new().with_text(msg),
                            )
                            .with_thread_ts(task.message_context.ts),
                        )
                        .await;
                }
                Err(None) => {}
            }
        });
    }
    Ok(())
}
