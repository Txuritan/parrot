use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};
use songbird::tracks::{LoopState, TrackHandle};

use crate::{
    errors::ParrotError, messaging::message::ParrotMusicMessage, messaging::messages::FAIL_LOOP,
    metrics, utils::create_response_music,
};

pub async fn repeat(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "repeat");

    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    let call = manager.get(guild_id).unwrap();

    let handler = call.lock().await;
    let track = handler.queue().current().unwrap();

    let was_looping = track.get_info().await.unwrap().loops == LoopState::Infinite;
    let toggler = if was_looping {
        TrackHandle::disable_loop
    } else {
        TrackHandle::enable_loop
    };

    match toggler(&track) {
        Ok(_) if was_looping => {
            create_response_music(&ctx.http, interaction, ParrotMusicMessage::LoopDisable).await
        }
        Ok(_) if !was_looping => {
            create_response_music(&ctx.http, interaction, ParrotMusicMessage::LoopEnable).await
        }
        _ => Err(ParrotError::Other(FAIL_LOOP)),
    }
}
