use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use azalea::pathfinder::GotoEvent;
use azalea::pathfinder::astar::PathfinderTimeout;
use azalea::pathfinder::goals::{BlockPosGoal, Goal, XZGoal, YGoal};
use azalea::pathfinder::moves::default_move;
use azalea::registry::EntityKind;
use azalea::swarm::prelude::*;
use azalea::{BlockPos, prelude::*};
use azalea::{chat::ChatPacket, entity::Position};
use plugins::modules::auto_eat::AutoEatPlugin;
use plugins::modules::auto_look::{self, AutoLookPlugin};
use plugins::modules::auto_totem::{self, AutoTotemPlugin};
use plugins::modules::kill_aura::{AutoKillClientExt, AutoKillPlugin};
use plugins::utils::entity_target::{EntityTarget, EntityTargets};
use tracing::{error, info};

const USERNAMES: [&str; 1] = ["lickbot"];
const ADDRESS: &str = "localhost:25555";
const PATHFINDER_DEBUG_PARTICLES: bool = true;

#[derive(Debug, Component, Clone, Default)]
pub struct State {}

#[derive(Debug, Resource, Clone, Default)]
pub struct SwarmState {}

#[tokio::main]
async fn main() {
    thread::spawn(deadlock_detection_thread);

    let mut swarm = SwarmBuilder::new()
        .add_plugins(AutoLookPlugin)
        .add_plugins(AutoTotemPlugin)
        .add_plugins(AutoKillPlugin)
        // .add_plugins(AutoEatPlugin)
        .set_handler(handle)
        .set_swarm_handler(swarm_handle)
        .join_delay(Duration::from_secs(5));

    for name in USERNAMES {
        let account = Account::offline(name);
        swarm = swarm.add_account_with_state(account, State::default());
    }

    swarm.start(ADDRESS).await.unwrap();
}

/// Runs a loop that checks for deadlocks every 10 seconds.
///
/// Note that this requires the `deadlock_detection` parking_lot feature to be
/// enabled, which is only enabled in azalea by default when running in debug
/// mode.
fn deadlock_detection_thread() {
    loop {
        thread::sleep(Duration::from_secs(10));
        let deadlocks = parking_lot::deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        println!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            println!("Deadlock #{i}");
            for t in threads {
                println!("Thread Id {:#?}", t.thread_id());
                println!("{:#?}", t.backtrace());
            }
        }
    }
}

async fn handle(bot: Client, event: Event, state: State) -> Result<()> {
    match &event {
        Event::Init => {
            bot.set_client_information(azalea::ClientInformation {
                view_distance: 32,
                ..Default::default()
            })
            .await?;
            if PATHFINDER_DEBUG_PARTICLES {
                bot.ecs
                    .lock()
                    .entity_mut(bot.entity)
                    .insert(azalea::pathfinder::PathfinderDebugParticles);
            }
        }
        Event::Spawn => {
            info!("{} has logged in to world", bot.username());
            bot.ecs
                .lock()
                .entity_mut(bot.entity)
                .insert(auto_totem::AutoTotem);
            bot.ecs
                .lock()
                .entity_mut(bot.entity)
                .insert(auto_look::AutoLook);
        }
        Event::Chat(chat) => handle_chat(bot, state, chat).await?,
        Event::Death(death) => {
            info!("{} has died! Reason: ```{:?}```", bot.username(), death)
        }
        _ => {}
    }
    Ok(())
}

