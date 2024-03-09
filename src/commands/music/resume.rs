use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};

use crate::{
    errors::{verify, ParrotError},
    messaging::message::ParrotMusicMessage,
    metrics,
    utils::create_response_music,
};

pub async fn resume(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "resume");

    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let handler = call.lock().await;
    let queue = handler.queue();

    verify(!queue.is_empty(), ParrotError::NothingPlaying)?;
    verify(queue.resume(), ParrotError::Other("Failed resuming track"))?;

    create_response_music(&ctx.http, interaction, ParrotMusicMessage::Resume).await
}
