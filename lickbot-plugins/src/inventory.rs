use azalea::inventory::ItemStack;
use azalea::registry::Item;

pub fn num_items_in_slots(slots: &[ItemStack], item: Item) -> i32 {
    slots
        .iter()
        .map(|item_stack| match item_stack {
            ItemStack::Present(data) => {
                if data.kind == item {
                    data.count
                } else {
                    0
                }
            }
            ItemStack::Empty => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use azalea::inventory::{DataComponentPatch, ItemStack, ItemStackData};
    use azalea::registry::Item;

    use super::num_items_in_slots;

    #[test]
    fn test_num_items_in_slots() {
        let slots = vec![
            ItemStack::Present(ItemStackData {
                kind: Item::Diamond,
                count: 17,
                component_patch: DataComponentPatch::default(),
            }),
            ItemStack::Present(ItemStackData {
                kind: Item::Diamond,
                count: 3,
                component_patch: DataComponentPatch::default(),
            }),
            ItemStack::Empty,
            ItemStack::Present(ItemStackData {
                kind: Item::Dirt,
                count: 5,
                component_patch: DataComponentPatch::default(),
            }),
        ];

        let total = num_items_in_slots(&slots, Item::Diamond);
        assert_eq!(total, 20);
    }
}
