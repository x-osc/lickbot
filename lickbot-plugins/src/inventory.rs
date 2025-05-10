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
