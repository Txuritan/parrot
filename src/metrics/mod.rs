use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration, env,
};

use hyper::{Server, service::{make_service_fn, service_fn}, Request, Body, Response, header::CONTENT_TYPE};
use once_cell::sync::Lazy;
use prometheus::{
    register_gauge_vec, register_histogram_vec, register_int_counter_vec,
    register_int_gauge_vec, GaugeVec, HistogramTimer, HistogramVec, IntCounterVec,
    IntGaugeVec, TextEncoder, Encoder as _,
};
use serenity::{
    client::bridge::gateway::{ShardId, ShardManager},
    prelude::Context,
};
use tokio::sync::Mutex;

type LatencyMap = HashMap<ShardId, Option<Duration>>;
type ShardSet = HashSet<ShardId>;

static SHARD_WATCH_INTERVAL: Duration = Duration::from_secs(5);

pub static SHARD_COUNT: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!("parrot_shard_count", "Number of active shards", &["shard"]).unwrap()
});

pub static SHARD_LATENCY: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "parrot_shard_latency",
        "Shard latency to Discord",
        &["shard"]
    )
    .unwrap()
});

pub static COMMAND_COUNTER: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "parrot_command_calls",
        "Number of times a command was called",
        &["shard", "command"],
    )
    .unwrap()
});

pub static COMMAND_TIME: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "parrot_command_seconds",
        "Time a command took (measured to high precision)",
        &["shard", "command"],
    )
    .unwrap()
});

#[track_caller]
pub fn record_command(ctx: &Context, name: &str) -> HistogramTimer {
    let values = &[&ctx.shard_id.to_string(), name];

    COMMAND_COUNTER.with_label_values(values).inc();

    COMMAND_TIME.with_label_values(values).start_timer()
}

pub async fn initialize(client: &serenity::Client) {
    let port = env::var("PROMETHEUS_PORT").expect("Fatality! PROMETHEUS_PORT not set!");
    let port = port.parse().expect("Fatality! PROMETHEUS_PORT not set to a valid port!");

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(shard_watcher(shard_manager));

    tokio::spawn(start_server(port));
}

async fn shard_watcher(shard_manager: Arc<Mutex<ShardManager>>) {
    tracing::debug!("starting shards watcher");

    let mut old_shards = ShardSet::new();

    loop {
        let manager_lock = shard_manager.lock().await;
        let shards = manager_lock
            .shards_instantiated()
            .await
            .into_iter()
            .collect();

        // TODO: maybe this should cloned so we don't have to lock the manager
        let info_lock = manager_lock.runners.lock().await;
        let latency: LatencyMap = info_lock
            .iter()
            .map(|(id, info)| (*id, info.latency))
            .collect();

        drop(info_lock);
        drop(manager_lock);

        for id in old_shards.difference(&shards) {
            let id_str = id.to_string();

            // Reset old shards to 0 as they are no longer active
            SHARD_COUNT.with_label_values(&[&id_str]).dec();

            // Reset latency gauge for the missing ones
            SHARD_LATENCY.with_label_values(&[&id_str]).set(0.0);
        }

        // Increment new shards as they where just spawned
        // Theres no need to set existing shards as they are already on
        for id in shards.difference(&old_shards) {
            SHARD_COUNT.with_label_values(&[&id.to_string()]).inc();
        }

        // Update latency for *all* shards
        for (id, latency) in latency {
            if let Some(latency) = latency {
                SHARD_LATENCY
                    .with_label_values(&[&id.to_string()])
                    .set(latency.as_secs_f64());
            }
        }

        old_shards = shards;

        tokio::time::sleep(SHARD_WATCH_INTERVAL).await;
    }
}

async fn start_server(port: u16) {
    let addr = ([127, 0, 0, 1], port).into();

    let serve_future = Server::bind(&addr).serve(make_service_fn(|_| async {
        Ok::<_, hyper::Error>(service_fn(serve_metrics))
    }));

    tracing::debug!(port = %port, "starting metrics server");

    if let Err(err) = serve_future.await {
        tracing::error!(err = ?err, "server error");
    }
}

async fn serve_metrics(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let encoder = TextEncoder::new();

    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let response = Response::builder()
        .status(200)
        .header(CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap();

    Ok(response)
}
