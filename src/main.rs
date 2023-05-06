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
mod database;
mod destination;
mod protocols;
mod sources;
mod store;

#[cfg(not(target_os = "linux"))]
mod local {
    use std::num::NonZeroU8;

    use anyhow::Result;
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

    use crate::{app::app, database};

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

    pub async fn main() -> Result<()> {
        init_tracing();

        app(&database::File).await
    }
}

mod lambda {
    use aws_lambda_events::event::cloudwatch_events::CloudWatchEvent;
    use lambda_runtime::{run, service_fn, LambdaEvent};
    use tracing_subscriber::EnvFilter;

    use crate::{app::app, database};

    pub fn init_tracing() {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::from("timelineecho=trace,reqwest=trace")),
            )
            .with_ansi(false)
            .with_target(false)
            .without_time()
            .init();
    }

    pub async fn function_handler(
        _event: LambdaEvent<CloudWatchEvent>,
    ) -> Result<(), lambda_runtime::Error> {
        if let Err(err) = app(&database::DynamoDB::new().await).await {
            tracing::error!("{:?}", err);
            return Err(err.into());
        }
        Ok(())
    }

    #[allow(unused)]
    pub async fn main() -> Result<(), lambda_runtime::Error> {
        init_tracing();

        run(service_fn(function_handler)).await
    }
}

#[cfg(not(target_os = "linux"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    local::main().await
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    lambda::main().await
}
