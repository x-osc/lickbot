use std::thread;
use std::time::Duration;

use anyhow::Result;
use azalea::prelude::*;
use azalea::swarm::prelude::*;
use tracing::info;

const USERNAMES: [&str; 1] = ["lickbot"];
const ADDRESS: &str = "localhost:60000";
const PATHFINDER_DEBUG_PARTICLES: bool = false;

#[derive(Debug, Component, Clone, Default)]
pub struct State {}

#[derive(Debug, Resource, Clone, Default)]
pub struct SwarmState {}

#[tokio::main]
async fn main() {
    thread::spawn(deadlock_detection_thread);

    let mut swarm = SwarmBuilder::new()
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
        Event::Death(death) => {
            info!("{} has died! Reason: ```{:?}```", bot.username(), death)
        }
        _ => {}
    }
    Ok(())
}

async fn swarm_handle(swarm: Swarm, event: SwarmEvent, state: SwarmState) -> Result<()> {
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
            if chat.message().to_string() == "The particle was not visible for anybody" {
                return Ok(());
            }
            println!("{}", chat.message().to_ansi())
        }
        _ => {}
    }

    Ok(())
}
