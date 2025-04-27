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
