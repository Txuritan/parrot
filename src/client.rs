use std::{collections::HashMap, env, error::Error};

use serenity::model::gateway::GatewayIntents;
use songbird::serenity::SerenityInit;

use crate::{
    commands::roll::RerollTable,
    guild::{cache::GuildCacheMap, settings::GuildSettingsMap},
    handlers::SerenityHandler,
    metrics,
};

pub struct Client {
    client: serenity::Client,
}

impl Client {
    pub async fn default() -> Result<Client, Box<dyn Error>> {
        let token = env::var("DISCORD_TOKEN").expect("Fatality! DISCORD_TOKEN not set!");
        Client::new(token).await
    }

    pub async fn new(token: String) -> Result<Client, Box<dyn Error>> {
        let application_id = env::var("DISCORD_APP_ID")
            .expect("Fatality! DISCORD_APP_ID not set!")
            .parse()?;

        let gateway_intents = GatewayIntents::non_privileged();

        let client = serenity::Client::builder(token, gateway_intents)
            .event_handler(SerenityHandler)
            .application_id(application_id)
            .register_songbird()
            .await?;

        metrics::initialize(&client).await;

        let mut data = client.data.write().await;
        data.insert::<GuildCacheMap>(HashMap::default());
        data.insert::<GuildSettingsMap>(HashMap::default());
        data.insert::<RerollTable>(HashMap::default());
        drop(data);

        Ok(Client { client })
    }

    pub async fn start(&mut self) -> Result<(), serenity::Error> {
        tokio::task::spawn(crate::commands::cetus::cycle(
            self.client.cache_and_http.http.clone(),
        ));

        self.client.start_autosharded().await
    }
}
