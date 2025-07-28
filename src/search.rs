use radiobrowser::RadioBrowserAPI;
use radiobrowser::StationOrder;
use std::error::Error;

use crate::Station;
pub async fn search(api: RadioBrowserAPI, query: String) -> Result<Vec<Station>, Box<dyn Error>> {
    let stations = api
        .get_stations()
        .name(query)
        .reverse(true)
        .hidebroken(true)
        .order(StationOrder::Clickcount)
        .send()
        .await?;
    Ok(stations
        .iter()
        .map(|station| Station {
            name: station.name.to_string(),
            url: station.url_resolved.to_string(),
        })
        .collect())
}
