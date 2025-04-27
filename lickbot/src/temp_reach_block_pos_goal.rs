use std::fmt::Debug;

use azalea::interact::pick;
use azalea::pathfinder::goals::{BlockPosGoal, Goal};
use azalea::world::ChunkStorage;
use azalea::{BlockPos, Vec3, direction_looking_at};
use tracing::info;

/// Move to a position where we can reach the given block.
#[derive(Clone)]
pub struct ReachBlockPosGoal {
    pub pos: BlockPos,
    pub chunk_storage: ChunkStorage,
}
impl Goal for ReachBlockPosGoal {
    fn heuristic(&self, n: BlockPos) -> f32 {
        BlockPosGoal(self.pos).heuristic(n)
    }
    fn success(&self, n: BlockPos) -> bool {
        // only do the expensive check if we're close enough
        let max_pick_range = 6;
        let actual_pick_range = 4.5;

        let distance = (self.pos - n).length_squared();
        if distance > max_pick_range * max_pick_range {
            return false;
        }

        if n == self.pos {
            return true;
        }

        let eye_position = n.to_vec3_floored() + Vec3::new(0.5, 1.62, 0.5);
        let look_direction = direction_looking_at(&eye_position, &self.pos.center());
        let block_hit_result = pick(
            &look_direction,
            &eye_position,
            &self.chunk_storage,
            actual_pick_range,
        );

        info!("block_hit_result: {:?}", block_hit_result);

        block_hit_result.block_pos == self.pos
    }
}

impl Debug for ReachBlockPosGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct ReachBlockPosGoal<'a> {
            pos: &'a BlockPos,
        }

        let Self {
            pos,
            chunk_storage: _,
        } = self;

        Debug::fmt(&ReachBlockPosGoal { pos }, f)
    }
}
