use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};

use crate::{
    errors::{verify, ParrotError},
    handlers::track_end::update_queue_messages,
    messaging::message::ParrotMusicMessage,
    metrics,
    utils::create_response_music,
};

pub async fn stop(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "stop");

    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let handler = call.lock().await;
    let queue = handler.queue();

    verify(!queue.is_empty(), ParrotError::NothingPlaying)?;
    queue.stop();

    // refetch the queue after modification
    let queue = handler.queue().current_queue();
    drop(handler);

    create_response_music(&ctx.http, interaction, ParrotMusicMessage::Stop).await?;
    update_queue_messages(&ctx.http, &ctx.data, &queue, guild_id).await;
    Ok(())
}
