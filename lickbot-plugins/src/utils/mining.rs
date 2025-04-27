use azalea::auto_tool::best_tool_in_hotbar_for_block;
use azalea::inventory::SetSelectedHotbarSlotEvent;
use azalea::{BlockPos, BotClientExt, Client};
use tracing::warn;

pub trait MiningExtrasClientExt {
    fn mine_with_best_tool(&self, pos: BlockPos) -> impl Future<Output = ()> + Send;
}

impl MiningExtrasClientExt for Client {
    async fn mine_with_best_tool(&self, pos: BlockPos) {
        let block_state = self
            .world()
            .read()
            .get_block_state(&pos)
            .unwrap_or_default();
        if block_state.is_air() {
            warn!("Block is air, not mining");
            return;
        }
        let best_tool_result = best_tool_in_hotbar_for_block(block_state, &self.menu());
        if best_tool_result.percentage_per_tick == 0. {
            warn!("Block is not breakable, not mining");
            return;
        }

        self.ecs.lock().send_event(SetSelectedHotbarSlotEvent {
            entity: self.entity,
            slot: best_tool_result.index as u8,
        });

        self.look_at(pos.center());
        self.mine(pos).await;
    }
}
