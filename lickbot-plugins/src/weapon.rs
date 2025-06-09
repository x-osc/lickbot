use std::collections::HashMap;
use std::sync::LazyLock;

use azalea::inventory::{ItemStack, Menu, components};
use azalea::registry::Item;

/// finds the best weapon in hotbar and returns hotbar index
pub fn best_weapon_in_hotbar(menu: &Menu) -> usize {
    let hotbar_slots = &menu.slots()[menu.hotbar_slots_range()];

    let weapon_slots: Vec<(usize, &ItemStack)> = hotbar_slots.iter().enumerate().collect();

    // TODO: return option
    weapon_slots
        .iter()
        .max_by(|(_, item1), (_, item2)| {
            let dps1 = get_dps_fancy(item1);
            let dps2 = get_dps_fancy(item2);
            dps1.partial_cmp(&dps2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("should have iterator of length 9 (hotbar)")
        .0
}

/// Returns the dps of the item.
/// Takes into account invulnability frames.
/// This means the attack speed is capped at 2 per second because invulnability frames are 0.5 seconds long.
pub fn get_dps_capped(item: &ItemStack) -> f64 {
    let (damage, attack_speed) = get_damage_and_attack_speed_durability(item);

    // attack speed is limited to 2 per second because of damage immunity
    let capped_attack_speed = f64::min(attack_speed, 2.);
    damage * capped_attack_speed
}

/// Returns the dps of the item.
/// Does not take into account invulnability frames.
pub fn get_dps(item: &ItemStack) -> f64 {
    let (damage, attack_speed) = get_damage_and_attack_speed_durability(item);

    damage * attack_speed
}

/// Returns the dps of the item with a fancy formula to prioritize weapons with a faster attack speed.
pub fn get_dps_fancy(item: &ItemStack) -> f64 {
    let (damage, attack_speed) = get_damage_and_attack_speed_durability(item);

    // attack speed is limited to 2 per second because of damage immunity
    let capped_attack_speed = f64::min(attack_speed, 2.);
    // take average of attack speed and capped attack speed
    let dps = damage * (attack_speed + capped_attack_speed) / 2.0;
    // multiply dps by 1.(attack_speed) to make faster attack speed more valuable
    let new_dps = dps * (1. + capped_attack_speed / 10.0);

    #[allow(clippy::let_and_return)]
    new_dps
}

/// Returns the damage and attack speed of the item.
/// If the item has durability but is not a weapon, the damage is reduced to incentivise not using items with durability.
pub fn get_damage_and_attack_speed_durability(item: &ItemStack) -> (f64, f64) {
    // dps of fist
    let mut damage = 1.;
    let mut attack_speed = 4.;

    // damage from hashmap
    if let Some((in_damage, in_attack_speed)) = WEAPON_ITEMS.get(&item.kind()) {
        damage = *in_damage;
        attack_speed = *in_attack_speed;
    } else {
        // if has durability -> lower
        if let ItemStack::Present(item_data) = item
            && item_data.components.has::<components::Damage>()
        {
            damage = 0.8;
            attack_speed = 4.;
        }
    }

    (damage, attack_speed)
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
