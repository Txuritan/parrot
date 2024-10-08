use std::{collections::HashSet, sync::Arc};

use serenity::{
    client::Context,
    model::{
        application::interaction::application_command::ApplicationCommandInteraction, id::GuildId,
    },
    prelude::{Mentionable, RwLock, TypeMap},
};

use crate::{
    commands::music::skip::{create_skip_response, force_skip_top_track},
    connection::get_voice_channel_for_user,
    errors::{verify, ParrotError},
    guild::cache::GuildCacheMap,
    messaging::message::ParrotMusicMessage,
    metrics,
    utils::create_response_music,
};

pub async fn voteskip(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "voteskip");

    let guild_id = interaction.guild_id.unwrap();
    let bot_channel_id = get_voice_channel_for_user(
        &ctx.cache.guild(guild_id).unwrap(),
        &ctx.cache.current_user_id(),
    )
    .unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let handler = call.lock().await;
    let queue = handler.queue();

    verify(!queue.is_empty(), ParrotError::NothingPlaying)?;

    let mut data = ctx.data.write().await;
    let cache_map = data.get_mut::<GuildCacheMap>().unwrap();

    let cache = cache_map.entry(guild_id).or_default();
    cache.current_skip_votes.insert(interaction.user.id);

    let guild_users = ctx.cache.guild(guild_id).unwrap().voice_states;
    let channel_guild_users = guild_users
        .into_values()
        .filter(|v| v.channel_id.unwrap() == bot_channel_id);
    let skip_threshold = channel_guild_users.count() / 2;

    if cache.current_skip_votes.len() >= skip_threshold {
        force_skip_top_track(&handler).await?;
        create_skip_response(ctx, interaction, &handler, 1).await
    } else {
        create_response_music(
            &ctx.http,
            interaction,
            ParrotMusicMessage::VoteSkip {
                mention: interaction.user.id.mention(),
                missing: skip_threshold - cache.current_skip_votes.len(),
            },
        )
        .await
    }
}

pub async fn forget_skip_votes(data: &Arc<RwLock<TypeMap>>, guild_id: GuildId) -> Result<(), ()> {
    let mut data = data.write().await;

    let cache_map = data.get_mut::<GuildCacheMap>().ok_or(())?;
    let cache = cache_map.get_mut(&guild_id).ok_or(())?;
    cache.current_skip_votes = HashSet::new();

    Ok(())
}
