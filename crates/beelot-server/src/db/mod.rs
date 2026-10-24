//! MariaDB through sqlx: the pool, the embedded migrations, and one repository module per table.

pub mod accounts;
pub mod events;
pub mod passkeys;
pub mod sessions;

use sqlx::mysql::{
    MySql, MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlTypeInfo, MySqlValueRef,
};
use sqlx::types::uuid::fmt::Hyphenated;
use sqlx::{Decode, Type};
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};
use uuid::Uuid;

use crate::config::Config;

/// Connects to the configured database and brings its schema up to date.
pub async fn connect(config: &Config) -> anyhow::Result<MySqlPool> {
    let options: MySqlConnectOptions = config.database_url.parse()?;
    let options = options
        .username(&config.database_username)
        .password(&config.database_password);
    let pool = MySqlPoolOptions::new().connect_with(options).await?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::migrate!().run(pool).await?;
    Ok(())
}

/// A time as the database keeps it: `DATETIME(6)` in UTC.
pub fn utc(time: OffsetDateTime) -> PrimitiveDateTime {
    let time = time.to_offset(UtcOffset::UTC);
    PrimitiveDateTime::new(time.date(), time.time())
}

/// A time read back from the database.
pub fn from_utc(time: PrimitiveDateTime) -> OffsetDateTime {
    time.assume_utc()
}

/// MariaDB's `UUID` columns read and write the hyphenated text form.
pub fn id(uuid: Uuid) -> Hyphenated {
    uuid.hyphenated()
}

/// A `UUID` column as read back: MariaDB sends its text form, flagged as binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbUuid(pub Uuid);

impl Type<MySql> for DbUuid {
    fn type_info() -> MySqlTypeInfo {
        <&[u8] as Type<MySql>>::type_info()
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        <&[u8] as Type<MySql>>::compatible(ty) || <&str as Type<MySql>>::compatible(ty)
    }
}

impl<'r> Decode<'r, MySql> for DbUuid {
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let bytes = <&[u8] as Decode<MySql>>::decode(value)?;
        let uuid = match bytes.len() {
            16 => Uuid::from_slice(bytes)?,
            _ => Uuid::try_parse_ascii(bytes)?,
        };
        Ok(DbUuid(uuid))
    }
}
