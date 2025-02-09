use crate::{
    app::App,
    game_engine::{
        game::{Game, GameSummary},
        types::GameStatsMap,
    },
    world::{
        constants::*, kartoffel::Kartoffel, planet::Planet, player::Player, resources::Resource,
        team::Team,
    },
};
use anyhow::anyhow;
use chrono::{prelude::DateTime, Datelike, Local, Timelike};
use itertools::Itertools;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// A Tick represents a unit of time in the game world.
// It corresponds to a millisecond in the real world.
pub type Tick = u128;

pub type PlayerId = uuid::Uuid;
pub type TeamId = uuid::Uuid;
pub type PlanetId = uuid::Uuid;
pub type GameId = uuid::Uuid;
pub type KartoffelId = uuid::Uuid;

// pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub type AppResult<T> = Result<T, anyhow::Error>;
pub type AppCallback = Box<dyn Fn(&mut App) -> AppResult<Option<String>>>;

pub type PlayerMap = HashMap<PlayerId, Player>;
pub type TeamMap = HashMap<TeamId, Team>;
pub type PlanetMap = HashMap<PlanetId, Planet>;
pub type GameMap = HashMap<GameId, Game>;
pub type GameSummaryMap = HashMap<GameId, GameSummary>;
pub type KartoffelMap = HashMap<KartoffelId, Kartoffel>;
pub type ResourceMap = HashMap<Resource, u32>;

pub trait StorableResourceMap {
    fn value(&self, resource: &Resource) -> u32;
    fn used_storage_capacity(&self) -> u32;
    fn update(&mut self, resource: Resource, amount: i32, max_capacity: u32) -> AppResult<()>;
    fn add(&mut self, resource: Resource, amount: u32, max_capacity: u32) -> AppResult<()>;
    fn saturating_add(&mut self, resource: Resource, amount: u32, max_capacity: u32);
    fn sub(&mut self, resource: Resource, amount: u32) -> AppResult<()>;
    fn saturating_sub(&mut self, resource: Resource, amount: u32);
}
impl StorableResourceMap for ResourceMap {
    fn value(&self, resource: &Resource) -> u32 {
        self.get(resource).copied().unwrap_or_default()
    }

    fn used_storage_capacity(&self) -> u32 {
        self.iter().map(|(k, v)| k.to_storing_space() * v).sum()
    }

    fn update(&mut self, resource: Resource, amount: i32, max_capacity: u32) -> AppResult<()> {
        if amount > 0 {
            self.add(resource, amount as u32, max_capacity)?;
        } else if amount < 0 {
            self.sub(resource, (-amount) as u32)?;
        }

        Ok(())
    }

    fn add(&mut self, resource: Resource, amount: u32, max_capacity: u32) -> AppResult<()> {
        if self.used_storage_capacity() + resource.to_storing_space() * amount > max_capacity {
            log::info!(
                "Adding {} {} used is {}, adding extra {}, max is {}",
                amount,
                resource,
                self.used_storage_capacity(),
                resource.to_storing_space() * amount,
                max_capacity
            );
            return Err(anyhow!("Not enough storage to add resource"));
        }

        self.entry(resource)
            .and_modify(|e| {
                *e = e.saturating_add(amount);
            })
            .or_insert(amount);

        Ok(())
    }

    fn saturating_add(&mut self, resource: Resource, amount: u32, max_capacity: u32) {
        let max_amount = if resource.to_storing_space() == 0 {
            amount
        } else {
            amount.min((max_capacity - self.used_storage_capacity()) / resource.to_storing_space())
        };

        self.entry(resource)
            .and_modify(|e| {
                *e = e.saturating_add(max_amount);
            })
            .or_insert(max_amount);
    }

    fn sub(&mut self, resource: Resource, amount: u32) -> AppResult<()> {
        let current = self.get(&resource).copied().unwrap_or_default();
        if current < amount {
            return Err(anyhow!("Not enough resources to remove"));
        }

        self.entry(resource)
            .and_modify(|e| {
                *e = e.saturating_sub(amount);
            })
            .or_insert(0);
        Ok(())
    }

