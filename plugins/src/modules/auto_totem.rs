// this file is a modified version of https://github.com/AS1100K/aether/blob/main/plugins/utility/src/auto_totem/mod.rs
// licensed under the GPL 3.0
// copyright AS1100k

use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::LocalEntity;
use azalea::entity::metadata::Player;
use azalea::inventory::operations::{ClickOperation, SwapClick};
use azalea::inventory::{self, ContainerClickEvent, Inventory, ItemStack, Menu};
use azalea::prelude::*;
use azalea::registry::Item;
use tracing::{debug, error};

/// Plugin which automatically switches totem to offhand.
#[derive(Clone, Default)]
pub struct AutoTotemPlugin;
impl Plugin for AutoTotemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            handle_auto_totem.before(inventory::handle_container_click_event),
        );
    }
}

/// Component present when autototem is enabled.
#[derive(Component)]
pub struct AutoTotem;

#[allow(clippy::type_complexity)]
pub fn handle_auto_totem(
    query: Query<(Entity, &Inventory), (With<AutoTotem>, With<Player>, With<LocalEntity>)>,
    mut container_click_event: EventWriter<ContainerClickEvent>,
) {
    for (entity, inventory) in query.iter() {
        // guaranteed to be `Menu::Player`
        let Menu::Player(player_inventory) = &inventory.inventory_menu else {
            continue;
        };

        if player_inventory.offhand.kind() == Item::TotemOfUndying {
            continue;
        }

        let menu = &inventory.inventory_menu;

        let mut totem_index: Option<usize> = None;
        for slot in menu.player_slots_range() {
            let Some(item) = menu.slot(slot) else {
                error!("tried to access player slot out of bounds");
                continue;
            };

            let ItemStack::Present(item_data) = item else {
                continue;
            };

            if item_data.kind == Item::TotemOfUndying {
                totem_index = Some(slot);
                debug!("found totem at slot {}", slot)
            }
        }

        if let Some(index) = totem_index {
            container_click_event.send(ContainerClickEvent {
                entity,
                window_id: inventory.id,
                operation: ClickOperation::Swap(SwapClick {
                    source_slot: index as u16,
                    // this is the button number, 40 is for offhand
                    // https://minecraft.wiki/w/Java_Edition_protocol#Click_Container
                    target_slot: 40,
                }),
            });
        }
    }
}
