// yoinked from https://github.com/ShayBox/ShaysBot/blob/master/src/modules/auto_kill.rs
// MIT license
// copyright ShaysBox

use std::{cmp::Ordering, collections::HashMap, sync::LazyLock};

use azalea::{
    Hunger,
    app::{App, Plugin},
    ecs::prelude::*,
    entity::{LocalEntity, metadata::Player},
    interact::StartUseItemEvent,
    inventory::{
        ContainerClickEvent, Inventory, InventorySet, SetSelectedHotbarSlotEvent,
        operations::{ClickOperation, SwapClick},
    },
    mining::continue_mining_block,
    packet::game::handle_outgoing_packets,
    physics::PhysicsSet,
    prelude::*,
    protocol::packets::game::s_interact::InteractionHand,
    registry::Item,
};
use tracing::{debug, trace};

use crate::plugins::kill_aura::AutoKill;

/// Automatically eat food to avoid starving to death
pub struct AutoEatPlugin;

impl Plugin for AutoEatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            handle_auto_eat
                .after(crate::plugins::kill_aura::handle_auto_kill)
                .before(handle_outgoing_packets)
                .before(continue_mining_block)
                .before(InventorySet)
                .before(PhysicsSet),
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn handle_auto_eat(
    mut query: Query<
        (Entity, &Hunger, &Inventory, Option<&AutoKill>),
        (With<Player>, With<LocalEntity>),
    >,
    mut start_use_item_events: EventWriter<StartUseItemEvent>,
    mut container_click_events: EventWriter<ContainerClickEvent>,
    mut set_selected_hotbar_slot_events: EventWriter<SetSelectedHotbarSlotEvent>,
) {
    for (entity, hunger, inventory, auto_kill) in &mut query {
        // dont eat if killing
        if let Some(auto_kill) = auto_kill {
            if auto_kill.is_attacking {
                continue;
            }
        }

        if hunger.food >= 18 {
            continue;
        }

        let mut food_slots = Vec::new();

        for slot in inventory.inventory_menu.player_slots_range() {
            let Some(item) = inventory.inventory_menu.slot(slot) else {
                continue;
            };

            if let Some((nutrition, saturation)) = FOOD_ITEMS.get(&item.kind()) {
                food_slots.push((slot, item.kind(), *nutrition, *saturation));
            }
        }

        let best_food = food_slots.iter().max_by(|a, b| {
            b.3.partial_cmp(&a.3)
                .unwrap_or(Ordering::Equal)
                .then_with(|| b.2.cmp(&a.2))
        });

        let Some((best_slot, best_item, _, _)) = best_food else {
            trace!("No food found in inventory");
            continue;
        };

        if *best_item != inventory.held_item().kind() {
            // slot num is 0 indexed
            debug!("Swapping Food from {best_slot} and selecting slot {}", 9);

            container_click_events.write(ContainerClickEvent {
                entity,
                window_id: inventory.id,
                operation: ClickOperation::Swap(SwapClick {
                    source_slot: *best_slot as u16,
                    target_slot: 8,
                }),
            });

            if inventory.selected_hotbar_slot != 8 {
                set_selected_hotbar_slot_events
                    .write(SetSelectedHotbarSlotEvent { entity, slot: 8 });
            }
        }

        start_use_item_events.write(StartUseItemEvent {
            entity,
            hand: InteractionHand::MainHand,
            force_block: None,
        });
    }
}

pub static FOOD_ITEMS: LazyLock<HashMap<Item, (i32, f32)>> = LazyLock::new(|| {
    HashMap::from([
        (Item::Apple, (4, 2.4)),
        (Item::BakedPotato, (5, 6.0)),
        (Item::Beef, (3, 1.8)),
        (Item::Beetroot, (1, 1.2)),
        (Item::BeetrootSoup, (6, 7.2)),
        (Item::Bread, (5, 6.0)),
        (Item::Carrot, (3, 3.6)),
        (Item::Chicken, (2, 1.2)),
        (Item::Cod, (2, 0.4)),
        (Item::CookedBeef, (8, 12.8)),
        (Item::CookedChicken, (6, 7.2)),
        (Item::CookedCod, (5, 6.0)),
        (Item::CookedMutton, (6, 9.6)),
        (Item::CookedPorkchop, (8, 12.8)),
        (Item::CookedRabbit, (5, 6.0)),
        (Item::CookedSalmon, (6, 9.6)),
        (Item::Cookie, (2, 0.4)),
        (Item::DriedKelp, (1, 0.6)),
        (Item::EnchantedGoldenApple, (4, 9.6)),
        (Item::GlowBerries, (2, 0.4)),
        (Item::GoldenApple, (4, 9.6)),
        (Item::GoldenCarrot, (6, 14.4)),
        (Item::HoneyBottle, (6, 1.2)),
        (Item::MelonSlice, (2, 1.2)),
        (Item::MushroomStew, (6, 7.2)),
        (Item::Mutton, (2, 1.2)),
        (Item::Porkchop, (3, 1.8)),
        (Item::Potato, (1, 0.6)),
        (Item::PumpkinPie, (8, 4.8)),
        (Item::Rabbit, (3, 1.8)),
        (Item::RabbitStew, (10, 12.0)),
        (Item::Salmon, (2, 0.4)),
        (Item::SweetBerries, (2, 0.4)),
        (Item::TropicalFish, (1, 0.2)),
    ])
});