async fn swarm_handle(swarm: Swarm, event: SwarmEvent, _state: SwarmState) -> Result<()> {
    match &event {
        SwarmEvent::Disconnect(account, join_opts) => {
            info!(
                "{} got disconnected! Reconnecting in 5 seconds",
                account.username
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
            swarm
                .add_and_retry_forever_with_opts(account, State::default(), join_opts)
                .await;
        }
        SwarmEvent::Chat(chat) => {
            if [
                "The particle was not visible for anybody",
                "Displaying particle minecraft:dust",
            ]
            .contains(&chat.message().to_string().as_str())
            {
                return Ok(());
            }
            println!("{}", chat.message().to_ansi())
        }
        _ => {}
    }

    Ok(())
}

async fn handle_chat(bot: Client, _state: State, chat: &ChatPacket) -> Result<()> {
    let content = chat.content();

    let parts: Vec<&str> = content.split_whitespace().collect();

    match *parts.as_slice().first().unwrap_or(&"") {
        "!ping" => {
            bot.chat("pong!");
        }
        "!health" => {
            let health = bot.health();
            bot.chat(&format!("health: {}", health));
        }
        "!hunger" => {
            let hunger = bot.hunger();
            bot.chat(&format!(
                "hunger: {}, saturation: {}",
                hunger.food, hunger.saturation
            ));
        }
        "!pos" => {
            let pos = bot.position();
            bot.chat(&format!("x: {}, y: {}, z: {}", pos.x, pos.y, pos.z));
        }
        "!goto" => {
            let goal: Arc<dyn Goal>;

            match parts.len() {
                1 => {
                    let error_fn = || {
                        error!("Got !goto, could not find sender");
                        anyhow::anyhow!("could not find message sender")
                    };
                    let uuid = chat.sender_uuid().ok_or_else(error_fn)?;
                    let entity = bot.entity_by_uuid(uuid).ok_or_else(error_fn)?;
                    let position = bot
                        .get_entity_component::<Position>(entity)
                        .ok_or_else(error_fn)?;

                    goal = Arc::new(BlockPosGoal(position.into()));

                    info!(
                        "going to location of {}",
                        chat.sender().ok_or_else(error_fn)?
                    );
                }
                2 => {
                    let y: i32 = parts[1].parse()?;
                    goal = Arc::new(YGoal { y });
                }
                3 => {
                    let x: i32 = parts[1].parse()?;
                    let z: i32 = parts[2].parse()?;
                    goal = Arc::new(XZGoal { x, z });
                }
                4 => {
                    let x: i32 = parts[1].parse()?;
                    let y: i32 = parts[2].parse()?;
                    let z: i32 = parts[3].parse()?;
                    goal = Arc::new(BlockPosGoal(BlockPos::new(x, y, z)));
                }
                _ => {
                    info!("Invalid number of arguments for !goto command");
                    return Err(anyhow!("Invalid number of arguments for !goto command"));
                }
            }

            bot.ecs.lock().send_event(GotoEvent {
                entity: bot.entity,
                goal,
                successors_fn: default_move,
                allow_mining: true,
                min_timeout: PathfinderTimeout::Time(Duration::from_secs(2)),
                max_timeout: PathfinderTimeout::Time(Duration::from_secs(10)),
            });
        }
        "!killaura" => match parts.get(1) {
            Some(&"on") => {
                let target = match parts.get(2) {
                    Some(&"hostile") => EntityTarget::AllMonsters,
                    Some(&"players") => EntityTarget::AllPlayers,
                    Some(&"entity") => {
                        let entity_name = parts.get(3).ok_or_else(|| {
                            error!("!killaura entity requires an entity name");
                            anyhow!("!killaura entity requires an entity name")
                        })?;
                        EntityTarget::EntityKind(
                            EntityKind::from_str(&("minecraft:".to_owned() + *entity_name))
                                .map_err(|_| {
                                    error!("Invalid entity name: {}", entity_name);
                                    anyhow!("Invalid entity name: {}", entity_name)
                                })?,
                        )
                    }
                    Some(&"player") => {
                        let player_name = parts.get(3).ok_or_else(|| {
                            error!("!killaura player requires a player name");
                            anyhow!("!killaura player requires a player name")
                        })?;
                        EntityTarget::PlayerName(player_name.to_string())
                    }
                    _ => {
                        info!("Invalid arguments for !killaura command");
                        return Err(anyhow!("Invalid arguments for !killaura command"));
                    }
                };

                info!("killaura enabled for target {:?}!", &target);
                bot.enable_auto_kill(EntityTargets::new(&[target]));
            }
            Some(&"off") => {
                bot.disable_auto_kill();
                info!("killaura disabled!");
            }
            _ => {
                info!("Invalid arguments for !killaura command");
                return Err(anyhow!("Invalid arguments for !killaura command"));
            }
        },
        _ => {}
    };

    Ok(())
}
