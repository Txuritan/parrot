use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};

use crate::{
    errors::{verify, ParrotError},
    messaging::message::ParrotMessage,
    metrics,
    utils::create_response,
};

pub async fn pause(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "pause");

    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let handler = call.lock().await;
    let queue = handler.queue();

    verify(!queue.is_empty(), ParrotError::NothingPlaying)?;
    verify(queue.pause(), ParrotError::Other("Failed to pause"))?;

    create_response(&ctx.http, interaction, ParrotMessage::Pause).await
}
