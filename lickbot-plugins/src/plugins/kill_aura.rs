// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::time::Instant;

use azalea::app::{App, Plugin};
use azalea::attack::{AttackEvent, AttackStrengthScale};
use azalea::bot::LookAtEvent;
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::Player;
use azalea::entity::{LocalEntity, Position};
use azalea::inventory::{Inventory, InventorySystems, SetSelectedHotbarSlotEvent};
use azalea::pathfinder::Pathfinder;
use azalea::physics::PhysicsSystems;
use azalea::world::MinecraftEntityId;
use azalea::{Vec3, prelude::*};
use tracing::{debug, error, trace};

use crate::entity_target::{EntityTarget, EntityTargets, TargetFinder};
use crate::plugins;
use crate::weapon::best_weapon_in_hotbar;

/// Automatically swap weapon and attack nearby monsters
pub struct AutoKillPlugin;

impl Plugin for AutoKillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            handle_auto_kill
                .after(plugins::auto_look::handle_auto_look)
                .before(plugins::look_when_mining::look_while_mining)
                .before(InventorySystems)
                .before(PhysicsSystems),
        );
    }
}

/// Component present when auto kill is enabled.
#[derive(Component, Clone)]
pub struct AutoKill {
    /// if true, will switch to the best weapon in hotbar
    pub switch_weapon: bool,
    /// if true, will knock back the target when close
    /// (will attack when charge is not refilled and target is close)
    pub knock_back_when_close: bool,
    /// target to attack
    pub targets: EntityTargets,

    /// whether currently attacking a target
    pub is_attacking: bool,
}

impl Default for AutoKill {
    fn default() -> Self {
        Self {
            switch_weapon: true,
            knock_back_when_close: true,
            targets: EntityTargets::new(&[EntityTarget::AllMonsters]),
            is_attacking: false,
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn handle_auto_kill(
    mut query: Query<
        (
            Entity,
            &mut AutoKill,
            Option<&Inventory>,
            Option<&Pathfinder>,
        ),
        (With<Player>, With<LocalEntity>),
    >,
    attack_strengths: Query<&AttackStrengthScale, (With<Player>, With<LocalEntity>)>,

    targets: TargetFinder,
    positions: Query<(&MinecraftEntityId, &Position, Option<&EntityDimensions>)>,
    mut look_at_events: MessageWriter<LookAtEvent>,
    mut attack_events: MessageWriter<AttackEvent>,
    mut commands: Commands,
) {
    for (entity, mut auto_kill, inventory, pathfinder) in &mut query {
        let start = Instant::now();

        auto_kill.is_attacking = false;

        if let Some(pathfinder) = pathfinder
            && pathfinder.goal.is_some()
        {
            continue;
        }

        let Some(target) = targets.nearest_to_entity(entity, &auto_kill.targets, 3.2) else {
            continue;
        };

        let Ok((_, target_pos, maybe_dimensions)) = positions.get(target) else {
            continue;
        };

        let mut position: Vec3 = target_pos.into();
        if let Some(dimensions) = maybe_dimensions {
            position.y += f64::from(dimensions.eye_height);
        }

        auto_kill.is_attacking = true;
        look_at_events.write(LookAtEvent { entity, position });

        // if target is within 0.7 blocks, try to knock it away, even if charge is not refilled
        if !(auto_kill.knock_back_when_close
            && targets
                .nearest_to_entity(entity, &auto_kill.targets, 0.7)
                .is_some())
        {
            if let Ok(AttackStrengthScale(scale)) = attack_strengths.get(entity) {
                if *scale < 1.0 {
                    continue;
                }
            } else {
                error!("player with killaura doesn't have AttackStrengthScale component");
            };
        }

        // switch weapon
        let Some(inventory) = inventory else {
            error!("player with killaura doesn't have Inventory component");
            continue;
        };

        let best_slot = best_weapon_in_hotbar(&inventory.inventory_menu) as u8;
        if inventory.selected_hotbar_slot != best_slot {
            debug!("setting selected weapon to slot {}", best_slot);
            commands.trigger(SetSelectedHotbarSlotEvent {
                entity,
                slot: best_slot,
            });
        }

        attack_events.write(AttackEvent { entity, target });

        let duration = start.elapsed();
        trace!("AutoKill took {:?}", duration);
    }
}

pub trait AutoKillClientExt {
    /// Enable auto kill
    fn enable_auto_kill(&self, targets: EntityTargets);
    /// Disable auto kill
    fn disable_auto_kill(&self);
}

impl AutoKillClientExt for Client {
    fn enable_auto_kill(&self, targets: EntityTargets) {
        self.ecs.lock().entity_mut(self.entity).remove::<AutoKill>();

        self.ecs.lock().entity_mut(self.entity).insert(AutoKill {
            targets,
            ..Default::default()
        });
    }

    fn disable_auto_kill(&self) {
        self.ecs.lock().entity_mut(self.entity).remove::<AutoKill>();
    }
}
