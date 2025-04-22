// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::collections::HashMap;
use std::sync::LazyLock;

use azalea::app::{App, Plugin, Update};
use azalea::attack::{AttackEvent, AttackStrengthScale};
use azalea::ecs::prelude::*;
use azalea::entity::metadata::Player;
use azalea::entity::{EyeHeight, LocalEntity, Position};
use azalea::events::LocalPlayerEvents;
use azalea::inventory::{
    Inventory, InventorySet, ItemStack, Menu, SetSelectedHotbarSlotEvent, components,
};
use azalea::pathfinder::Pathfinder;
use azalea::physics::PhysicsSet;
use azalea::registry::Item;
use azalea::world::MinecraftEntityId;
use azalea::{LookAtEvent, Vec3, prelude::*};
use tracing::{debug, error};

use crate::utils::entity_target::{EntityTarget, EntityTargets, TargetFinder};

/// Automatically swap weapon and attack nearby monsters
pub struct AutoKillPlugin;

impl Plugin for AutoKillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            handle_auto_kill
                .after(crate::modules::auto_look::handle_auto_look)
                .before(InventorySet)
                .before(PhysicsSet),
        )
        .add_systems(Update, insert_auto_kill);
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
        look_at_events.send(LookAtEvent { entity, position });

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
            set_selected_hotbar_slot_events.send(SetSelectedHotbarSlotEvent {
                entity,
                slot: best_slot,
            });
        }

        attack_events.send(AttackEvent {
            entity,
            target: *target_id,
        });
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
            let dps1 = get_dps(item1);
            let dps2 = get_dps(item2);
            dps1.partial_cmp(&dps2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("should have iterator of length 9 (hotbar)")
        .0
}

fn get_dps(item: &ItemStack) -> f64 {
    // damage from hashmap
    if let Some(dps) = WEAPON_ITEMS.get(&item.kind()) {
        return *dps;
    }

    // if has durability -> lower
    if let ItemStack::Present(item_data) = item {
        if item_data.components.has::<components::Damage>() {
            return 1.;
        }
    }

    // dps of fist against single target
    2.
}

/// DPS of each weapon in the game
/// https://minecraft.wiki/w/Damage#Dealing_damage
pub static WEAPON_ITEMS: LazyLock<HashMap<Item, f64>> = LazyLock::new(|| {
    HashMap::from([
        (Item::WoodenSword, 6.4),
        (Item::GoldenSword, 6.4),
        (Item::StoneSword, 8.),
        (Item::IronSword, 9.6),
        (Item::DiamondSword, 11.2),
        (Item::NetheriteSword, 12.8),
        //
        (Item::WoodenAxe, 5.6),
        (Item::GoldenAxe, 7.),
        (Item::StoneAxe, 7.2),
        (Item::IronAxe, 8.1),
        (Item::DiamondAxe, 9.),
        (Item::NetheriteAxe, 10.),
        //
        (Item::WoodenPickaxe, 2.4),
        (Item::GoldenPickaxe, 2.4),
        (Item::StonePickaxe, 3.6),
        (Item::IronPickaxe, 4.8),
        (Item::DiamondPickaxe, 6.),
        (Item::NetheritePickaxe, 7.2),
        //
        (Item::WoodenShovel, 2.5),
        (Item::GoldenShovel, 2.5),
        (Item::StoneShovel, 3.5),
        (Item::IronShovel, 4.5),
        (Item::DiamondShovel, 5.5),
        (Item::NetheriteShovel, 6.5),
        //
        (Item::WoodenHoe, 1.),
        (Item::GoldenHoe, 1.),
        (Item::StoneHoe, 2.),
        (Item::IronHoe, 3.),
        (Item::DiamondHoe, 4.),
        (Item::NetheriteHoe, 4.),
        //
        (Item::Trident, 9.9),
        (Item::Mace, 3.6),
    ])
});

#[allow(clippy::type_complexity)]
fn insert_auto_kill(
    mut commands: Commands,
    mut query: Query<
        Entity,
        (
            Without<AutoKill>,
            // added when player logs in
            Added<LocalPlayerEvents>,
        ),
    >,
) {
    for entity in &mut query {
        commands.entity(entity).insert(AutoKill::default());
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
