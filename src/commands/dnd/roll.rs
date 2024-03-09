use std::collections::HashMap;

use caith::{RollResult, RollResultType, Roller};
use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    prelude::TypeMapKey,
};

use crate::{
    errors::ParrotError, messaging::message::ParrotMessage, metrics, utils::create_response,
};

pub(crate) struct RerollTable;
impl TypeMapKey for RerollTable {
    type Value = HashMap<String, caith::Roller>;
}

async fn process_roll(
    mut roller: caith::Roller,
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<Option<RollResult>, ParrotError> {
    match roller.roll() {
        Ok(res) => {
            {
                // do not store comment for reroll
                roller.trim_reason();
                let mut data = ctx.data.write().await;
                let reroll_table = data.get_mut::<RerollTable>().unwrap();
                reroll_table.insert(interaction.user.id.to_string(), roller);
            }
            Ok(Some(res))
        }
        Err(err) => {
            create_response(&ctx.http, interaction, ParrotMessage::RollError { err }).await?;

            Ok(None)
        }
    }
}

pub async fn roll(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let _timer = metrics::record_command(ctx, "roll");

    let args = interaction.data.options.clone();
    let first_arg = args.first().unwrap();
    let value = first_arg.value.as_ref().unwrap().as_str().unwrap();

    if let Some(res) = process_roll(Roller::new(value).unwrap(), ctx, interaction).await? {
        let sep = if res.as_repeated().is_some() {
            "\n"
        } else {
            ""
        }
        .to_string();

        metrics::record_roll(
            &interaction.user.id.to_string(),
            value,
            match res.get_result() {
                RollResultType::Single(roll) => roll.get_total(),
                RollResultType::Repeated(rolls) => rolls.iter().map(|roll| roll.get_total()).sum(),
            },
        );

        let roll = format!("{}{}", sep, res);

        create_response(&ctx.http, interaction, ParrotMessage::RollResult { roll }).await?;
    }

    Ok(())
}
