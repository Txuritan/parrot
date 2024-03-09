use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
};

use crate::{
    errors::ParrotError,
    guild::settings::{GuildSettings, GuildSettingsMap},
    messaging::message::{ParrotMessage, ParrotMusicMessage},
    metrics,
    utils::create_response_music,
};

pub async fn autopause(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "autopause");

    let guild_id = interaction.guild_id.unwrap();
    let mut data = ctx.data.write().await;
    let settings = data.get_mut::<GuildSettingsMap>().unwrap();

    let guild_settings = settings
        .entry(guild_id)
        .or_insert_with(|| GuildSettings::new(guild_id));
    guild_settings.toggle_autopause();
    guild_settings.save()?;

    if guild_settings.autopause {
        create_response_music(&ctx.http, interaction, ParrotMusicMessage::AutopauseOn).await
    } else {
        create_response_music(&ctx.http, interaction, ParrotMusicMessage::AutopauseOff).await
    }
}
