use std::{cmp::Ordering, error::Error as StdError, sync::Arc, time::Duration};

use serenity::{
    builder::CreateEmbed, client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    prelude::Mutex,
};
use songbird::{input::Restartable, tracks::TrackHandle, Call};
use url::Url;

use crate::{
    commands::music::{skip::force_skip_top_track, summon::summon},
    errors::{verify, ParrotError},
    guild::settings::{GuildSettings, GuildSettingsMap},
    handlers::track_end::update_queue_messages,
    messaging::message::ParrotMusicMessage,
    messaging::messages::{
        PLAY_QUEUE, PLAY_TOP, SPOTIFY_AUTH_FAILED, TRACK_DURATION, TRACK_TIME_TO_PLAY,
    },
    metrics,
    sources::{
        spotify::{Spotify, SPOTIFY},
        youtube::{YouTube, YouTubeRestartable},
    },
    utils::{
        compare_domains, create_now_playing_embed, create_response_music, edit_embed_response,
        edit_response_music, get_human_readable_timestamp,
    },
};

#[derive(Clone, Copy)]
pub enum Mode {
    End,
    Next,
    All,
    Reverse,
    Shuffle,
    Jump,
}

#[derive(Clone)]
pub enum QueryType {
    Keywords(String),
    KeywordList(Vec<String>),
    VideoLink(String),
    PlaylistLink(String),
}

