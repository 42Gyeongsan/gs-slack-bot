pub mod bot_cmd;
pub mod excutor;
pub mod handler;
pub mod server;
pub mod user;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub const WAKEUP_WORD: &str = "YourSlackBotInternalIdHere";
pub const WAKEUP_WORD_FOR_USER: &str = "@YourSlackBotNameHere";
