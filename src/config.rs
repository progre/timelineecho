use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "protocol")]
pub enum Account {
    #[serde(rename = "atproto")]
    #[serde(rename_all = "camelCase")]
    AtProtocol {
        origin: String,
        identifier: String,
        password: String,
    },
    #[serde(rename = "mastodon")]
    #[serde(rename_all = "camelCase")]
    Mastodon {
        origin: String,
        access_token: String,
    },
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
