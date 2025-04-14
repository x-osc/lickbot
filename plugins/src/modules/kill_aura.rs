// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::collections::HashMap;
use std::sync::LazyLock;

use azalea::app::{App, Plugin};
use azalea::attack::AttackEvent;
use azalea::ecs::prelude::*;
use azalea::entity::metadata::{AbstractMonster, Player};
use azalea::entity::{EyeHeight, LocalEntity, Position};
use azalea::inventory::{Inventory, ItemStack, Menu, SetSelectedHotbarSlotEvent, components};
use azalea::nearest_entity::EntityFinder;
use azalea::pathfinder::Pathfinder;
use azalea::physics::PhysicsSet;
use azalea::registry::Item;
use azalea::world::MinecraftEntityId;
use azalea::{LookAtEvent, Vec3, prelude::*};
use tracing::debug;

/// Automatically swap and attack nearby monsters
pub struct AutoKillPlugin;

impl Plugin for AutoKillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            handle_auto_kill
                .after(super::auto_look::handle_auto_look)
                .before(PhysicsSet),
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn handle_auto_kill(
    mut query: Query<(Entity, &Inventory, &Pathfinder), (With<Player>, With<LocalEntity>)>,
    entities: EntityFinder<With<AbstractMonster>>,
    targets: Query<(&MinecraftEntityId, &Position, Option<&EyeHeight>)>,
    mut set_selected_hotbar_slot_events: EventWriter<SetSelectedHotbarSlotEvent>,
    mut look_at_events: EventWriter<LookAtEvent>,
    mut attack_events: EventWriter<AttackEvent>,
) {
    for (entity, inventory, pathfinder) in &mut query {
        if let Some(_goal) = &pathfinder.goal {
            continue;
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

        // add delay here ?

        let best_slot = best_weapon_in_hotbar(&inventory.inventory_menu) as u8;
        if inventory.selected_hotbar_slot != best_slot {
            debug!("setting selected weapon to {}", best_slot);
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
