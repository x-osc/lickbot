use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use azalea::auto_tool::best_tool_in_hotbar_for_block;
use azalea::blocks::Block;
use azalea::core::hit_result::HitResult;
use azalea::entity::Position;
use azalea::interact::pick;
use azalea::inventory::SetSelectedHotbarSlotEvent;
use azalea::pathfinder::astar::PathfinderTimeout;
use azalea::pathfinder::goals::OrGoals;
use azalea::pathfinder::{GotoEvent, moves};
use azalea::prelude::PathfinderClientExt;
use azalea::registry::Item;
use azalea::world::ChunkStorage;
use azalea::{BlockPos, BotClientExt, Client, Vec3, direction_looking_at};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::goals::{ReachBlockPosGoal, StandInBlockGoal, StandNextToBlockGoal};
use crate::inventory::num_items_in_slots;

use super::nearest_entity::NearestEntityClientExt;

pub trait MiningExtrasClientExt {
    //// Mines a block with the best tool in hotbar.
    /// Also checks whether the block is mineable.
    fn mine_block_with_best_tool(
        &self,
        pos: &BlockPos,
    ) -> impl Future<Output = Result<(), MiningError>> + Send;
    /// Mines one of the blocks in the list with the best tool in hotbar.
    fn mine_blocks_with_best_tool(
        &self,
        blocks_pos: &[BlockPos],
    ) -> impl Future<Output = Result<(), CantMineAnyError>> + Send;
    /// Mines a checks whether the block is mineable and then mines it.
    fn checked_mine(&self, pos: &BlockPos) -> impl Future<Output = Result<(), MiningError>> + Send;
    /// Goto the block and try to mine it.
    /// Will retry 3 times.
    fn goto_and_try_mine_block(
        &self,
        pos: &BlockPos,
    ) -> impl Future<Output = Result<(), CantMineAnyError>> + Send;
    /// Will mine the easiest to reach of the blocks in the list.
    /// Will retry 3 times.
    fn goto_and_try_mine_blocks(
        &self,
        blocks_pos: &[BlockPos],
    ) -> impl std::future::Future<Output = Result<(), CantMineAnyError>> + Send;
    fn try_pick_up_item(
        &self,
        item: Item,
    ) -> impl std::future::Future<Output = Result<(), NoItemsError>> + Send;
}

impl MiningExtrasClientExt for Client {
    async fn mine_block_with_best_tool(&self, pos: &BlockPos) -> Result<(), MiningError> {
        can_mine_block(pos, self.eye_position(), &self.world().read().chunks)?;

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

    async fn mine_blocks_with_best_tool(
        &self,
        blocks_pos: &[BlockPos],
    ) -> Result<(), CantMineAnyError> {
        for block_pos in blocks_pos {
            #[allow(clippy::redundant_pattern_matching)]
            if let Ok(_) = self.mine_block_with_best_tool(block_pos).await {
                return Ok(());
            }
        }

        Err(CantMineAnyError)
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
        if mine_blocks_with_best_tool_unless_already_mined(self, blocks_pos)
            .await
            .is_ok()
        {
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
        if mine_blocks_with_best_tool_unless_already_mined(self, blocks_pos)
            .await
            .is_ok()
        {
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

        if mine_blocks_with_best_tool_unless_already_mined(self, blocks_pos)
            .await
            .is_ok()
        {
            return Ok(());
        }

        warn!("could not mine any blocks, returning");

        Err(CantMineAnyError)
    }

    async fn try_pick_up_item(&self, item: Item) -> Result<(), NoItemsError> {
        let nearest_items = self.nearest_items_by_distance(item, 20.).take(5);
        let nearest_positions: Vec<_> = nearest_items
            .map(|entity| *self.ecs.lock().get::<Position>(entity).unwrap())
            .collect();

        if nearest_positions.is_empty() {
            return Err(NoItemsError);
        }

        let inventory_items = &self.menu().slots()[self.menu().player_slots_range()];
        let num_items = num_items_in_slots(inventory_items, item);
        debug!("num_items: {}", num_items);

        info!(
            "pickup items at {:?}",
            nearest_positions
                .iter()
                .map(|pos| pos.to_block_pos_floor())
                .collect::<Vec<_>>()
        );
        let goal = OrGoals(
            nearest_positions
                .iter()
                .map(|pos| StandInBlockGoal {
                    pos: pos.to_block_pos_floor(),
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

        let mut tick_broadcaster = self.get_tick_broadcaster();
        while tick_broadcaster.recv().await.is_ok() {
            let inventory_items = &self.menu().slots()[self.menu().player_slots_range()];
            if num_items_in_slots(inventory_items, item) > num_items {
                return Ok(());
            }
        }

        Err(NoItemsError)
    }
}

/// Checks whether the block at the given position can be mined.
/// Returns an error if the block is air, not breakable, or not reachable.
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
    let block_hit_result = match pick(&look_direction, &eye_pos, chunks, actual_pick_range) {
        HitResult::Block(block_hit_result) => block_hit_result,
        // there is an entity in the way
        HitResult::Entity => return Err(MiningError::EntityBlocking),
    };

    if !(block_hit_result.block_pos == *pos) {
        return Err(MiningError::BlockIsNotReachable);
    }

    Ok(())
}

async fn mine_blocks_with_best_tool_unless_already_mined(
    bot: &Client,
    blocks_pos: &[BlockPos],
) -> Result<(), CantMineAnyError> {
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

        match bot.mine_block_with_best_tool(block_pos).await {
            Ok(_) => {
                return Ok(());
            }
            Err(_) => {
                continue;
            }
        }
    }

    Err(CantMineAnyError)
}

#[derive(Debug, Error)]
pub enum MiningError {
    #[error("Block is air")]
    BlockIsAir,
    #[error("Block is not breakable")]
    BlockIsNotBreakable,
    #[error("Block is not reachable")]
    BlockIsNotReachable,
    #[error("there is an entity blocking the block")]
    EntityBlocking,
}

#[derive(Debug)]
pub struct CantMineAnyError;
impl Display for CantMineAnyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cant mine any blocks requested")
    }
}
impl Error for CantMineAnyError {}

#[derive(Debug)]
pub struct NoItemsError;
impl Display for NoItemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No items found")
    }
}
impl Error for NoItemsError {}
