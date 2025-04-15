use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::metadata::Player;
use azalea::entity::{EyeHeight, LocalEntity, Position};
use azalea::nearest_entity::EntityFinder;
use azalea::physics::PhysicsSet;
use azalea::{LookAtEvent, Vec3, prelude::*};

/// Automatically look at the nearest player
pub struct AutoLookPlugin;

impl Plugin for AutoLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameTick, handle_auto_look.before(PhysicsSet));
    }
}

/// Component present when autolook is enabled
#[derive(Component, Clone)]
pub struct AutoLook;

#[allow(clippy::type_complexity)]
pub fn handle_auto_look(
    query: Query<Entity, (With<AutoLook>, With<Player>, With<LocalEntity>)>,
    entities: EntityFinder<With<Player>>,
    targets: Query<(&Position, Option<&EyeHeight>)>,
    mut look_at_events: EventWriter<LookAtEvent>,
) {
    for entity in &query {
        let Some(target) = entities.nearest_to_entity(entity, f64::MAX) else {
            continue;
        };

        let Ok((target_pos, maybe_eye_height)) = targets.get(target) else {
            continue;
        };

        let mut position: Vec3 = target_pos.into();
        if let Some(eye_height) = maybe_eye_height {
            position.y += f64::from(eye_height);
        }

        look_at_events.send(LookAtEvent { entity, position });
    }
}
