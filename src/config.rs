use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "protocol")]
pub enum Account {
    #[serde(rename = "mastodon")]
    #[serde(rename_all = "camelCase")]
    Mastodon {
        origin: String,
        access_token: String,
    },
    #[serde(rename = "atproto")]
    #[serde(rename_all = "camelCase")]
    AtProtocol {
        origin: String,
        identifier: String,
        password: String,
    },
}

impl Account {
    pub fn origin(&self) -> &str {
        match self {
            Account::Mastodon {
                origin,
                access_token: _,
            }
            | Account::AtProtocol {
                origin,
                identifier: _,
                password: _,
            } => origin,
        }
    }

    pub fn identifier(&self) -> &str {
        match self {
            Account::Mastodon {
                origin: _,
                access_token: _,
            } => todo!(),
            Account::AtProtocol {
                origin: _,
                identifier,
                password: _,
            } => identifier,
        }
    }
}

#[derive(Deserialize)]
pub struct User {
    pub src: Account,
    pub dsts: Vec<Account>,
}

#[derive(Deserialize)]
pub struct Config {
    pub users: Vec<User>,
}
