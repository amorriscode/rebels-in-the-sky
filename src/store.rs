use crate::{types::AppResult, world::world::World};
use directories;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use std::{fs::File, path::PathBuf};

pub static ASSETS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/");
pub static PERSISTED_WORLD_FILENAME: &str = "world.json";
pub static PERSISTED_GAMES_PREFIX: &str = "game_";

fn store_path(filename: &str) -> AppResult<PathBuf> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or("Failed to get directories")?;
    let config_dirs = dirs.config_dir();
    if !config_dirs.exists() {
        std::fs::create_dir_all(config_dirs)?;
    }
    let path = config_dirs.join(filename);
    Ok(path)
}

pub fn save_world(world: &World, with_backup: bool) -> AppResult<()> {
    let stored_world = world.to_store();
    save_to_json(PERSISTED_WORLD_FILENAME, &stored_world)?;
    if with_backup {
        let backup_filename = format!("{}.back", PERSISTED_WORLD_FILENAME);
        save_to_json(&backup_filename, &stored_world)?;
    }
    Ok(())
}

pub fn get_world_size() -> AppResult<u64> {
    let size = file_data(PERSISTED_WORLD_FILENAME)?.len();
    // let file_size = File::open(store_path(filename)?)?.metadata()?.len();
    Ok(size)
}

pub fn save_to_json<T: Serialize>(filename: &str, data: &T) -> AppResult<()> {
    let file = File::create(store_path(filename)?)?;
    assert!(file.metadata()?.is_file());
    let buffer = std::io::BufWriter::new(file);
    serde_json::to_writer(buffer, data)?;
    Ok(())
}

pub fn load_from_json<T: for<'a> Deserialize<'a>>(filename: &str) -> AppResult<T> {
    let file = File::open(store_path(filename)?)?;
    let data: T = serde_json::from_reader(file)?;
    Ok(data)
}

pub fn reset() -> AppResult<()> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or("Failed to get directories")?;
    let config_dirs = dirs.config_dir();
    if config_dirs.exists() {
        std::fs::remove_dir_all(config_dirs)?;
    }
    std::fs::create_dir_all(config_dirs)?;
    Ok(())
}

pub fn world_exists() -> bool {
    let path = store_path(&PERSISTED_WORLD_FILENAME);
    path.is_ok() && path.unwrap().exists()
}

pub fn file_data(filename: &str) -> AppResult<std::fs::Metadata> {
    let path = store_path(filename)?;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use crate::world::world::World;
    use directories;
    use std::fs::File;

    #[test]
    fn test_path() {
        let dirs = directories::ProjectDirs::from("org", "frittura", "puma");
        assert!(dirs.is_some());
        let dirs_ok = dirs.unwrap();
        let config_dirs = dirs_ok.config_dir();
        println!("{:?}", config_dirs);
        if !config_dirs.exists() {
            std::fs::create_dir_all(config_dirs).unwrap();
        }
        let path = config_dirs.join("test");
        let file = File::create(path.clone());
        assert!(file.is_ok());
        assert!(path.is_file());
        if config_dirs.exists() {
            std::fs::remove_dir_all(config_dirs).unwrap();
        }
    }

    #[test]
    fn test_save() {
        let world = World::new(None);
        let result = super::save_to_json("test", &world);
        assert!(result.is_ok());
    }
}
