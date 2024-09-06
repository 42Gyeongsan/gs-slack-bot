use ft_api::config_env_var;
use slack_morphism::prelude::*;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::Response;

use axum::Extension;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::debug;

use crate::bot_cmd::*;

pub async fn oauth_install_function(
    resp: SlackOAuthV2AccessTokenResponse,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) {
    println!("{:#?}", resp);
}

pub async fn push_event(
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackPushEvent>,
    Extension(sender): Extension<mpsc::Sender<BotTask>>,
) -> Response<BoxBody<Bytes, Infallible>> {
    match event {
        SlackPushEvent::UrlVerification(url_ver) => {
            Response::new(Full::new(url_ver.challenge.into()).boxed())
        }
        SlackPushEvent::EventCallback(callback) => {
            let token = SlackApiToken::new(config_env_var("SLACK_TOKEN").unwrap().into());
            let session = _environment.client.open_session(&token);

            if let SlackEventCallbackBody::AppMention(SlackAppMentionEvent {
                user,
                channel,
                content:
                    SlackMessageContent {
                        text: Some(text), ..
                    },
                origin: SlackMessageOrigin { ts, thread_ts, .. },
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
                    debug!("message from user:{real_name}, is_admin:{is_admin}, text:{text}");
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

pub fn error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> HttpStatusCode {
    println!("{:#?}", err);

    // Defines what we return Slack server
    HttpStatusCode::BAD_REQUEST
}
