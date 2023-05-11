use crate::store;

fn destination_statuses<'a>(
    users: &'a [store::user::User],
    src_origin: &str,
    dst_origin: &str,
) -> Vec<&'a store::user::DestinationStatus> {
    users
        .iter()
        .filter(|user| user.src.origin == src_origin)
        .flat_map(|user| &user.dsts)
        .filter(|dst| dst.origin == dst_origin)
        .flat_map(|dst| &dst.statuses)
        .collect()
}

pub fn find_post_dst_identifier<'a>(
    users: &'a [store::user::User],
    src_origin: &str,
    src_identifier: &str,
    dst_origin: &str,
) -> Option<&'a str> {
    Some(
        destination_statuses(users, src_origin, dst_origin)
            .iter()
            .filter_map(|dst_status| match dst_status {
                store::user::DestinationStatus::Post(post) => Some(post),
                store::user::DestinationStatus::Repost(_) => None,
            })
            .find(|dst_post| dst_post.src_identifier == src_identifier)?
            .identifier
            .as_str(),
    )
}

pub fn find_repost_dst_identifier<'a>(
    users: &'a [store::user::User],
    src_origin: &str,
    src_identifier: &str,
    dst_origin: &str,
) -> Option<&'a str> {
    Some(
        destination_statuses(users, src_origin, dst_origin)
            .iter()
            .filter_map(|dst_status| match dst_status {
                store::user::DestinationStatus::Post(_) => None,
                store::user::DestinationStatus::Repost(repost) => Some(repost),
            })
            .find(|dst_post| dst_post.src_identifier == src_identifier)?
            .identifier
            .as_str(),
    )
}
