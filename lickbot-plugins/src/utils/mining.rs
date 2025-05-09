use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use azalea::auto_tool::best_tool_in_hotbar_for_block;
use azalea::blocks::Block;
use azalea::interact::pick;
use azalea::inventory::SetSelectedHotbarSlotEvent;
use azalea::pathfinder::astar::PathfinderTimeout;
use azalea::pathfinder::goals::OrGoals;
use azalea::pathfinder::{GotoEvent, moves};
use azalea::prelude::PathfinderClientExt;
use azalea::world::ChunkStorage;
use azalea::{BlockPos, BotClientExt, Client, Vec3, direction_looking_at};
use thiserror::Error;
use tracing::{debug, warn};

use crate::utils::goals::{ReachBlockPosGoal, StandInBlockGoal, StandNextToBlockGoal};

pub trait MiningExtrasClientExt {
    fn mine_with_best_tool(
        &self,
        pos: &BlockPos,
    ) -> impl Future<Output = Result<(), MiningError>> + Send;
    fn checked_mine(&self, pos: &BlockPos) -> impl Future<Output = Result<(), MiningError>> + Send;
    fn goto_and_try_mine_block(
        &self,
        pos: &BlockPos,
    ) -> impl Future<Output = Result<(), CantMineAnyError>> + Send;
    fn goto_and_try_mine_blocks(
        &self,
        blocks_pos: &[BlockPos],
    ) -> impl std::future::Future<Output = Result<(), CantMineAnyError>> + Send;
}

impl MiningExtrasClientExt for Client {
    async fn mine_with_best_tool(&self, pos: &BlockPos) -> Result<(), MiningError> {
        match can_mine_block(pos, self.eye_position(), &self.world().read().chunks) {
            Ok(_) => (),
            Err(e) => return Err(e),
        }

        let block_state = self.world().read().get_block_state(pos).unwrap_or_default();
        let best_tool_result = best_tool_in_hotbar_for_block(block_state, &self.menu());
        if best_tool_result.percentage_per_tick == 0. {
            return Err(MiningError::BlockIsNotBreakable);
        }

        self.ecs.lock().send_event(SetSelectedHotbarSlotEvent {
            entity: self.entity,
            slot: best_tool_result.index as u8,
        });

        self.look_at(pos.center());
        self.mine(*pos).await;

        Ok(())
    }

    async fn checked_mine(&self, pos: &BlockPos) -> Result<(), MiningError> {
        match can_mine_block(pos, self.eye_position(), &self.world().read().chunks) {
            Ok(_) => (),
            Err(e) => return Err(e),
        }

        self.look_at(pos.center());
        self.mine(*pos).await;

        Ok(())
    }

    async fn goto_and_try_mine_block(&self, pos: &BlockPos) -> Result<(), CantMineAnyError> {
        self.goto_and_try_mine_blocks(&[*pos]).await
    }

