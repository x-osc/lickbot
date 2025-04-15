// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::collections::HashMap;
use std::sync::LazyLock;

use azalea::app::{App, Plugin};
use azalea::attack::{AttackEvent, AttackStrengthScale};
use azalea::ecs::prelude::*;
use azalea::entity::metadata::{AbstractMonster, Player};
use azalea::entity::{Dead, EyeHeight, LocalEntity, Position};
use azalea::inventory::{
    Inventory, InventorySet, ItemStack, Menu, SetSelectedHotbarSlotEvent, components,
};
use azalea::nearest_entity::EntityFinder;
use azalea::pathfinder::Pathfinder;
use azalea::physics::PhysicsSet;
use azalea::registry::Item;
use azalea::world::MinecraftEntityId;
use azalea::{LookAtEvent, Vec3, prelude::*};
use tracing::{debug, error};

/// Automatically swap weapon and attack nearby monsters
pub struct AutoKillPlugin;

impl Plugin for AutoKillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            (
                handle_auto_weapon.before(InventorySet),
                handle_auto_kill.after(crate::modules::auto_look::handle_auto_look),
            )
                .chain()
                .before(PhysicsSet),
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn handle_auto_kill(
    mut query: Query<Entity, (With<Player>, With<LocalEntity>)>,
    pathfinders: Query<&Pathfinder, (With<Player>, With<LocalEntity>)>,
    attack_strengths: Query<&AttackStrengthScale, (With<Player>, With<LocalEntity>)>,
    entities: EntityFinder<(With<AbstractMonster>, Without<LocalEntity>, Without<Dead>)>,
    targets: Query<(&MinecraftEntityId, &Position, Option<&EyeHeight>)>,
    mut look_at_events: EventWriter<LookAtEvent>,
    mut attack_events: EventWriter<AttackEvent>,
) {
    for entity in &mut query {
        if let Ok(pathfinder) = pathfinders.get(entity) {
            if pathfinder.goal.is_some() {
                continue;
            }
        }

        let Some(target) = entities.nearest_to_entity(entity, 3.2) else {
            continue;
        };

        let Ok((target_id, target_pos, maybe_eye_height)) = targets.get(target) else {
            continue;
        };

        let mut position: Vec3 = target_pos.into();
        if let Some(eye_height) = maybe_eye_height {
            position.y += f64::from(eye_height);
        }

        look_at_events.send(LookAtEvent { entity, position });

        if let Ok(AttackStrengthScale(scale)) = attack_strengths.get(entity) {
            if *scale < 1.0 {
                continue;
            }
        } else {
            error!("player with killaura doesn't have AttackStrengthScale component");
        };

        attack_events.send(AttackEvent {
            entity,
            target: *target_id,
        });
    }
}

#[allow(clippy::type_complexity)]
fn handle_auto_weapon(
    query: Query<(Entity, &Inventory), (With<Player>, With<LocalEntity>)>,
    pathfinders: Query<&Pathfinder, (With<Player>, With<LocalEntity>)>,
    entities: EntityFinder<With<AbstractMonster>>,
    mut set_selected_hotbar_slot_events: EventWriter<SetSelectedHotbarSlotEvent>,
) {
    for (entity, inventory) in &query {
        if let Ok(pathfinder) = pathfinders.get(entity) {
            if pathfinder.goal.is_some() {
                continue;
            }
        }

        if entities.nearest_to_entity(entity, 3.2).is_none() {
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
    }
}

/// finds the best weapon in hotbar and returns hotbar index
pub fn best_weapon_in_hotbar(menu: &Menu) -> usize {
    let hotbar_slots = &menu.slots()[menu.hotbar_slots_range()];

    let weapon_slots: Vec<(usize, &ItemStack)> = hotbar_slots.iter().enumerate().collect();

    // TODO: return option
    weapon_slots
        .iter()
        .max_by_key(|(_, item)| {
            // damage from hashmap
            if let Some(damage) = WEAPON_ITEMS.get(&item.kind()) {
                return damage;
            }

            // if has durability -> lower
            if let ItemStack::Present(item_data) = item {
                if item_data.components.has::<components::Damage>() {
                    return &-1;
                }
            }

            &0
        })
        .expect("should have iterator of length 9 (hotbar)")
        .0
}

pub static WEAPON_ITEMS: LazyLock<HashMap<Item, i32>> = LazyLock::new(|| {
    HashMap::from([
        (Item::DiamondAxe, 9),
        (Item::DiamondSword, 7),
        (Item::GoldenAxe, 7),
        (Item::GoldenSword, 4),
        (Item::IronAxe, 9),
        (Item::IronSword, 6),
        (Item::NetheriteAxe, 10),
        (Item::NetheriteSword, 8),
        (Item::StoneAxe, 9),
        (Item::StoneSword, 5),
        (Item::WoodenAxe, 7),
        (Item::WoodenSword, 4),
    ])
});
