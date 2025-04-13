// this file is a modified version of https://github.com/AS1100K/aether/blob/main/plugins/utility/src/auto_totem/mod.rs
// licensed under the GPL 3.0
// copyright AS1100k

use azalea::app::{App, Plugin, Update};
use azalea::ecs::prelude::*;
use azalea::entity::LocalEntity;
use azalea::entity::metadata::Player;
use azalea::inventory::operations::{ClickOperation, SwapClick};
use azalea::inventory::{
    self, ContainerClickEvent, Inventory, ItemStack, Menu, handle_container_click_event,
};
use azalea::prelude::*;
use azalea::registry::Item;
use tracing::{debug, info};

/// Plugin which automatically switches totem to offhand.
#[derive(Clone, Default)]
pub struct AutoTotemPlugin;
impl Plugin for AutoTotemPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EnableAutoTotemEvent>()
            .add_event::<DisableAutoTotemEvent>()
            .add_systems(
                GameTick,
                handle_auto_totem.before(handle_container_click_event),
            )
            .add_systems(
                Update,
                (enable_auto_totem_listener, disable_auto_totem_listener),
            );
    }
}

/// Component present when autototem is enabled.
#[derive(Component)]
pub struct AutoTotem;

/// Enable autototem for an entity.
#[derive(Event)]
pub struct EnableAutoTotemEvent {
    pub entity: Entity,
}

/// Disable autototem for an entity.
#[derive(Event)]
pub struct DisableAutoTotemEvent {
    pub entity: Entity,
}

#[allow(clippy::type_complexity)]
fn handle_auto_totem(
    query: Query<(Entity, &Inventory), (With<AutoTotem>, With<Player>, With<LocalEntity>)>,
    mut container_click_event: EventWriter<ContainerClickEvent>,
) {
    for (entity, inventory_component) in query.iter() {
        // guaranteed to be `Menu::Player`
        let Menu::Player(player_inventory) = &inventory_component.inventory_menu else {
            continue;
        };

        if player_inventory.offhand.kind() == Item::TotemOfUndying {
            continue;
        }

        let menu = &inventory_component.inventory_menu;
        let slots = &menu.slots()[menu.player_slots_range()];
        let mut totem_index: Option<usize> = None;

        for (i, stack) in slots.iter().enumerate() {
            if let ItemStack::Present(item_data) = stack {
                if item_data.kind == Item::TotemOfUndying {
                    let index = i + menu.player_slots_range().start();
                    totem_index = Some(index);
                    info!("found totem at index {}", index);
                    break;
                }
            }
        }

        println!("{:?}", inventory::Player::OFFHAND_SLOT as u8);
        println!("{:?}", menu.slot(totem_index.unwrap_or(1)));

        if let Some(index) = totem_index {
            container_click_event.send(ContainerClickEvent {
                entity,
                window_id: inventory_component.id,
                operation: ClickOperation::Swap(SwapClick {
                    source_slot: index as u16,
                    target_slot: 36,
                }),
            });
        }
    }
}

fn enable_auto_totem_listener(
    mut events: EventReader<EnableAutoTotemEvent>,
    mut commands: Commands,
) {
    for event in events.read() {
        debug!("enabled autototem");
        commands.entity(event.entity).insert(AutoTotem);
    }
}

fn disable_auto_totem_listener(
    mut events: EventReader<DisableAutoTotemEvent>,
    mut commands: Commands,
) {
    for event in events.read() {
        debug!("disabled autototem");
        commands.entity(event.entity).remove::<AutoTotem>();
    }
}
