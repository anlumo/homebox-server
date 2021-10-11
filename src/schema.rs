use std::{collections::HashMap, ops::DerefMut};

use anyhow::Error;
use async_graphql::{
    futures_util::{lock::MutexGuard, TryStreamExt},
    Context, EmptySubscription, Object, Schema, SimpleObject,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::MetadataDatabase;

pub type HomeboxSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub const CONTAINER_IMAGE_TYPE: u8 = 2;
pub const ITEM_IMAGE_TYPE: u8 = 11;
pub const SESSION_TYPE: u8 = 255;

pub struct QueryRoot;

impl QueryRoot {
    async fn fetch_container(
        mut db: MutexGuard<'_, sqlx::SqliteConnection>,
        id: Uuid,
    ) -> Result<Option<Container>, Error> {
        match sqlx::query!("SELECT * FROM containers WHERE uuid = ?", id)
            .fetch_one(db.deref_mut())
            .await
        {
            Ok(row) => {
                let location = if let Some(location) = row.location {
                    sqlx::query!("SELECT * FROM locations WHERE uuid = ?", location)
                        .fetch_one(db.deref_mut())
                        .await
                        .ok()
                } else {
                    None
                };
                Ok(Some(Container {
                    id: Uuid::from_slice(&row.uuid).unwrap(),
                    created: DateTime::from_utc(row.created, Utc),
                    updated: DateTime::from_utc(row.updated, Utc),
                    name: row.name,
                    location: location.map(|location| Location {
                        id: Uuid::from_slice(&location.uuid).unwrap(),
                        name: location.name,
                    }),
                }))
            }
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

#[Object]
impl QueryRoot {
    async fn all_locations(&self, ctx: &Context<'_>) -> Result<Vec<Location>, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let mut locations = sqlx::query!("SELECT * from locations").fetch(db.deref_mut());
        let mut result = Vec::new();
        while let Some(row) = locations.try_next().await? {
            result.push(Location {
                id: Uuid::from_slice(&row.uuid).unwrap(),
                name: row.name,
            });
        }
        Ok(result)
    }
    async fn all_containers(&self, ctx: &Context<'_>) -> Result<Vec<Container>, Error> {
        let locations: HashMap<_, _> = self
            .all_locations(ctx)
            .await?
            .into_iter()
            .map(|location| (location.id, location))
            .collect();
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let mut containers = sqlx::query!("SELECT * FROM containers").fetch(db.deref_mut());
        // let mut containers = sqlx::query!("SELECT c.uuid as uuid, c.created as created, c.updated as updated, c.name as name, l.uuid as location_uuid, l.name as location_name FROM containers as c LEFT JOIN locations as l ON (c.location = l.uuid)").fetch(&mut db);
        let mut result = Vec::new();
        while let Some(row) = containers.try_next().await? {
            result.push(Container {
                id: Uuid::from_slice(&row.uuid).unwrap(),
                created: DateTime::from_utc(row.created, Utc),
                updated: DateTime::from_utc(row.updated, Utc),
                name: row.name,
                location: row
                    .location
                    .and_then(|uuid| Uuid::from_slice(&uuid).ok())
                    .and_then(|uuid| locations.get(&uuid).cloned()),
            });
        }
        Ok(result)
    }
    async fn container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
    ) -> Result<Option<Container>, Error> {
        Self::fetch_container(ctx.data_unchecked::<MetadataDatabase>().lock().await, id).await
    }
    async fn items_in_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
    ) -> Result<Vec<Item>, Error> {
        if let Some(container) = self.container(ctx, id).await? {
            let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
            let mut items =
                sqlx::query!("SELECT * FROM items WHERE container = ?", id).fetch(db.deref_mut());
            let mut result = Vec::new();
            while let Some(row) = items.try_next().await? {
                result.push(Item {
                    id: Uuid::from_slice(&row.uuid).unwrap(),
                    created: DateTime::from_utc(row.created, Utc),
                    updated: DateTime::from_utc(row.updated, Utc),
                    name: row.name,
                    quantity: row.quantity as _,
                    description: row.description,
                    container: container.clone(),
                });
            }
            Ok(result)
        } else {
            Err(sqlx::Error::RowNotFound.into())
        }
    }
    async fn all_items(&self, ctx: &Context<'_>) -> Result<Vec<Item>, Error> {
        let containers: HashMap<_, _> = self
            .all_containers(ctx)
            .await?
            .into_iter()
            .map(|container| (container.id, container))
            .collect();
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let mut items = sqlx::query!("SELECT * FROM items").fetch(db.deref_mut());
        let mut result = Vec::new();
        while let Some(row) = items.try_next().await? {
            if let Some(container) = containers.get(&Uuid::from_slice(&row.container).unwrap()) {
                result.push(Item {
                    id: Uuid::from_slice(&row.uuid).unwrap(),
                    created: DateTime::from_utc(row.created, Utc),
                    updated: DateTime::from_utc(row.updated, Utc),
                    name: row.name,
                    quantity: row.quantity as _,
                    description: row.description,
                    container: container.clone(),
                });
            }
        }
        Ok(result)
    }
    async fn item(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of an item")] id: Uuid,
    ) -> Result<Option<Item>, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        match sqlx::query!("SELECT * FROM items WHERE uuid = ?", id)
            .fetch_one(db.deref_mut())
            .await
        {
            Ok(row) => {
                if let Some(container) =
                    Self::fetch_container(db, Uuid::from_slice(&row.container).unwrap()).await?
                {
                    Ok(Some(Item {
                        id: Uuid::from_slice(&row.uuid).unwrap(),
                        created: DateTime::from_utc(row.created, Utc),
                        updated: DateTime::from_utc(row.updated, Utc),
                        name: row.name,
                        quantity: row.quantity as _,
                        description: row.description,
                        container,
                    }))
                } else {
                    Err(sqlx::Error::RowNotFound.into())
                }
            }
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn add_location(&self, ctx: &Context<'_>, name: String) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        sqlx::query!(
            "INSERT INTO locations (uuid, name) VALUES (?, ?)",
            uuid,
            name
        )
        .execute(db.deref_mut())
        .await?;
        Ok(uuid)
    }
    async fn update_location(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        name: String,
    ) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let result = sqlx::query!("UPDATE locations SET name = ? WHERE uuid = ?", name, id)
            .execute(db.deref_mut())
            .await?;
        Ok(result.rows_affected() > 0)
    }
    async fn delete_location(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let result = sqlx::query!("DELETE FROM locations WHERE uuid = ?", id)
            .execute(db.deref_mut())
            .await?;
        if result.rows_affected() > 0 {
            let now = Utc::now();
            sqlx::query!(
                "UPDATE containers SET updated = ?, location = NULL WHERE location = ?",
                now,
                id
            )
            .execute(db.deref_mut())
            .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn add_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Name of the new container")] name: String,
        #[graphql(desc = "Physical location of container")] location: Uuid,
    ) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let now = Utc::now();
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        sqlx::query!("INSERT INTO containers (uuid, created, updated, name, location) VALUES (?, ?, ?, ?, ?)", uuid, now, now, name, location).execute(db.deref_mut()).await?;
        Ok(uuid)
    }
    async fn update_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
        #[graphql(desc = "New name")] name: String,
        #[graphql(desc = "New physical location")] location: Uuid,
    ) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE containers SET name = ?, location = ?, updated = ? WHERE uuid = ?",
            name,
            location,
            now,
            id
        )
        .execute(db.deref_mut())
        .await?;
        Ok(result.rows_affected() > 0)
    }
    async fn delete_container(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Primary key of a container")] id: Uuid,
    ) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let result = sqlx::query!("DELETE FROM containers WHERE uuid = ?", id)
            .execute(db.deref_mut())
            .await?;
        if result.rows_affected() > 0 {
            sqlx::query!("DELETE FROM items WHERE container = ?", id)
                .execute(db.deref_mut())
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn add_item(
        &self,
        ctx: &Context<'_>,
        container: Uuid,
        name: String,
        quantity: usize,
        description: Option<String>,
    ) -> Result<Uuid, Error> {
        let uuid = Uuid::new_v4();
        let now = Utc::now();
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let quantity = quantity as i64;
        sqlx::query!("INSERT INTO items (uuid, created, updated, name, description, quantity, container) VALUES (?, ?, ?, ?, ?, ?, ?)", uuid, now, now, name, description, quantity, container).execute(db.deref_mut()).await?;
        Ok(uuid)
    }
    async fn update_item(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        name: String,
        description: Option<String>,
        quantity: Option<usize>,
    ) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let quantity = quantity.map(|q| q as i64);
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE items SET updated = ?, name = ?, description = ?, quantity = ? WHERE uuid = ?",
            now,
            name,
            description,
            quantity,
            id
        )
        .execute(db.deref_mut())
        .await?;
        Ok(result.rows_affected() > 0)
    }
    async fn move_item(&self, ctx: &Context<'_>, id: Uuid, container: Uuid) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE items SET updated = ?, container = ? WHERE uuid = ?",
            now,
            container,
            id
        )
        .execute(db.deref_mut())
        .await?;
        Ok(result.rows_affected() > 0)
    }
    async fn delete_item(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool, Error> {
        let mut db = ctx.data_unchecked::<MetadataDatabase>().lock().await;
        let result = sqlx::query!("DELETE FROM items WHERE uuid = ?", id)
            .execute(db.deref_mut())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Location {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Container {
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub name: Option<String>,
    pub location: Option<Location>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Item {
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub name: String,
    pub quantity: usize,
    pub description: Option<String>,
    pub container: Container,
}
