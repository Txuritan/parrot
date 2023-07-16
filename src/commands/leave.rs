use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};

use crate::{
    errors::ParrotError, messaging::message::ParrotMessage, metrics, utils::create_response,
};

pub async fn leave(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "leave");

    let guild_id = interaction.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    manager.remove(guild_id).await.unwrap();

    create_response(&ctx.http, interaction, ParrotMessage::Leaving).await
}
