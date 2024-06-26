mod app;
mod config;
mod database;
mod operations;
mod protocols;
mod sources;
mod store;
mod utils;

use tracing_subscriber::{
    fmt::{
        format::{DefaultFields, FmtSpan, Format, Full, Writer},
        time::FormatTime,
        SubscriberBuilder,
    },
    EnvFilter,
};

struct NoTime;

impl FormatTime for NoTime {
    fn format_time(&self, _writer: &mut Writer<'_>) -> std::fmt::Result {
        Ok(())
    }
}

fn default_subscriber_builder(
    log_level: &str,
) -> SubscriberBuilder<DefaultFields, Format<Full, NoTime>, EnvFilter> {
    let s = format!("timelineecho={},reqwest={}", log_level, log_level);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from(&s)))
        .with_span_events(FmtSpan::CLOSE)
        // no timestamp, with elapsed
        .with_timer(NoTime)
}

#[cfg(not(target_os = "linux"))]
mod local {
    use std::num::NonZeroU8;

    use anyhow::Result;
    use time::format_description::well_known::{
        iso8601::{self, EncodedConfig},
        Iso8601,
    };
    use tracing_subscriber::fmt::time::LocalTime;

    use crate::{app::app, database, default_subscriber_builder};

    pub fn init_tracing() {
        const MY_CONFIG: EncodedConfig = iso8601::Config::DEFAULT
            .set_time_precision(iso8601::TimePrecision::Second {
                decimal_digits: NonZeroU8::new(6),
            })
            .encode();
        default_subscriber_builder("trace")
            .with_timer(LocalTime::new(Iso8601::<MY_CONFIG>))
            .compact()
            .init();
    }

    pub async fn main() -> Result<()> {
        init_tracing();

        app(database::File).await
    }
}

mod lambda {
    use aws_lambda_events::event::cloudwatch_events::CloudWatchEvent;
    use lambda_runtime::{run, service_fn, LambdaEvent};

    use crate::{app::app, database, default_subscriber_builder};

    pub fn init_tracing() {
        default_subscriber_builder("debug")
            .with_ansi(false)
            .with_target(false)
            .init();
    }

    pub async fn function_handler(
        _event: LambdaEvent<CloudWatchEvent>,
    ) -> Result<(), lambda_runtime::Error> {
        if let Err(err) = app(database::DynamoDB::new().await).await {
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
