use ft_api::{FtClient, FtClientReqwestConnector};
use gs_slack_bot::{
    bot_cmd::{BotTask, GsctlCommand, SlackMessageContext},
    excutor::{RawCommand, SshExcutor},
    WAKEUP_WORD,
};
use slack_morphism::prelude::*;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::Response;
use tracing::{debug, *};

use axum::Extension;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::mpsc, task};

async fn test_oauth_install_function(
    resp: SlackOAuthV2AccessTokenResponse,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) {
    println!("{:#?}", resp);
}

async fn test_push_event(
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackPushEvent>,
    Extension(sender): Extension<mpsc::Sender<BotTask>>,
) -> Response<BoxBody<Bytes, Infallible>> {
    // println!("Received push event: {:?}", event);

    match event {
        SlackPushEvent::UrlVerification(url_ver) => {
            Response::new(Full::new(url_ver.challenge.into()).boxed())
        }
        SlackPushEvent::EventCallback(callback) => {
            let token = SlackApiToken::new(config_env_var("SLACK_TEST_TOKEN").unwrap().into());
            let session = _environment.client.open_session(&token);

            if let SlackEventCallbackBody::Message(SlackMessageEvent {
                origin:
                    SlackMessageOrigin {
                        ts,
                        channel: Some(channel),
                        thread_ts,
                        ..
                    },
                content:
                    Some(SlackMessageContent {
                        text: Some(text), ..
                    }),
                sender:
                    SlackMessageSender {
                        user: Some(user),
                        bot_id: None,
                        ..
                    },
                ..
            }) = callback.event
            {
                let user_info = session
                    .users_info(&SlackApiUsersInfoRequest::new(user))
                    .await;

                if let Ok(SlackApiUsersInfoResponse {
                    user:
                        SlackUser {
                            real_name: Some(real_name),
                            flags:
                                SlackUserFlags {
                                    is_admin: Some(is_admin),
                                    ..
                                },
                            ..
                        },
                }) = user_info
                {
                    let bot_cmd = BotTask {
                        message_context: SlackMessageContext {
                            channel,
                            ts,
                            thread_ts,
                            real_name,
                            is_admin,
                            text,
                        },
                    };
                    let _ = sender.send(bot_cmd).await;
                }
            }

            Response::new(Empty::new().boxed())
        }
        _ => Response::new(Empty::new().boxed()),
    }
}

fn test_error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> HttpStatusCode {
    println!("{:#?}", err);

    // Defines what we return Slack server
    HttpStatusCode::BAD_REQUEST
}

async fn server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client: Arc<SlackHyperClient> =
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
        SlackClientEventsListenerEnvironment::new(client.clone())
            .with_error_handler(test_error_handler),
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
            listener.oauth_router("/auth", &oauth_listener_config, test_oauth_install_function),
        )
        .route(
            "/push",
            axum::routing::post(test_push_event)
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
        let client = client.clone();

        task::spawn(async move {
            let token = SlackApiToken::new(config_env_var("SLACK_TEST_TOKEN").unwrap().into());
            let session = client.open_session(&token);
            let _ = session
                .chat_post_message(
                    &SlackApiChatPostMessageRequest::new(
                        task.message_context.channel.clone(),
                        SlackMessageContent::new().with_text(
                            "Please wait while the command is being executed.".to_owned(),
                        ),
                    )
                    .with_thread_ts(task.message_context.ts.clone()),
                )
                .await;

            let result_message = match GsctlCommand::from(&task.message_context, ft_client).await {
                GsctlCommand::Reboot(location) => {
                    let output = SshExcutor::new_ansible()
                        .with_port(4222)
                        .with_remote_cmd(RawCommand::build_reboot(&location))
                        .execute()
                        .await
                        .unwrap();

                    let stdout = String::from_utf8(output.stdout).unwrap_or_default();

                    if output.status.success() {
                        debug!("Reboot done: {stdout}");
                        "Reboot process is done, ready to use.".to_string()
                    } else {
                        debug!("Reboot failed with following error: {stdout}");
                        "Reboot failed.".to_string()
                    }
                }
                GsctlCommand::Home(sub) => unimplemented!(),
                GsctlCommand::Goinfre(sub) => unimplemented!(),
                GsctlCommand::Help => format!(
                    "사용법: {WAKEUP_WORD} [핵심 명령어] [하위 명령어]

    핵심 명령어:
      reboot       시스템을 재부팅합니다.

      home         'home' 디렉토리와 관련된 작업을 관리합니다.
        하위 명령어:
          reset    home 디렉토리 설정을 기본 상태로 재설정합니다.
          close    home과 pc의 연결을 끊습니다.

    일반 옵션:
      -h, --help   이 도움말 메시지를 보여주고 종료합니다.

    예제:
       {WAKEUP_WORD} reboot
       {WAKEUP_WORD} home reset
       {WAKEUP_WORD} goinfre reset

    인식할 수 없는 명령어나 하위 명령어가 제공될 경우 이 도움말이 표시됩니다."
                ),
                GsctlCommand::Error(msg) => {
                    debug!("{WAKEUP_WORD} command error with: {msg}");
                    "An internal server error has occurred. Please contact @Yondoo.".to_string()
                }
            };

            let _ = session
                .chat_post_message(
                    &SlackApiChatPostMessageRequest::new(
                        task.message_context.channel,
                        SlackMessageContent::new().with_text(result_message),
                    )
                    .with_thread_ts(task.message_context.ts),
                )
                .await;
        });
    }
    Ok(())
}

pub fn config_env_var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|e| format!("{}: {}", name, e))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("gs_slack_bot=debug,slack_morphism=debug,gsctl=debug")
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    server().await?;

    Ok(())
}