    fn saturating_sub(&mut self, resource: Resource, amount: u32) {
        self.entry(resource)
            .and_modify(|e| {
                *e = e.saturating_sub(amount);
            })
            .or_insert(0);
    }
}

pub trait SortablePlayerMap {
    fn by_position(&self, stats: &GameStatsMap) -> Vec<&Player>;
    fn by_total_skills(&self) -> Vec<&Player>;
}

impl SortablePlayerMap for PlayerMap {
    fn by_position(&self, stats: &GameStatsMap) -> Vec<&Player> {
        let bench = self
            .values()
            .filter(|&p| !stats[&p.id].is_playing())
            .sorted_by(|&a, &b| a.id.cmp(&b.id))
            .collect::<Vec<&Player>>();

        let starters = self
            .values()
            .filter(|&p| stats[&p.id].is_playing() && stats[&p.id].position.is_some())
            .sorted_by(|&a, &b| {
                stats[&a.id]
                    .position
                    .unwrap()
                    .cmp(&stats[&b.id].position.unwrap())
            })
            .collect::<Vec<&Player>>();
        let mut players = starters;
        players.extend(bench);
        players
    }
    fn by_total_skills(&self) -> Vec<&Player> {
        let mut players = self.values().collect::<Vec<&Player>>();
        players.sort_by(|&a, &b| {
            b.average_skill()
                .partial_cmp(&a.average_skill())
                .expect("Skill value should exist")
        });
        players
    }
}

pub trait SystemTimeTick {
    fn now() -> Self;
    fn from_system_time(time: SystemTime) -> Self;
    fn as_secs(&self) -> Tick;
    fn as_minutes(&self) -> Tick;
    fn as_hours(&self) -> Tick;
    fn as_days(&self) -> Tick;
    fn as_system_time(&self) -> SystemTime;
    fn formatted_as_time(&self) -> String;
    fn formatted_as_date(&self) -> String;
    fn formatted(&self) -> String;
}

impl SystemTimeTick for Tick {
    fn now() -> Self {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Invalid system time")
            .as_millis()
    }

    fn from_system_time(time: SystemTime) -> Tick {
        time.duration_since(UNIX_EPOCH)
            .expect("Invalid system time")
            .as_millis()
    }

    fn as_secs(&self) -> Tick {
        self / SECONDS
    }

    fn as_minutes(&self) -> Tick {
        self / MINUTES
    }

    fn as_hours(&self) -> Tick {
        self / HOURS
    }

    fn as_days(&self) -> Tick {
        self / DAYS
    }

    fn as_system_time(&self) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_millis(*self as u64)
    }

    fn formatted_as_date(&self) -> String {
        let dt: DateTime<Local> = self.as_system_time().into();
        format!(
            "{}/{}/{} {:02}:{:02}:{:02}",
            dt.day(),
            dt.month(),
            dt.year() + CALENDAR_OFFSET,
            dt.hour(),
            dt.minute(),
            dt.second()
        )
    }

    fn formatted_as_time(&self) -> String {
        let dt: DateTime<Local> = self.as_system_time().into();
        format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second())
    }

    fn formatted(&self) -> String {
        let seconds = self.as_secs() % 60;
        let minutes = (self.as_minutes() as f32) as u128 % 60;
        let hours = (self.as_hours() as f32) as u128 % 24;
        let days = (self.as_secs() as f32 / 60.0 / 60.0 / 24.0) as u128 % 365;
        let years = (self.as_secs() as f32 / 60.0 / 60.0 / 24.0 / 365.2425) as u128;

        if years > 0 {
            format!(
                "{}y {}d {:02}:{:02}:{:02}",
                years, days, hours, minutes, seconds
            )
        } else if days > 0 {
            format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        }
    }
}

// Write tests here
#[cfg(test)]
mod tests {
    use crate::types::{SystemTimeTick, Tick, SECONDS};

    #[test]
    fn test_system_time_conversion() {
        let now = Tick::now();
        let now_as_system_time = now.as_system_time();
        let now_as_tick = Tick::from_system_time(now_as_system_time);
        assert_eq!(now, now_as_tick);
    }

    #[test]
    fn test_formatted_as_time() {
        let time = 10 * SECONDS;
        let formatted = time.formatted();
        assert_eq!(formatted, "00:00:10");
    }
}
