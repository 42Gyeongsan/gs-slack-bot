use ft_api::FtLoginId;
use serde::{Deserialize, Serialize};
use slack_morphism::SlackUserId;

#[derive(Debug, Serialize, Deserialize)]
struct FtSlackIdLogin {
    slack_id: SlackUserId,
    login: FtLoginId,
}

#[derive(Debug, Serialize, Deserialize)]
struct SlackUserList {
    vec: Vec<FtSlackIdLogin>,
}
