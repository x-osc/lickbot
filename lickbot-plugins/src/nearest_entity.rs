use azalea::Client;
use azalea::entity::Position;
use azalea::entity::metadata::ItemItem;
use azalea::registry::Item;
use azalea::world::InstanceName;
use bevy_ecs::prelude::*;

pub trait NearestEntityClientExt {
    // fn nearest_item(&self, item: Item) -> Option<Entity>;

    /// Finds the nearest item of the specified type.
    /// Slow and unoptimized.
    fn nearest_items(&self, item: Item) -> impl Iterator<Item = Entity>;
    /// Finds the nearest item of the specified type within a certain distance.
    fn nearest_items_by_distance(
        &self,
        item: Item,
        max_distance: f64,
    ) -> impl Iterator<Item = Entity>;
}

impl NearestEntityClientExt for Client {
    // fn nearest_item(&self, item: Item) -> Option<Entity> {
    //     let client_position = self.eye_position();
    //     let client_instance_name = self.component::<InstanceName>();

    //     let mut item_query = self
    //         .ecs
    //         .lock()
    //         .query::<(Entity, &ItemItem, &Position, &InstanceName)>();

    //     let mut nearest_entity: Option<Entity> = None;
    //     let mut nearest_distance = f64::MAX;

    //     for (entity, item_component, position, instance_name) in item_query.iter(&self.ecs.lock()) {
    //         if instance_name != &client_instance_name {
    //             continue;
    //         }

    //         if item_component.kind() != item {
    //             continue;
    //         }

    //         let distance_sq = client_position.distance_squared_to(position);
    //         if distance_sq < nearest_distance {
    //             nearest_distance = distance_sq;
    //             nearest_entity = Some(entity);
    //         }
    //     }

    //     nearest_entity
    // }

    fn nearest_items(&self, item: Item) -> impl Iterator<Item = Entity> {
        self.nearest_items_by_distance(item, f64::INFINITY)
    }

    fn nearest_items_by_distance(
        &self,
        item: Item,
        max_distance: f64,
    ) -> impl Iterator<Item = Entity> {
        let client_instance_name = self.component::<InstanceName>();
        let client_position = self.eye_position();

        let mut item_query = self
            .ecs
            .lock()
            .query::<(Entity, &ItemItem, &Position, &InstanceName)>();

        let mut entities: Vec<_> = item_query
            .iter(&self.ecs.lock())
            .filter_map(|(entity, item_component, position, instance_name)| {
                if instance_name != &client_instance_name {
                    return None;
                }

                if item_component.kind() != item {
                    return None;
                }

                let distance_sq = client_position.distance_squared_to(**position);
                if distance_sq > max_distance * max_distance {
                    return None;
                }

                Some((entity, distance_sq))
            })
            .collect();

        // Sort by distance and truncate to the closest `max_results`
        entities.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        entities.into_iter().map(|(entity, _)| entity)
    }
}
