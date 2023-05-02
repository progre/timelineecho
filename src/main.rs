#![warn(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::single_match_else)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unreadable_literal)]

mod app;
mod config;
mod destination;
mod protocols;
mod source;
mod store;

use std::num::NonZeroU8;

use anyhow::Result;
use app::app;
use time::format_description::well_known::{
    iso8601::{self, EncodedConfig},
    Iso8601,
};
use tracing_subscriber::{
    fmt::{self, time::LocalTime},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_tracing() {
    const MY_CONFIG: EncodedConfig = iso8601::Config::DEFAULT
        .set_time_precision(iso8601::TimePrecision::Second {
            decimal_digits: NonZeroU8::new(6),
        })
        .encode();
    let fmt = Iso8601::<MY_CONFIG>;
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::from("timelineecho=trace,reqwest=trace")),
        )
        .with(fmt::layer().with_timer(LocalTime::new(fmt)).compact())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    app().await
}
