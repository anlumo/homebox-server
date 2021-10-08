use anyhow::Error;
use async_graphql::{Context, EmptySubscription, Object, Schema, SimpleObject};
use chrono::{DateTime, Utc};
use rocksdb::{Direction, IteratorMode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Database;

pub type HomeboxSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn all_containers(&self, ctx: &Context<'_>) -> Result<Vec<Container>, Error> {
        ctx.data_unchecked::<Database>()
            .iterator(IteratorMode::From(&[Container::TYPE], Direction::Forward))
            .take_while(|(key, _)| key[0] == Container::TYPE)
            .map(|(_, value)| bson::from_slice(&value).map_err(|err| err.into()))
            .collect()
    }
    async fn container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: String,
    ) -> Result<Option<Container>, Error> {
        let uuid: Uuid = id.parse()?;
        let key: Vec<u8> = std::iter::once(Container::TYPE)
            .chain(uuid.as_bytes().iter().copied())
            .collect();
        ctx.data_unchecked::<Database>()
            .get(key)?
            .map(|value| bson::from_slice(&value).map_err(|err| err.into()))
            .transpose()
    }
    async fn items_in_container(&self, ctx: &Context<'_>, id: String) -> Result<Vec<Item>, Error> {
        let uuid: Uuid = id.parse()?;
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(uuid.as_bytes().iter().copied())
            .collect();
        ctx.data_unchecked::<Database>()
            .iterator(IteratorMode::From(&key, Direction::Forward))
            .take_while(|(key, _)| key[0] == Item::TYPE && &key[1..17] == uuid.as_bytes())
            .map(|(_, value)| bson::from_slice(&value).map_err(|err| err.into()))
            .collect()
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn add_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Name of the new container")] name: String,
    ) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let key: Vec<u8> = std::iter::once(Container::TYPE)
            .chain(uuid.as_bytes().iter().copied())
            .collect();
        let now = Utc::now();
        ctx.data_unchecked::<Database>().put(
            key,
            bson::to_vec(&Container {
                id: uuid,
                created: now,
                updated: now,
                name,
            })?,
        )?;
        Ok(uuid)
    }
    async fn add_item(
        &self,
        ctx: &Context<'_>,
        container: String,
        name: String,
        quantity: usize,
        description: String,
    ) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let container_uuid: Uuid = container.parse()?;
        let key: Vec<u8> = std::iter::once(Item::TYPE)
            .chain(container_uuid.as_bytes().iter().copied())
            .chain(uuid.as_bytes().iter().copied())
            .collect();
        let now = Utc::now();
        ctx.data_unchecked::<Database>().put(
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
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Container {
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub name: String,
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
