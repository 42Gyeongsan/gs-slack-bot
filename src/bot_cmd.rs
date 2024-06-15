use slack_morphism::prelude::*;
use std::{collections::VecDeque, vec};

#[derive(Debug)]
pub struct BotTask {
    pub message_context: Option<SlackMessageContext>,
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
pub struct RemoteCommand {
    pub tokens: vec::IntoIter<Token>,
}

impl RemoteCommand {
    pub fn parse(text: String) -> Self {
        let mut split: VecDeque<&str> = text.split(' ').collect();
        let mut vec = Vec::with_capacity(split.len());

        for _ in 0..split.len() {
            vec.push(Token::parse(split.pop_front().unwrap()).unwrap())
        }

        Self {
            tokens: vec.into_iter(),
        }
    }
}

#[derive(Debug)]
pub enum Token {
    Command(String),
    Flag(String),
}

impl Token {
    pub fn parse(str: &str) -> Result<Self, Error> {
        match str.chars().next() {
            Some('-') => Ok(Token::Flag(str.to_string())),
            Some(_) => Ok(Token::Command(str.to_string())),
            None => Err(str.to_string().into()),
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