    async fn goto_and_try_mine_blocks(
        &self,
        blocks_pos: &[BlockPos],
    ) -> Result<(), CantMineAnyError> {
        let goal = OrGoals(
            blocks_pos
                .iter()
                .map(|pos| {
                    ReachBlockPosGoal::new_with_distance(
                        *pos,
                        3.2,
                        self.world().read().chunks.clone(),
                    )
                })
                .collect(),
        );
        self.ecs.lock().send_event(GotoEvent {
            entity: self.entity,
            goal: Arc::new(goal),
            successors_fn: moves::default_move,
            allow_mining: true,
            min_timeout: PathfinderTimeout::Time(Duration::from_secs(2)),
            max_timeout: PathfinderTimeout::Time(Duration::from_secs(10)),
        });
        self.wait_until_goto_target_reached().await;

        debug!("mining!");
        if try_mine_blocks(self, blocks_pos).await.is_ok() {
            return Ok(());
        }

        warn!("could not mine any blocks, trying to get closer");

        let goal = OrGoals(
            blocks_pos
                .iter()
                .map(|pos| StandNextToBlockGoal { pos: *pos })
                .collect(),
        );
        self.ecs.lock().send_event(GotoEvent {
            entity: self.entity,
            goal: Arc::new(goal),
            successors_fn: moves::default_move,
            allow_mining: true,
            min_timeout: PathfinderTimeout::Time(Duration::from_secs(2)),
            max_timeout: PathfinderTimeout::Time(Duration::from_secs(10)),
        });
        self.wait_until_goto_target_reached().await;

        debug!("mining!");
        if try_mine_blocks(self, blocks_pos).await.is_ok() {
            return Ok(());
        }

        warn!("could not mine any blocks, trying to stand in block");

        let goal = OrGoals(
            blocks_pos
                .iter()
                .map(|pos| StandInBlockGoal { pos: *pos })
                .collect(),
        );
        self.ecs.lock().send_event(GotoEvent {
            entity: self.entity,
            goal: Arc::new(goal),
            successors_fn: moves::default_move,
            allow_mining: true,
            min_timeout: PathfinderTimeout::Time(Duration::from_secs(2)),
            max_timeout: PathfinderTimeout::Time(Duration::from_secs(10)),
        });
        self.wait_until_goto_target_reached().await;

        if try_mine_blocks(self, blocks_pos).await.is_ok() {
            return Ok(());
        }

        warn!("could not mine any blocks, returning");

        Err(CantMineAnyError {
            blocks_pos: blocks_pos.to_vec(),
        })
    }
}

pub fn can_mine_block(
    pos: &BlockPos,
    eye_pos: Vec3,
    chunks: &ChunkStorage,
) -> Result<(), MiningError> {
    let block_state = chunks.get_block_state(pos).unwrap_or_default();
    if block_state.is_air() {
        return Err(MiningError::BlockIsAir);
    }
    let block: Box<dyn Block> = block_state.into();
    if (*block).behavior().destroy_time < -1. {
        return Err(MiningError::BlockIsNotBreakable);
    }

    let max_pick_range = 6;
    let actual_pick_range = 3.5;

    let distance = pos.distance_squared_to(&eye_pos.to_block_pos_ceil());
    if distance > max_pick_range * max_pick_range {
        return Err(MiningError::BlockIsNotReachable);
    }

    let look_direction = direction_looking_at(&eye_pos, &pos.center());
    let block_hit_result = pick(&look_direction, &eye_pos, chunks, actual_pick_range);
    if !(block_hit_result.block_pos == *pos) {
        return Err(MiningError::BlockIsNotReachable);
    }

    Ok(())
}

async fn try_mine_blocks(bot: &Client, blocks_pos: &[BlockPos]) -> Result<(), CantMineAnyError> {
    for block_pos in blocks_pos {
        let block_state = bot
            .world()
            .read()
            .get_block_state(block_pos)
            .unwrap_or_default();
        if block_state.is_air() {
            if block_pos.distance_squared_to(&bot.position().to_block_pos_ceil()) < 4 * 4 {
                // block was probably mined by the bot
                warn!("block {} is mined, returning", block_pos);
                return Ok(());
            }

            // block was probably mined by someone else
            warn!("block {} is already mined", block_pos);
            continue;
        }

        match bot.mine_with_best_tool(block_pos).await {
            Ok(_) => {
                return Ok(());
            }
            Err(_) => {
                continue;
            }
        }
    }

    Err(CantMineAnyError {
        // TODO: this is horrible
        // actually now that i changed it its not so horrible but still pretty bad we should change it
        blocks_pos: blocks_pos.to_vec(),
    })
}

#[derive(Debug, Error)]
pub enum MiningError {
    #[error("Block is air")]
    BlockIsAir,
    #[error("Block is not breakable")]
    BlockIsNotBreakable,
    #[error("Block is not reachable")]
    BlockIsNotReachable,
}

#[derive(Debug)]
pub struct CantMineAnyError {
    pub blocks_pos: Vec<BlockPos>,
}

impl Display for CantMineAnyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cant mine any of the blocks: {:?}", self.blocks_pos)
    }
}

impl Error for CantMineAnyError {}
