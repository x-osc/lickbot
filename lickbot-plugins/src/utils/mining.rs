use azalea::auto_tool::best_tool_in_hotbar_for_block;
use azalea::interact::pick;
use azalea::inventory::SetSelectedHotbarSlotEvent;
use azalea::world::ChunkStorage;
use azalea::{BlockPos, BotClientExt, Client, Vec3, direction_looking_at};
use tracing::warn;

pub trait MiningExtrasClientExt {
    fn mine_with_best_tool(&self, pos: BlockPos) -> impl Future<Output = ()> + Send;
    fn look_and_mine(&self, pos: BlockPos) -> impl Future<Output = ()> + Send;
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

        self.look_and_mine(pos).await;
    }

    async fn look_and_mine(&self, pos: BlockPos) {
        if !can_mine_block(pos, self.eye_position(), &self.world().read().chunks) {
            warn!("Block is not reachable, not mining");
            return;
        }

        self.look_at(pos.center());
        self.mine(pos).await;
    }
}

pub fn can_mine_block(pos: BlockPos, eye_pos: Vec3, chunks: &ChunkStorage) -> bool {
    let look_direction = direction_looking_at(&eye_pos, &pos.center());
    let block_hit_result = pick(&look_direction, &eye_pos, chunks, 3.5);
    block_hit_result.block_pos == pos
}
