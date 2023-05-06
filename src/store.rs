pub mod operation;
pub mod user;

use serde::{Deserialize, Serialize};

use self::user::{Destination, Source, User};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub users: Vec<User>,
}

impl Store {
    pub fn get_or_create_user<'a>(&'a mut self, origin: &str, identifier: &str) -> &'a mut User {
        let idx = self
            .users
            .iter()
            .position(|user| user.src.origin == origin && user.src.identifier == identifier);
        if let Some(idx) = idx {
            return &mut self.users[idx];
        }
        self.users.push(User {
            src: Source {
                origin: origin.to_owned(),
                identifier: identifier.to_owned(),
                statuses: Vec::default(),
            },
            dsts: Vec::default(),
        });
        self.users.last_mut().unwrap()
    }

    pub fn get_or_create_dst<'a>(
        &'a mut self,
        account_pair: &operation::AccountPair,
    ) -> &'a mut Destination {
        self.get_or_create_user(
            &account_pair.src_origin,
            &account_pair.src_account_identifier,
        )
        .get_or_create_dst(&account_pair.to_dst_key())
    }
}
