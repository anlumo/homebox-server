use std::sync::Arc;

use anyhow::Error;
use async_graphql::{Context, EmptySubscription, Object, Schema, SimpleObject};
use chrono::{DateTime, Utc};
use rocksdb::{Direction, IteratorMode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Database;

pub type HomeboxSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub const CONTAINER_IMAGE_TYPE: u8 = 2;
pub const ITEM_IMAGE_TYPE: u8 = 11;
pub const SESSION_TYPE: u8 = 255;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn all_containers(&self, ctx: &Context<'_>) -> Result<Vec<Container>, Error> {
        ctx.data_unchecked::<Arc<Database>>()
            .iterator(IteratorMode::From(&[Container::TYPE], Direction::Forward))
            .take_while(|(key, _)| key[0] == Container::TYPE)
            .map(|(key, value)| {
                log::debug!("key: {:?}, container: {:?}", key, value);
                bson::from_slice(&value).map_err(|err| err.into())
            })
            .collect()
    }
    async fn container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
    ) -> Result<Option<Container>, Error> {
        let key: Vec<u8> = std::iter::once(Container::TYPE)
            .chain(id.as_bytes().iter().copied())
            .collect();
        ctx.data_unchecked::<Arc<Database>>()
            .get_pinned(key)?
            .map(|value| bson::from_slice(&value).map_err(|err| err.into()))
            .transpose()
    }
    async fn items_in_container(&self, ctx: &Context<'_>, id: Uuid) -> Result<Vec<Item>, Error> {
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(id.as_bytes().iter().copied())
            .collect();
        ctx.data_unchecked::<Arc<Database>>()
            .iterator(IteratorMode::From(&key, Direction::Forward))
            .take_while(|(key_iter, _)| key == key_iter[..key.len()])
            .map(|(_, value)| bson::from_slice(&value).map_err(|err| err.into()))
            .collect()
    }
    async fn item(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        container: Uuid,
    ) -> Result<Option<Item>, Error> {
        let db = ctx.data_unchecked::<Arc<Database>>();
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(container.as_bytes().iter().copied())
            .chain(id.as_bytes().iter().copied())
            .collect();
        db.get_pinned(key)?
            .map(|item| bson::from_slice(&item).map_err(|err| err.into()))
            .transpose()
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn add_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Name of the new container")] name: String,
        #[graphql(desc = "Physical location of container")] location: String,
    ) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let key: Vec<u8> = std::iter::once(Container::TYPE)
            .chain(uuid.as_bytes().iter().copied())
            .collect();
        let now = Utc::now();
        ctx.data_unchecked::<Arc<Database>>().put(
            key,
            bson::to_vec(&Container {
                id: uuid,
                created: now,
                updated: now,
                name,
                location,
            })?,
        )?;
        Ok(uuid)
    }
    async fn update_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
        #[graphql(desc = "New name")] name: Option<String>,
        #[graphql(desc = "New physical location")] location: Option<String>,
    ) -> Result<bool, Error> {
        let db = ctx.data_unchecked::<Arc<Database>>();
        let key: Vec<u8> = std::iter::once(Container::TYPE)
            .chain(id.as_bytes().iter().copied())
            .collect();
        if let Some(mut container) = db
            .get_pinned(&key)?
            .map(|value| bson::from_slice::<Container>(&value))
            .transpose()?
        {
            if let Some(name) = name {
                container.name = name;
            }
            if let Some(location) = location {
                container.location = location;
            }
            container.updated = Utc::now();
            db.put(key, bson::to_vec(&container)?)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    async fn delete_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
    ) -> Result<bool, Error> {
        let db = ctx.data_unchecked::<Arc<Database>>();
        let key: Vec<u8> = std::iter::once(Container::TYPE)
            .chain(id.as_bytes().iter().copied())
            .collect();
        db.delete(key)?;
        let key: Vec<u8> = std::iter::once(CONTAINER_IMAGE_TYPE)
            .chain(id.as_bytes().iter().copied())
            .collect();
        db.delete(key)?;
        Ok(true)
    }

    async fn add_item(
        &self,
        ctx: &Context<'_>,
        container: Uuid,
        name: String,
        quantity: usize,
        description: String,
    ) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(container.as_bytes().iter().copied())
            .chain(uuid.as_bytes().iter().copied())
            .collect();
        let now = Utc::now();
        ctx.data_unchecked::<Arc<Database>>().put(
            key,
            bson::to_vec(&Item {
                id: uuid,
                created: now,
                updated: now,
                name,
                quantity,
                description,
            })?,
        )?;
        Ok(uuid)
    }
    async fn update_item(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        container: Uuid,
        name: Option<String>,
        quantity: Option<usize>,
        description: Option<String>,
    ) -> Result<bool, Error> {
        let db = ctx.data_unchecked::<Arc<Database>>();
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(container.as_bytes().iter().copied())
            .chain(id.as_bytes().iter().copied())
            .collect();

        if let Some(mut item) = db
            .get_pinned(&key)?
            .map(|value| bson::from_slice::<Item>(&value))
            .transpose()?
        {
            if let Some(name) = name {
                item.name = name;
            }
            if let Some(quantity) = quantity {
                item.quantity = quantity;
            }
            if let Some(description) = description {
                item.description = description;
            }
            item.updated = Utc::now();
            db.put(key, bson::to_vec(&item)?)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    async fn move_item(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        from_container: Uuid,
        to_container: Uuid,
    ) -> Result<bool, Error> {
        let db = ctx.data_unchecked::<Arc<Database>>();
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(from_container.as_bytes().iter().copied())
            .chain(id.as_bytes().iter().copied())
            .collect();

        if let Some(mut item) = db
            .get_pinned(&key)?
            .map(|value| bson::from_slice::<Item>(&value))
            .transpose()?
        {
            item.updated = Utc::now();
            let to_key: Vec<u8> = std::iter::once(Item::TYPE)
                .chain(to_container.as_bytes().iter().copied())
                .chain(id.as_bytes().iter().copied())
                .collect();
            db.put(to_key, bson::to_vec(&item)?)?;
            db.delete(key)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    async fn delete_item(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        container: Uuid,
    ) -> Result<bool, Error> {
        let db = ctx.data_unchecked::<Arc<Database>>();
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(container.as_bytes().iter().copied())
            .chain(id.as_bytes().iter().copied())
            .collect();
        db.delete(key)?;
        let key: Vec<u8> = std::iter::once(ITEM_IMAGE_TYPE)
            .chain(container.as_bytes().iter().copied())
            .chain(id.as_bytes().iter().copied())
            .collect();
        db.delete(key)?;
        Ok(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Container {
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub name: String,
    pub location: String,
}

impl Container {
    pub const TYPE: u8 = 0;
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Item {
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub name: String,
    pub quantity: usize,
    pub description: String,
}

impl Item {
    pub const TYPE: u8 = 10;
}
