use std::ops::{Deref, DerefMut};

use azalea::entity::metadata::AbstractMonster;
use azalea::entity::{self, Dead, LocalEntity, Position};
use azalea::world::{InstanceName, MinecraftEntityId};
use azalea::{GameProfileComponent, registry};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

/// A single entity target. This can be a specific entity, a player name, or a
/// entity type.
#[derive(Debug, Clone)]
pub enum EntityTarget {
    EntityKind(registry::EntityKind),
    EntityId(MinecraftEntityId),
    PlayerName(String),
    AllMonsters,
    AllPlayers,
}

/// A collection of entity targets. This is used to find entities that match
/// the given targets.
#[derive(Debug, Default, Clone)]
pub struct EntityTargets(Vec<EntityTarget>);

impl EntityTargets {
    pub fn new(targets: &[EntityTarget]) -> Self {
        Self(targets.to_vec())
    }
}

impl Deref for EntityTargets {
    type Target = Vec<EntityTarget>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EntityTargets {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

type TargetsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static entity::EntityKind>,
        Option<&'static MinecraftEntityId>,
        Option<&'static GameProfileComponent>,
        Option<&'static AbstractMonster>,
    ),
    (With<MinecraftEntityId>, Without<Dead>, Without<LocalEntity>),
>;

/// Checks if the given entity is in the targets list.
/// Returns true if the entity is in the targets list, false otherwise.
fn is_entity_in_targets(entity: &Entity, targets: &EntityTargets, query: &TargetsQuery) -> bool {
    for target in targets.iter() {
        let Ok((_entity, entity_kind, entity_id, game_profile, monster)) = query.get(*entity)
        else {
            return false;
        };

        match target {
            EntityTarget::EntityKind(kind) => {
                if let Some(entity_kind) = entity_kind {
                    if **entity_kind == *kind {
                        return true;
                    }
                }
            }
            EntityTarget::EntityId(id) => {
                if let Some(entity_id) = entity_id {
                    if entity_id == id {
                        return true;
                    }
                }
            }
            EntityTarget::PlayerName(name) => {
                if let Some(game_profile) = game_profile {
                    if game_profile.name == *name {
                        return true;
                    }
                }
            }
            EntityTarget::AllMonsters => {
                if let Some(_monster) = monster {
                    return true;
                }
            }
            EntityTarget::AllPlayers => {
                if let Some(_game_profile) = game_profile {
                    return true;
                }
            }
        }
    }

    false
}

/// This system parameter can be used as a to find [`EntityTarget`]s close to a given position.
///
/// ref: [`EntityFinder`](azalea::nearest_entity::EntityFinder)
#[derive(SystemParam)]
pub struct TargetFinder<'w, 's> {
    all_entities:
        Query<'w, 's, (&'static Position, &'static InstanceName), With<MinecraftEntityId>>,

    target_entities: TargetsQuery<'w, 's>,

    filtered_entities:
        Query<'w, 's, (Entity, &'static InstanceName, &'static Position), With<MinecraftEntityId>>,
}

impl<'a> TargetFinder<'_, '_> {
    /// Gets the nearest entity to the given position and world instance name.
    /// This method will return `None` if there are no entities within range. If
    /// multiple entities are within range, only the closest one is returned.
    pub fn nearest_to_position(
        &'a self,
        position: &Position,
        instance_name: &InstanceName,
        targets: &EntityTargets,
        max_distance: f64,
    ) -> Option<Entity> {
        let mut nearest_entity = None;
        let mut min_distance = max_distance;

        for (target_entity, e_instance, e_pos) in self.filtered_entities.iter() {
            if e_instance != instance_name {
                continue;
            }

            if !is_entity_in_targets(&target_entity, targets, &self.target_entities) {
                continue;
            }

            let target_distance = position.distance_to(e_pos);
            if target_distance < min_distance {
                nearest_entity = Some(target_entity);
                min_distance = target_distance;
            }
        }

        nearest_entity
    }

    /// Gets the nearest entity to the given entity. This method will return
    /// `None` if there are no entities within range. If multiple entities are
    /// within range, only the closest one is returned.
    pub fn nearest_to_entity(
        &'a self,
        entity: Entity,
        targets: &EntityTargets,
        max_distance: f64,
    ) -> Option<Entity> {
        let Ok((position, instance_name)) = self.all_entities.get(entity) else {
            return None;
        };

        let mut nearest_entity = None;
        let mut min_distance = max_distance;

        for (target_entity, e_instance, e_pos) in self.filtered_entities.iter() {
            if entity == target_entity {
                continue;
            };

            if e_instance != instance_name {
                continue;
            }

            if !is_entity_in_targets(&target_entity, targets, &self.target_entities) {
                continue;
            }

            let target_distance = position.distance_to(e_pos);
            if target_distance < min_distance {
                nearest_entity = Some(target_entity);
                min_distance = target_distance;
            }
        }

        nearest_entity
    }

    /// This function get an iterator over all nearby entities to the given
    /// position within the given maximum distance. The entities in this
    /// iterator are not returned in any specific order.
    ///
    /// This function returns the Entity ID of nearby entities and their
    /// distance away.
    pub fn nearby_entities_to_position(
        &'a self,
        position: &'a Position,
        instance_name: &'a InstanceName,
        targets: &'a EntityTargets,
        max_distance: f64,
    ) -> impl Iterator<Item = (Entity, f64)> + 'a {
        self.filtered_entities
            .iter()
            .filter_map(move |(target_entity, e_instance, e_pos)| {
                if e_instance != instance_name {
                    return None;
                }

                if !is_entity_in_targets(&target_entity, targets, &self.target_entities) {
                    return None;
                }

                let distance = position.distance_to(e_pos);
                if distance < max_distance {
                    Some((target_entity, distance))
                } else {
                    None
                }
            })
    }

    /// This function get an iterator over all nearby entities to the given
    /// entity within the given maximum distance. The entities in this iterator
    /// are not returned in any specific order.
    ///
    /// This function returns the Entity ID of nearby entities and their
    /// distance away.
    pub fn nearby_entities_to_entity(
        &'a self,
        entity: Entity,
        targets: &'a EntityTargets,
        max_distance: f64,
    ) -> impl Iterator<Item = (Entity, f64)> + 'a {
        let position;
        let instance_name;
        if let Ok((pos, instance)) = self.all_entities.get(entity) {
            position = *pos;
            instance_name = Some(instance);
        } else {
            position = Position::default();
            instance_name = None;
        };

        self.filtered_entities
            .iter()
            .filter_map(move |(target_entity, e_instance, e_pos)| {
                if entity == target_entity {
                    return None;
                }

                if Some(e_instance) != instance_name {
                    return None;
                }

                if !is_entity_in_targets(&target_entity, targets, &self.target_entities) {
                    return None;
                }

                let distance = position.distance_to(e_pos);
                if distance < max_distance {
                    Some((target_entity, distance))
                } else {
                    None
                }
            })
    }
}
