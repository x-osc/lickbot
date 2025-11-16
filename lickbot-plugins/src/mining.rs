use std::error::Error;
use std::fmt::Display;

use azalea::auto_tool::AutoToolClientExt;
use azalea::blocks::BlockTrait;
use azalea::bot::{BotClientExt, direction_looking_at};
use azalea::ecs::prelude::*;
use azalea::entity::Position;
use azalea::interact::pick::pick_block;
use azalea::pathfinder::PathfinderOpts;
use azalea::pathfinder::goals::OrGoals;
use azalea::prelude::PathfinderClientExt;
use azalea::registry::Item;
use azalea::world::ChunkStorage;
use azalea::{BlockPos, Client, Vec3};
use thiserror::Error;
use tokio::sync::broadcast::error::RecvError;
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
    fn pick_up_item(
        &self,
        item: Item,
    ) -> impl std::future::Future<Output = Result<(), NoItemsError>> + Send;
}

impl MiningExtrasClientExt for Client {
    async fn mine_block_with_best_tool(&self, pos: &BlockPos) -> Result<(), MiningError> {
        can_mine_block(pos, self.eye_position(), &self.world().read().chunks)?;

        self.look_at(pos.center());
        self.mine_with_auto_tool(*pos).await;

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
        self.start_goto_with_opts(goal, PathfinderOpts::new());
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
        self.start_goto_with_opts(goal, PathfinderOpts::new());
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
        self.start_goto_with_opts(goal, PathfinderOpts::new());
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

    async fn pick_up_item(&self, item: Item) -> Result<(), NoItemsError> {
        // recalculate the nearest item and send a goto event to the location
        // returns the entities and positions for the items
        fn recalculate_and_send_path(
            bot: &Client,
            item: Item,
        ) -> Result<(Vec<Entity>, Vec<Vec3>), NoItemsError> {
            let nearest_items: Vec<Entity> =
                bot.nearest_items_by_distance(item, 20.).take(5).collect();
            let nearest_positions: Vec<_> = nearest_items
                .iter()
                .map(|entity| **bot.ecs.lock().get::<Position>(*entity).unwrap())
                .collect();

            if nearest_positions.is_empty() {
                return Err(NoItemsError);
            }

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

            bot.start_goto_with_opts(goal, PathfinderOpts::new());

            Ok((nearest_items, nearest_positions))
        }

        // update needs to be here too to make sure ecs is not broken
        self.wait_updates(2).await;

        let (mut prev_entities, mut prev_positions) = recalculate_and_send_path(self, item)?;

        let inventory_items = &self.menu().slots()[self.menu().player_slots_range()];
        let starting_num_items = num_items_in_slots(inventory_items, item);
        debug!("num_items: {}", starting_num_items);

        self.wait_updates(2).await;

        let mut tick_broadcaster = self.get_tick_broadcaster();
        loop {
            // every tick
            match tick_broadcaster.recv().await {
                Ok(_) => (),
                Err(RecvError::Closed) => {
                    warn!("tick broadcaster closed");
                    return Ok(());
                }
                Err(err) => {
                    warn!("{err}");
                    return Ok(());
                }
            };

            // if we pick up an item, done
            let inventory_items = &self.menu().slots()[self.menu().player_slots_range()];
            if num_items_in_slots(inventory_items, item) > starting_num_items {
                self.stop_pathfinding();
                return Ok(());
            }

            // if path is completed, uhh what
            // return with warning
            if self.is_goto_target_reached() {
                warn!("goto target reached, but no items picked up");
                return Ok(());
            }

            let nearest_items: Vec<Entity> =
                self.nearest_items_by_distance(item, 20.).take(5).collect();
            let nearest_positions: Vec<_> = nearest_items
                .iter()
                .map(|entity| **self.ecs.lock().get::<Position>(*entity).unwrap())
                .collect();

            if nearest_items.is_empty() {
                return Err(NoItemsError);
            }

            // check if any of the items were removed
            let missing = prev_entities
                .iter()
                .any(|prev_entity| !nearest_items.contains(prev_entity));

            // check if any of the items moved
            let moved =
                nearest_positions
                    .iter()
                    .zip(prev_positions.iter())
                    .any(|(new_pos, old_pos)| {
                        new_pos.to_block_pos_floor() != old_pos.to_block_pos_floor()
                    });

            if moved || missing {
                debug!("items moved, recalculating path");
                self.stop_pathfinding();
                // updates are required to make ecs not break and be dum dum
                self.wait_updates(1).await;
                (prev_entities, prev_positions) = recalculate_and_send_path(self, item)?;
                self.wait_updates(1).await;
            }
        }
    }
}

/// Checks whether the block at the given position can be mined.
/// Returns an error if the block is air, not breakable, or not reachable.
pub fn can_mine_block(
    pos: &BlockPos,
    eye_pos: Vec3,
    chunks: &ChunkStorage,
) -> Result<(), MiningError> {
    let block_state = chunks.get_block_state(*pos).unwrap_or_default();
    if block_state.is_air() {
        return Err(MiningError::BlockIsAir);
    }
    let block: Box<dyn BlockTrait> = block_state.into();
    if (*block).behavior().destroy_time < -1. {
        return Err(MiningError::BlockIsNotBreakable);
    }

    let max_pick_range = 6;
    let actual_pick_range = 3.5;

    let distance = pos.distance_squared_to(eye_pos.to_block_pos_ceil());
    if distance > max_pick_range * max_pick_range {
        return Err(MiningError::BlockIsNotReachable);
    }

    let look_direction = direction_looking_at(eye_pos, pos.center());
    let block_hit_result = pick_block(look_direction, eye_pos, chunks, actual_pick_range);

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
            .get_block_state(*block_pos)
            .unwrap_or_default();
        if block_state.is_air() {
            if block_pos.distance_squared_to(bot.position().to_block_pos_ceil()) < 4 * 4 {
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
