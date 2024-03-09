use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};

use crate::{
    errors::ParrotError, messaging::message::ParrotMusicMessage, metrics,
    utils::create_response_music,
};

pub async fn version(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "version");

    let current = option_env!("CARGO_PKG_VERSION").unwrap_or_else(|| "Unknown");
    create_response_music(
        &ctx.http,
        interaction,
        ParrotMusicMessage::Version {
            current: current.to_owned(),
        },
    )
    .await
}
