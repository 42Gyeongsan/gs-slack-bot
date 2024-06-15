use ft_api::{AuthInfo, FtApiToken, FtClient, FtClientReqwestConnector};
use gs_slack_bot::{
    bot_cmd::{BotTask, GsctlCommand, SlackMessageContext, SubCommand},
    excutor::{RawCommand, SshExcutor},
};
use slack_morphism::prelude::*;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::Response;
use tracing::*;

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
    println!("Received push event: {:?}", event);

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
                    println!("{real_name}");
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
            let output = match GsctlCommand::from(&task.message_context, ft_client).await {
                GsctlCommand::Reboot(location) => {
                    SshExcutor::new_ansible()
                        .with_port(4222)
                        .with_remote_cmd(RawCommand::build_reboot(&location))
                        .execute()
                        .await
                }
                GsctlCommand::Home(sub) => unimplemented!(),
                GsctlCommand::Goinfre(sub) => unimplemented!(),
                GsctlCommand::Help => unimplemented!(),
                GsctlCommand::Error(msg) => todo!(),
            };

            // TODO: send a message according to result of cmd.
            let token = SlackApiToken::new(config_env_var("SLACK_TEST_TOKEN").unwrap().into());
            let session = client.open_session(&token);
            let _ = session
                .chat_post_message(
                    &SlackApiChatPostMessageRequest::new(
                        task.message_context.channel,
                        SlackMessageContent::new()
                            .with_text(format!("Result: {:?}", output.unwrap())),
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
