use std::{sync::Arc, time::Duration};

use reqwest::Client;
use serenity::{
    http::Http, model::prelude::application_command::ApplicationCommandInteraction, prelude::*,
};

use crate::errors::ParrotError;

static URL: &str = "https://api.warframestat.us/pc/";

struct Api {
    fissures: Vec<Fissure>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fissure {
    id: String,
    activation: String,
    start_string: String,
    expiry: String,
    active: bool,
    node: String,
    mission_type: String,
    mission_key: String,
    enemy: String,
    enemy_key: String,
    node_key: String,
    tier: String,
    tier_num: i64,
    expired: bool,
    eta: String,
    is_storm: bool,
    is_hard: bool,
}

pub async fn cycle(http: Arc<Http>) {
    async fn inner(
        http: Arc<Http>,
        client: &Client,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    let client = Client::new();

    loop {
        tokio::time::sleep(Duration::new(60, 0)).await;

        if let Err(err) = inner(http.clone(), &client).await {}
    }
}

pub async fn alert_fissure(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    Ok(())
}
