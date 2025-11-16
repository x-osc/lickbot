use azalea::{
    app::{App, Plugin},
    bot::LookAtEvent,
    entity::{LocalEntity, metadata::Player},
    mining::{MineBlockPos, MiningSystems},
    pathfinder::Pathfinder,
    prelude::*,
};
use bevy_ecs::prelude::*;

/// A plugin that makes the player always look at the block they are mining.
pub struct LookMinePlugin;

impl Plugin for LookMinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameTick, look_while_mining.after(MiningSystems));
    }
}

// TODO: figure out how to stop it from spamming the log (might have to modify lib)
#[allow(clippy::type_complexity)]
pub fn look_while_mining(
    query: Query<(Entity, &MineBlockPos, Option<&Pathfinder>), (With<Player>, With<LocalEntity>)>,
    mut look_at_events: MessageWriter<LookAtEvent>,
) {
    for (entity, mining_component, pathfinder) in &query {
        // let pathfinder handle looking at
        if let Some(pathfinder) = pathfinder
            && pathfinder.goal.is_some()
        {
            continue;
        }

        if let Some(pos) = **mining_component {
            look_at_events.write(LookAtEvent {
                entity,
                position: pos.center(),
            });
        }
    }
}
