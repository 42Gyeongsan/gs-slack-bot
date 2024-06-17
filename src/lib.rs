pub mod bot_cmd;
pub mod excutor;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub const WAKEUP_WORD: &str = "groot";
