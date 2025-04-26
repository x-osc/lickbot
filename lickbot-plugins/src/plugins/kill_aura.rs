// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Instant;

use azalea::app::{App, Plugin};
use azalea::attack::{AttackEvent, AttackStrengthScale};
use azalea::ecs::prelude::*;
use azalea::entity::metadata::Player;
use azalea::entity::{EyeHeight, LocalEntity, Position};
use azalea::inventory::{
    Inventory, InventorySet, ItemStack, Menu, SetSelectedHotbarSlotEvent, components,
};
use azalea::pathfinder::Pathfinder;
use azalea::physics::PhysicsSet;
use azalea::registry::Item;
use azalea::world::MinecraftEntityId;
use azalea::{LookAtEvent, Vec3, prelude::*};
use tracing::{debug, error, trace};

use crate::utils::entity_target::{EntityTarget, EntityTargets, TargetFinder};

/// Automatically swap weapon and attack nearby monsters
pub struct AutoKillPlugin;

impl Plugin for AutoKillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            handle_auto_kill
                .after(crate::plugins::auto_look::handle_auto_look)
                .before(InventorySet)
                .before(PhysicsSet),
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
    positions: Query<(&MinecraftEntityId, &Position, Option<&EyeHeight>)>,
    mut look_at_events: EventWriter<LookAtEvent>,
    mut attack_events: EventWriter<AttackEvent>,
    mut set_selected_hotbar_slot_events: EventWriter<SetSelectedHotbarSlotEvent>,
) {
    for (entity, mut auto_kill, inventory, pathfinder) in &mut query {
        let start = Instant::now();

        auto_kill.is_attacking = false;

        if let Some(pathfinder) = pathfinder {
            if pathfinder.goal.is_some() {
                continue;
            }
        }

        let Some(target) = targets.nearest_to_entity(entity, &auto_kill.targets, 3.2) else {
            continue;
        };

        let Ok((target_id, target_pos, maybe_eye_height)) = positions.get(target) else {
            continue;
        };

        let mut position: Vec3 = target_pos.into();
        if let Some(eye_height) = maybe_eye_height {
            position.y += f64::from(eye_height);
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
            set_selected_hotbar_slot_events.write(SetSelectedHotbarSlotEvent {
                entity,
                slot: best_slot,
            });
        }

        attack_events.write(AttackEvent {
            entity,
            target: *target_id,
        });

        let duration = start.elapsed();
        trace!("AutoKill took {:?}", duration);
    }
}

/// finds the best weapon in hotbar and returns hotbar index
pub fn best_weapon_in_hotbar(menu: &Menu) -> usize {
    let hotbar_slots = &menu.slots()[menu.hotbar_slots_range()];

    let weapon_slots: Vec<(usize, &ItemStack)> = hotbar_slots.iter().enumerate().collect();

    // TODO: return option
    weapon_slots
        .iter()
        .max_by(|(_, item1), (_, item2)| {
            let dps1 = get_dps(item1, true);
            let dps2 = get_dps(item2, true);
            dps1.partial_cmp(&dps2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("should have iterator of length 9 (hotbar)")
        .0
}

fn get_dps(item: &ItemStack, do_fancy_calculation: bool) -> f64 {
    // dps of fist
    let mut damage = 1.;
    let mut attack_speed = 4.;

    // damage from hashmap
    if let Some((in_damage, in_attack_speed)) = WEAPON_ITEMS.get(&item.kind()) {
        damage = *in_damage;
        attack_speed = *in_attack_speed;
    } else {
        // if has durability -> lower
        if let ItemStack::Present(item_data) = item {
            if item_data.components.has::<components::Damage>() {
                damage = 0.8;
                attack_speed = 4.;
            }
        }
    }

    // attack speed is limited to 2 per second because of damage immunity
    let capped_attack_speed = f64::min(attack_speed, 2.);
    if do_fancy_calculation {
        // take average of attack speed and capped attack speed
        let dps = damage * (attack_speed + capped_attack_speed) / 2.0;
        // multiply dps by 1.(attack_speed) to make faster attack speed more valuable
        let new_dps = dps * (1. + capped_attack_speed / 10.0);

        #[allow(clippy::let_and_return)]
        new_dps
    } else {
        damage * capped_attack_speed
    }
}

/// damage and attack speed of each weapon in the game
/// https://minecraft.wiki/w/Damage#Dealing_damage
pub static WEAPON_ITEMS: LazyLock<HashMap<Item, (f64, f64)>> = LazyLock::new(|| {
    HashMap::from([
        (Item::WoodenSword, (4., 1.6)),
        (Item::GoldenSword, (4., 1.6)),
        (Item::StoneSword, (5., 1.6)),
        (Item::IronSword, (6., 1.6)),
        (Item::DiamondSword, (7., 1.6)),
        (Item::NetheriteSword, (8., 1.6)),
        //
        (Item::WoodenAxe, (7., 0.8)),
        (Item::GoldenAxe, (7., 1.)),
        (Item::StoneAxe, (9., 0.8)),
        (Item::IronAxe, (9., 0.9)),
        (Item::DiamondAxe, (9., 1.)),
        (Item::NetheriteAxe, (10., 1.)),
        //
        (Item::WoodenPickaxe, (2., 1.2)),
        (Item::GoldenPickaxe, (2., 1.2)),
        (Item::StonePickaxe, (3., 1.2)),
        (Item::IronPickaxe, (4., 1.2)),
        (Item::DiamondPickaxe, (5., 1.2)),
        (Item::NetheritePickaxe, (6., 1.2)),
        //
        (Item::WoodenShovel, (2.5, 1.)),
        (Item::GoldenShovel, (2.5, 1.)),
        (Item::StoneShovel, (3.5, 1.)),
        (Item::IronShovel, (4.5, 1.)),
        (Item::DiamondShovel, (5.5, 1.)),
        (Item::NetheriteShovel, (6.5, 1.)),
        //
        (Item::WoodenHoe, (1., 1.)),
        (Item::GoldenHoe, (1., 1.)),
        (Item::StoneHoe, (1., 2.)),
        (Item::IronHoe, (1., 3.)),
        (Item::DiamondHoe, (1., 4.)),
        (Item::NetheriteHoe, (1., 4.)),
        //
        (Item::Trident, (9., 1.1)),
        (Item::Mace, (6., 0.6)),
    ])
});

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
