use radiobrowser::RadioBrowserAPI;
use radiobrowser::StationOrder;
use std::error::Error;

use crate::Station;

pub async fn search(query: String) -> Result<Vec<Station>, Box<dyn Error>> {
    let api = RadioBrowserAPI::new().await?;
    let stations = api
        .get_stations()
        .name(query)
        .reverse(true)
        .hidebroken(true)
        .order(StationOrder::Clickcount)
        .send()
        .await?;
    return Ok(stations
        .iter()
        .take(10)
        .map(|station| Station {
            name: station.name.to_string(),
            url: station.url_resolved.to_string(),
        })
        .collect());
}
