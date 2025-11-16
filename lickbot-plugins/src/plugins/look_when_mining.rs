use azalea::{
    app::{App, Plugin},
    bot::LookAtEvent,
    ecs::prelude::*,
    entity::{LocalEntity, metadata::Player},
    mining::{MineBlockPos, MiningSystems},
    pathfinder::{self, Pathfinder},
    prelude::*,
};

/// A plugin that makes the player always look at the block they are mining.
pub struct LookMinePlugin;

impl Plugin for LookMinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            look_while_mining
                .after(MiningSystems)
                .after(pathfinder::recalculate_if_has_goal_but_no_path),
        );
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
