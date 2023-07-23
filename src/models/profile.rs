#![allow(unused)]
use crate::http::{Error, Result, ResultExt};
use anyhow::Context;
use argon2::{password_hash::SaltString, Argon2, PasswordHash};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct Profile {
    pub username: String,
    pub bio: String,
    pub image: Option<String>,
    pub following: bool,
}

#[derive(Clone)]
pub struct ProfileController {
    pool: PgPool,
}

impl ProfileController {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ProfileController {
    pub async fn get_profile_by_id(&self, user_id: Option<Uuid>, username: &str) -> Result<Profile> {
        let profile = sqlx::query_as!(
            Profile,
            r#"
                select
                    username,
                    bio,
                    image,
                    exists(
                        select 1 from follow 
                        where followed_user_id = "user".user_id and following_user_id = $2
                    ) "following!" -- This tells SQLx that this column will never be null
                from "user"
                where username = $1
            "#,
            username,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::NotFound)?;

        Ok(profile)
    }

    /// Follow a user identified by `username`.
    pub async fn create_follow(&self, follower: &Uuid, following: &str) -> Result<Profile> {
        let mut tx = self.pool.begin().await?;
        
        let user = sqlx::query!(
            r#"select user_id, username, bio, image from "user" where username = $1"#,
            following
        )
        .fetch_optional(&mut tx)
        .await?
        .ok_or(Error::NotFound)?;

        let result = sqlx::query!(
            "insert into follow(following_user_id, followed_user_id) values ($1, $2) \
             on conflict do nothing", // If the row already exists, we don't need to do anything.
            follower,
            user.user_id
        )
        .execute(&mut tx)
        .await
        .on_constraint("user_cannot_follow_self", |_| Error::Forbidden)?;
        
        tx.commit().await?;

        Ok(Profile {
            username: user.username,
            bio: user.bio,
            image: user.image,
            following: true,
        })
    }

    pub async fn unfollow(&self, follower: &Uuid, following: &str) -> Result<Profile> {
        let mut tx = self.pool.begin().await?;

        let user = sqlx::query!(
            r#"select user_id, username, bio, image from "user" where username = $1"#,
            following
        )
        .fetch_optional(&mut tx)
        .await?
        .ok_or(Error::NotFound)?;

        sqlx::query!(
            "delete from follow where following_user_id = $1 and followed_user_id = $2",
            follower,
            user.user_id
        )
        .execute(&mut tx)
        .await?;

        tx.commit().await?;

        Ok(Profile {
            username: user.username,
            bio: user.bio,
            image: user.image,
            // We just made sure of this.
            following: false,
        })
    }
}
