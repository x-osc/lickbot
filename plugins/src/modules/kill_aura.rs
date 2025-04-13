// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;

use azalea::app::{App, Plugin};
use azalea::attack::AttackEvent;
use azalea::ecs::prelude::*;
use azalea::entity::metadata::{AbstractMonster, Player};
use azalea::entity::{EyeHeight, LocalEntity, Position};
use azalea::inventory::operations::{ClickOperation, SwapClick};
use azalea::inventory::{ContainerClickEvent, Inventory};
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
    mut container_click_events: EventWriter<ContainerClickEvent>,
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

        let held_kind = inventory.held_item().kind();
        if !WEAPON_ITEMS.contains_key(&held_kind) {
            let mut weapon_slots = Vec::new();

            for slot in inventory.inventory_menu.player_slots_range() {
                let Some(item) = inventory.inventory_menu.slot(slot) else {
                    continue;
                };

                if let Some(damage) = WEAPON_ITEMS.get(&item.kind()) {
                    weapon_slots.push((slot, *damage));
                }
            }

            weapon_slots.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

            if let Some((slot, _)) = weapon_slots.first() {
                debug!(
                    "Swapping Weapon from {slot} to {}",
                    inventory.selected_hotbar_slot
                );
                container_click_events.send(ContainerClickEvent {
                    entity,
                    window_id: inventory.id,
                    operation: ClickOperation::Swap(SwapClick {
                        source_slot: u16::try_from(*slot).unwrap(),
                        target_slot: inventory.selected_hotbar_slot,
                    }),
                });
            }
        }

        attack_events.send(AttackEvent {
            entity,
            target: *target_id,
        });
    }
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
