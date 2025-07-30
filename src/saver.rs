use std::env;
use std::fs;
use std::fs::File;
use std::io::prelude::*;

fn get_data_dir() -> String {
    env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("~/.local/share/{}", env!("CARGO_CRATE_NAME")))
}

pub fn save_stations(stations: Vec<crate::Station>) -> std::io::Result<()> {
    let _ = fs::create_dir(get_data_dir());
    let mut data_file = File::create(format!("{}/stations.json", get_data_dir()))?;
    let json = serde_json::to_string(&stations)?;
    data_file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn load_stations() -> Vec<crate::Station> {
    File::open(format!("{}/stations.json", get_data_dir())).map_or_else(
        |_| Vec::new(),
        |mut data_file| {
            let mut data = String::new();
            data_file.read_to_string(&mut data).unwrap();
            serde_json::from_str(&data).unwrap()
        },
    )
}
