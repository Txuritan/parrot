use serenity::{
    builder::CreateEmbed, client::Context, json::Value,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};
use songbird::tracks::TrackHandle;

use crate::{
    errors::{verify, ParrotError},
    messaging::messages::FAIL_VOLUME_PARSING,
    metrics,
    utils::create_embed_response,
};

pub async fn volume(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "volume");

    let args = interaction.data.options.clone();

    let volume = match args.first() {
        Some(arg) => arg.value.as_ref().and_then(Value::as_i64).map(|n| n as f32),
        None => return display_volume(ctx, interaction).await,
    };

    update_volume(ctx, interaction, volume).await
}

async fn display_volume(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let handler = call.lock().await;
    let queue = handler.queue();

    verify(!queue.is_empty(), ParrotError::NothingPlaying)?;

    let track_handle: TrackHandle = queue.current().ok_or(ParrotError::NothingPlaying)?;
    let volume = track_handle.get_info().await.unwrap().volume;

    let mut embed = CreateEmbed::default();
    embed.description(format!("Current volume is {}%", volume * 100.0));
    create_embed_response(&ctx.http, interaction, embed).await?;

    Ok(())
}

async fn update_volume(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
    volume: Option<f32>,
) -> Result<(), ParrotError> {
    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let volume = verify(volume, ParrotError::Other(FAIL_VOLUME_PARSING))?;

    let adjusted_volume = volume / 100.0;

    let handler = call.lock().await;
    let queue = handler.queue();

    verify(!queue.is_empty(), ParrotError::NothingPlaying)?;

    let track_handle: TrackHandle = queue.current().ok_or(ParrotError::NothingPlaying)?;
    let old_volume = track_handle.get_info().await.unwrap().volume;

    track_handle.set_volume(adjusted_volume).unwrap();

    let embed = create_volume_embed(old_volume, adjusted_volume);

    create_embed_response(&ctx.http, interaction, embed).await
}

fn create_volume_embed(old_volume: f32, new_volume: f32) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.description(format!(
        "Volume changed from {}% to {}%",
        old_volume, new_volume
    ));
    embed
}