#[tracing::instrument(skip(ctx, interaction), err)]
pub async fn play(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "play");

    let args = interaction.data.options.clone();
    let first_arg = args.first().unwrap();

    let mode = match first_arg.name.as_str() {
        "next" => Mode::Next,
        "all" => Mode::All,
        "reverse" => Mode::Reverse,
        "shuffle" => Mode::Shuffle,
        "jump" => Mode::Jump,
        _ => Mode::End,
    };

    let url = match mode {
        Mode::End => first_arg.value.as_ref().unwrap().as_str().unwrap(),
        _ => first_arg
            .options
            .first()
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap(),
    };

    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();

    // try to join a voice channel if not in one just yet
    summon(ctx, interaction, false).await?;
    let call = manager.get(guild_id).unwrap();

    tracing::info!(url = %url, "adding url to queue");

    // determine whether this is a link or a query string
    let query_type = match Url::parse(url) {
        Ok(url_data) => match url_data.host_str() {
            Some("open.spotify.com") => {
                let spotify = SPOTIFY.lock().await;
                let spotify = verify(spotify.as_ref(), ParrotError::Other(SPOTIFY_AUTH_FAILED))?;

                spotify.request_token().await?;

                Some(Spotify::extract(spotify, url).await?)
            }
            Some(other) => {
                let mut data = ctx.data.write().await;
                let settings = data.get_mut::<GuildSettingsMap>().unwrap();
                let guild_settings = settings
                    .entry(guild_id)
                    .or_insert_with(|| GuildSettings::new(guild_id));

                let is_allowed = guild_settings
                    .allowed_domains
                    .iter()
                    .any(|d| compare_domains(d, other));

                let is_banned = guild_settings
                    .banned_domains
                    .iter()
                    .any(|d| compare_domains(d, other));

                if is_banned || (guild_settings.banned_domains.is_empty() && !is_allowed) {
                    return create_response_music(
                        &ctx.http,
                        interaction,
                        ParrotMusicMessage::PlayDomainBanned {
                            domain: other.to_string(),
                        },
                    )
                    .await;
                }

                YouTube::extract(url)
            }
            None => None,
        },
        Err(_) => {
            let mut data = ctx.data.write().await;
            let settings = data.get_mut::<GuildSettingsMap>().unwrap();
            let guild_settings = settings
                .entry(guild_id)
                .or_insert_with(|| GuildSettings::new(guild_id));

            if guild_settings.banned_domains.contains("youtube.com")
                || (guild_settings.banned_domains.is_empty()
                    && !guild_settings.allowed_domains.contains("youtube.com"))
            {
                return create_response_music(
                    &ctx.http,
                    interaction,
                    ParrotMusicMessage::PlayDomainBanned {
                        domain: "youtube.com".to_string(),
                    },
                )
                .await;
            }

            Some(QueryType::Keywords(url.to_string()))
        }
    };

    let query_type = verify(
        query_type,
        ParrotError::Other("Something went wrong while parsing your query!"),
    )?;

    // reply with a temporary message while we fetch the source
    // needed because interactions must be replied within 3s and queueing takes longer
    create_response_music(&ctx.http, interaction, ParrotMusicMessage::Search).await?;

    let handler = call.lock().await;
    let queue_was_empty = handler.queue().is_empty();
    drop(handler);

    match mode {
        Mode::End => match query_type.clone() {
            QueryType::Keywords(_) | QueryType::VideoLink(_) => {
                let queue = enqueue_track(&call, &query_type).await?;
                update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
            }
            QueryType::PlaylistLink(url) => {
                let urls = YouTubeRestartable::ytdl_playlist(&url, mode)
                    .await
                    .ok_or(ParrotError::Other("failed to fetch playlist"))?;

                for url in urls.iter() {
                    let queue =
                        match enqueue_track(&call, &QueryType::VideoLink(url.to_string())).await {
                            Ok(queue) => queue,
                            Err(err) => {
                                tracing::error!(err = ?err, url = %url, "Failed to enqueue track");
                                continue;
                            }
                        };
                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
            QueryType::KeywordList(keywords_list) => {
                for keywords in keywords_list.iter() {
                    let queue =
                        enqueue_track(&call, &QueryType::Keywords(keywords.to_string())).await?;
                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
        },
        Mode::Next => match query_type.clone() {
            QueryType::Keywords(_) | QueryType::VideoLink(_) => {
                let queue = insert_track(&call, &query_type, 1).await?;
                update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
            }
            QueryType::PlaylistLink(url) => {
                let urls = YouTubeRestartable::ytdl_playlist(&url, mode)
                    .await
                    .ok_or(ParrotError::Other("failed to fetch playlist"))?;

                for (idx, url) in urls.into_iter().enumerate() {
                    let queue = match insert_track(
                        &call,
                        &QueryType::VideoLink(url.clone()),
                        idx + 1,
                    )
                    .await
                    {
                        Ok(queue) => queue,
                        Err(err) => {
                            tracing::error!(err = ?err, url = %url, "Failed to insert track");
                            continue;
                        }
                    };
                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
            QueryType::KeywordList(keywords_list) => {
                for (idx, keywords) in keywords_list.into_iter().enumerate() {
                    let queue =
                        insert_track(&call, &QueryType::Keywords(keywords), idx + 1).await?;
                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
        },
        Mode::Jump => match query_type.clone() {
            QueryType::Keywords(_) | QueryType::VideoLink(_) => {
                let mut queue = enqueue_track(&call, &query_type).await?;

                if !queue_was_empty {
                    rotate_tracks(&call, 1).await.ok();
                    queue = force_skip_top_track(&call.lock().await).await?;
                }

                update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
            }
            QueryType::PlaylistLink(url) => {
                let urls = YouTubeRestartable::ytdl_playlist(&url, mode)
                    .await
                    .ok_or(ParrotError::Other("failed to fetch playlist"))?;

                let mut insert_idx = 1;

                for (i, url) in urls.into_iter().enumerate() {
                    let mut queue =
                        match insert_track(&call, &QueryType::VideoLink(url.clone()), insert_idx)
                            .await
                        {
                            Ok(queue) => queue,
                            Err(err) => {
                                tracing::error!(err = ?err, url = %url, "Failed to insert track");
                                continue;
                            }
                        };

                    if i == 0 && !queue_was_empty {
                        queue = force_skip_top_track(&call.lock().await).await?;
                    } else {
                        insert_idx += 1;
                    }

                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
            QueryType::KeywordList(keywords_list) => {
                let mut insert_idx = 1;

                for (i, keywords) in keywords_list.into_iter().enumerate() {
                    let mut queue =
                        insert_track(&call, &QueryType::Keywords(keywords), insert_idx).await?;

                    if i == 0 && !queue_was_empty {
                        queue = force_skip_top_track(&call.lock().await).await?;
                    } else {
                        insert_idx += 1;
                    }

                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
        },
        Mode::All | Mode::Reverse | Mode::Shuffle => match query_type.clone() {
            QueryType::VideoLink(url) | QueryType::PlaylistLink(url) => {
                let urls = YouTubeRestartable::ytdl_playlist(&url, mode)
                    .await
                    .ok_or(ParrotError::Other("failed to fetch playlist"))?;

                for url in urls.into_iter() {
                    let queue = match enqueue_track(&call, &QueryType::VideoLink(url.clone())).await
                    {
                        Ok(queue) => queue,
                        Err(err) => {
                            tracing::error!(err = ?err, url = %url, "Failed to enqueue track");
                            continue;
                        }
                    };
                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
            QueryType::KeywordList(keywords_list) => {
                for keywords in keywords_list.into_iter() {
                    let queue = enqueue_track(&call, &QueryType::Keywords(keywords)).await?;
                    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
                }
            }
            _ => {
                create_response_music(&ctx.http, interaction, ParrotMusicMessage::PlayAllFailed)
                    .await?;
                return Ok(());
            }
        },
    }

    let handler = call.lock().await;

    let mut data = ctx.data.write().await;
    let settings = data.get_mut::<GuildSettingsMap>().unwrap();
    let guild_settings = settings
        .entry(guild_id)
        .or_insert_with(|| GuildSettings::new(guild_id));

    // refetch the queue after modification
    let queue = handler.queue().current_queue();
    queue
        .iter()
        .for_each(|t| t.set_volume(guild_settings.default_volume).unwrap());
    drop(handler);

    match queue.len().cmp(&1) {
        Ordering::Greater => {
            let estimated_time = calculate_time_until_play(&queue, mode).await.unwrap();

            match (query_type, mode) {
                (QueryType::VideoLink(_) | QueryType::Keywords(_), Mode::Next) => {
                    let track = queue.get(1).unwrap();
                    let embed = create_queued_embed(PLAY_TOP, track, estimated_time).await;

                    edit_embed_response(&ctx.http, interaction, embed).await?;
                }
                (QueryType::VideoLink(_) | QueryType::Keywords(_), Mode::End) => {
                    let track = queue.last().unwrap();
                    let embed = create_queued_embed(PLAY_QUEUE, track, estimated_time).await;

                    edit_embed_response(&ctx.http, interaction, embed).await?;
                }
                (QueryType::PlaylistLink(_) | QueryType::KeywordList(_), _) => {
                    edit_response_music(&ctx.http, interaction, ParrotMusicMessage::PlaylistQueued)
                        .await?;
                }
                (_, _) => {}
            }
        }
        Ordering::Equal => {
            let track = queue.first().unwrap();
            let embed = create_now_playing_embed(track).await;

            edit_embed_response(&ctx.http, interaction, embed).await?;
        }
        // TODO: if the link is not valid (eg has `\` at the end), queue will be 0, figure out how to handle that
        _ => unreachable!(),
    }

    Ok(())
}

async fn calculate_time_until_play(queue: &[TrackHandle], mode: Mode) -> Option<Duration> {
    if queue.is_empty() {
        return None;
    }

    let top_track = queue.first()?;
    let top_track_elapsed = top_track.get_info().await.unwrap().position;

    let top_track_duration = match top_track.metadata().duration {
        Some(duration) => duration,
        None => return Some(Duration::MAX),
    };

    match mode {
        Mode::Next => Some(top_track_duration - top_track_elapsed),
        _ => {
            let center = &queue[1..queue.len() - 1];
            let livestreams =
                center.len() - center.iter().filter_map(|t| t.metadata().duration).count();

            // if any of the tracks before are livestreams, the new track will never play
            if livestreams > 0 {
                return Some(Duration::MAX);
            }

            let durations = center.iter().fold(Duration::ZERO, |acc, x| {
                acc + x.metadata().duration.unwrap()
            });

            Some(durations + top_track_duration - top_track_elapsed)
        }
    }
}

async fn create_queued_embed(
    title: &str,
    track: &TrackHandle,
    estimated_time: Duration,
) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    let metadata = track.metadata().clone();

    embed.thumbnail(&metadata.thumbnail.unwrap());

    embed.field(
        title,
        &format!(
            "[**{}**]({})",
            metadata.title.unwrap(),
            metadata.source_url.unwrap()
        ),
        false,
    );

    let footer_text = format!(
        "{}{}\n{}{}",
        TRACK_DURATION,
        get_human_readable_timestamp(metadata.duration),
        TRACK_TIME_TO_PLAY,
        get_human_readable_timestamp(Some(estimated_time))
    );

    embed.footer(|footer| footer.text(footer_text));
    embed
}

async fn get_track_source(query_type: QueryType) -> Result<Restartable, ParrotError> {
    match query_type {
        QueryType::VideoLink(query) => YouTubeRestartable::ytdl(query, true)
            .await
            .map_err(ParrotError::TrackFail),

        QueryType::Keywords(query) => YouTubeRestartable::ytdl_search(query, true)
            .await
            .map_err(ParrotError::TrackFail),

        _ => unreachable!(),
    }
}

async fn enqueue_track(
    call: &Arc<Mutex<Call>>,
    query_type: &QueryType,
) -> Result<Vec<TrackHandle>, ParrotError> {
    // safeguard against ytdl dying on a private/deleted video and killing the playlist
    let source = get_track_source(query_type.clone()).await?;

    let mut handler = call.lock().await;
    handler.enqueue_source(source.into());

    Ok(handler.queue().current_queue())
}

async fn insert_track(
    call: &Arc<Mutex<Call>>,
    query_type: &QueryType,
    idx: usize,
) -> Result<Vec<TrackHandle>, ParrotError> {
    let handler = call.lock().await;
    let queue_size = handler.queue().len();
    drop(handler);

    if queue_size <= 1 {
        let queue = enqueue_track(call, query_type).await?;
        return Ok(queue);
    }

    verify(
        idx > 0 && idx <= queue_size,
        ParrotError::NotInRange("index", idx as isize, 1, queue_size as isize),
    )?;

    enqueue_track(call, query_type).await?;

    let handler = call.lock().await;
    handler.queue().modify_queue(|queue| {
        let back = queue.pop_back().unwrap();
        queue.insert(idx, back);
    });

    Ok(handler.queue().current_queue())
}

async fn rotate_tracks(
    call: &Arc<Mutex<Call>>,
    n: usize,
) -> Result<Vec<TrackHandle>, Box<dyn StdError>> {
    let handler = call.lock().await;

    verify(
        handler.queue().len() > 2,
        ParrotError::Other("cannot rotate queues smaller than 3 tracks"),
    )?;

    handler.queue().modify_queue(|queue| {
        let mut not_playing = queue.split_off(1);
        not_playing.rotate_right(n);
        queue.append(&mut not_playing);
    });

    Ok(handler.queue().current_queue())
}
