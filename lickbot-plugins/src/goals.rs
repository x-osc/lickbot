use std::fmt::Debug;

use azalea::bot::direction_looking_at;
use azalea::interact::pick::pick_block;
use azalea::pathfinder::goals::{BlockPosGoal, Goal};
use azalea::world::ChunkStorage;
use azalea::{BlockPos, Vec3};

/// Move to a position where we can reach the given block.
#[derive(Clone)]
pub struct ReachBlockPosGoal {
    pub pos: BlockPos,
    pub distance: f64,
    pub chunk_storage: ChunkStorage,

    max_check_distance: i32,
}
impl ReachBlockPosGoal {
    pub fn new(pos: BlockPos, chunk_storage: ChunkStorage) -> Self {
        Self::new_with_distance(pos, 4.5, chunk_storage)
    }

    pub fn new_with_distance(pos: BlockPos, distance: f64, chunk_storage: ChunkStorage) -> Self {
        Self {
            pos,
            distance,
            chunk_storage,
            max_check_distance: (distance + 2.).ceil() as i32,
        }
    }
}
impl Goal for ReachBlockPosGoal {
    fn heuristic(&self, n: BlockPos) -> f32 {
        BlockPosGoal(self.pos).heuristic(n)
    }
    fn success(&self, n: BlockPos) -> bool {
        // only do the expensive check if we're close enough
        let distance = (self.pos - n).length_squared();
        if distance > self.max_check_distance * self.max_check_distance {
            return false;
        }

        if n == self.pos || n == self.pos.down(1) {
            return true;
        }

        let eye_position = n.to_vec3_floored() + Vec3::new(0.5, 1.62, 0.5);
        let look_direction = direction_looking_at(eye_position, self.pos.center());

        let block_hit_result = pick_block(
            look_direction,
            eye_position,
            &self.chunk_storage,
            self.distance,
        );

        block_hit_result.block_pos == self.pos
    }
}

impl Debug for ReachBlockPosGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct ReachBlockPosGoal {
            pos: BlockPos,
            distance: f64,
            max_check_distance: i32,
        }

        let Self {
            pos,
            distance,
            chunk_storage: _,
            max_check_distance,
        } = self;

        Debug::fmt(
            &ReachBlockPosGoal {
                pos: *pos,
                distance: *distance,
                max_check_distance: *max_check_distance,
            },
            f,
        )
    }
}

/// Move to a position where our head is directly adjacent to the given block.
#[derive(Clone, Debug)]
pub struct StandNextToBlockGoal {
    pub pos: BlockPos,
}
impl Goal for StandNextToBlockGoal {
    fn heuristic(&self, n: BlockPos) -> f32 {
        BlockPosGoal(self.pos).heuristic(n)
    }
    fn success(&self, n: BlockPos) -> bool {
        // if standing in the block
        n == self.pos || n == self.pos.down(1)
        // or on the block
        || n == self.pos.up(1)
        // or head is directly below the block
        || n == self.pos.down(2)
        // or head is right next to the block
        || n == self.pos.down(1).north(1)
        || n == self.pos.down(1).south(1)
        || n == self.pos.down(1).east(1)
        || n == self.pos.down(1).west(1)
    }
}

/// Move to a position where either head or feet are in the given block.
#[derive(Clone, Debug)]
pub struct StandInBlockGoal {
    pub pos: BlockPos,
}
impl Goal for StandInBlockGoal {
    fn heuristic(&self, n: BlockPos) -> f32 {
        BlockPosGoal(self.pos).heuristic(n)
    }
    fn success(&self, n: BlockPos) -> bool {
        // if standing in the block
        n == self.pos || n == self.pos.down(1)
    }
}
