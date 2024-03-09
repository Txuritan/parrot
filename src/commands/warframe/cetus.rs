use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use itertools::Itertools as _;
use once_cell::sync::Lazy;
use rand::prelude::*;
use regex::Regex;
use reqwest::Client;
use serenity::{
    http::Http,
    model::prelude::{
        application_command::ApplicationCommandInteraction, ChannelId, Mention, UserId,
    },
    prelude::Context,
};

use crate::{errors::ParrotError, messaging::messages};

static URL: &str = "https://api.warframestat.us/pc/cetusCycle/";

static TIME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([0-9]{2}|[0-9]{1}):([0-9]{2})").unwrap());
static DAY_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"((1)h )?([0-9]{1}|[0-9]{2})m to Night").unwrap());
static NIGHT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"([0-9]{1}|[0-9]{2})m to Day").unwrap());

#[derive(Default)]
struct EidolonTable {
    channel: Option<ChannelId>,
    leaders: Vec<UserId>,
    users: Vec<UserId>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Cetus {
    id: String,
    expiry: String,
    activation: String,
    #[serde(rename = "isDay")]
    day: bool,
    #[serde(rename = "isCetus")]
    cetus: bool,
    state: String,
    #[serde(rename = "timeLeft")]
    left: String,
    #[serde(rename = "shortString")]
    short: String,
}

impl ::std::fmt::Display for Cetus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "(day: {}, time: {})", self.day, self.short)
    }
}

#[derive(Clone, Debug)]
pub enum Api {
    Change(bool),
    Ten,
    Five,
}

fn create_groups(table: &EidolonTable) -> Option<Vec<Vec<UserId>>> {
    let mut leader_chunks = table
        .leaders
        .chunks(1)
        .map(|x| x.to_vec())
        .collect::<Vec<Vec<UserId>>>();

    let mut groups = Vec::new();

    for (i, user_chunk) in table
        .users
        .chunks(if table.users.len() % 2 == 0 { 2 } else { 3 })
        .enumerate()
    {
        if i < leader_chunks.len() {
            match leader_chunks.get_mut(i) {
                Some(group) => {
                    group.append(&mut user_chunk.to_vec());
                    groups.push(group.to_vec());
                }
                None => {
                    return None;
                }
            }
        }
    }

    Some(groups)
}

async fn create_response_cetus<R>(
    http: Arc<Http>,
    channel_id: ChannelId,
    rng: &mut R,
    list: [&'static str; 3],
) -> Result<(), ParrotError>
where
    R: Rng,
{
    let message = list.choose(rng).unwrap();
    let channel = http.get_channel(channel_id.0).await?.guild().unwrap();

    channel
        .send_message(http, |m| {
            m.add_embed(|e| e.description(message.to_string()))
        })
        .await?;

    Ok(())
}

async fn create_response_cetus_groups<R>(
    http: Arc<Http>,
    channel_id: ChannelId,
    rng: &mut R,
    list: [&'static str; 3],
    groups: &[Vec<UserId>],
) -> Result<(), ParrotError>
where
    R: Rng,
{
    let message = list.choose(rng).unwrap();
    let channel = http.get_channel(channel_id.0).await?.guild().unwrap();

    channel
        .send_message(http, |m| {
            m.add_embed(|e| {
                e.title("Groups").description(message);

                for (i, chunk) in groups.iter().enumerate() {
                    let members = chunk.iter().map(|id| Mention::from(*id)).join(", ");

                    e.field(format!("Group {}", i + 1), members, true);
                }

                e
            })
        })
        .await?;

    Ok(())
}

async fn create_team_response<R>(
    http: Arc<Http>,
    table: &EidolonTable,
    rng: &mut R,
    list: [&'static str; 3],
) -> Result<(), ParrotError>
where
    R: Rng,
{
    let channel = match table.channel {
        Some(table) => table,
        None => return Ok(()),
    };

    if table.leaders.is_empty() {
        // Not able to make groups without leaders
        return create_response_cetus(http.clone(), channel, rng, messages::NO_GROUPS).await;
    }

    let groups = create_groups(table).unwrap_or_default();

    if table.leaders.len() <= 1 && groups.is_empty() {
        // We have only one leader and no users
        return create_response_cetus(http.clone(), channel, rng, messages::NO_GROUPS).await;
    }

    if table.leaders.len() >= 2 && groups.is_empty() {
        // We have leaders and no users
    }

    create_response_cetus_groups(http.clone(), channel, rng, list, &groups).await?;

    Ok(())
}

pub async fn cycle(http: Arc<Http>) {
    async fn inner<R>(
        http: Arc<Http>,
        client: &Client,
        rng: &mut R,
        table: &mut EidolonTable,
        day: &mut bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        R: Rng,
    {
        let channel = match table.channel {
            Some(table) => table,
            None => return Ok(()),
        };

        let res = client.get(URL).send().await?;
        let json: Cetus = res.json().await?;

        let api = if *day == json.day {
            if json.short == "10m to Night" {
                Some(Api::Ten)
            } else if json.short == "5m to Day" {
                Some(Api::Five)
            } else {
                None
            }
        } else {
            *day = json.day;
            Some(Api::Change(json.day))
        };

        match api {
            Some(Api::Change(day)) => {
                if !table.leaders.is_empty() {
                    if day {
                        create_response_cetus(http, channel, rng, messages::CHANGE_DAY).await?;
                    } else {
                        create_response_cetus(http, channel, rng, messages::CHANGE_NIGHT).await?;
                    }
                }
            }
            Some(Api::Five) => {
                table.leaders.clear();
                table.users.clear();

                create_team_response(http, table, rng, messages::FIVE_MINUTE_WARNING).await?;
            }
            Some(Api::Ten) => {
                create_team_response(http, table, rng, messages::TEN_MINUTE_WARNING).await?;
            }
            None => {}
        }

        Ok(())
    }

    let client = Client::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let mut table = EidolonTable::default();
    let mut day = true;

    loop {
        tokio::time::sleep(Duration::new(60, 0)).await;

        if let Err(err) = inner(http.clone(), &client, &mut rng, &mut table, &mut day).await {}
    }
}

pub async fn eidolon(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    Ok(())
}
