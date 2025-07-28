use std::{
    sync::{Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use anyhow::Error;
use gstreamer::glib::{self, MainLoop};
use gstreamer_player::{Player, gst::prelude::*};
use tokio_util::sync::CancellationToken;

fn load(uri: &str, token: CancellationToken) -> Result<Player, Error> {
    gstreamer::init()?;

    let dispatcher = gstreamer_player::PlayerGMainContextSignalDispatcher::new(None);
    let player = gstreamer_player::Player::new(
        None::<gstreamer_player::PlayerVideoRenderer>,
        Some(dispatcher.upcast::<gstreamer_player::PlayerSignalDispatcher>()),
    );

    // Tell the player what uri to play.
    player.set_uri(Some(uri));

    let error = Arc::new(Mutex::new(Ok((player.clone()))));
    let token_clone = token.clone();
    // Connect to the player's "end-of-stream" signal, which will tell us when the
    // currently played media stream reached its end.
    player.connect_end_of_stream(move |player| {
        player.stop();
        token_clone.cancel();
    });

    let error_clone = Arc::clone(&error);
    let token_clone = token.clone();
    // Connect to the player's "error" signal, which will inform us about eventual
    // errors (such as failing to retrieve a http stream).
    player.connect_error(move |player, err| {
        let error = &error_clone;

        *error.lock().unwrap() = Err(err.clone());

        player.stop();
        token_clone.cancel();
    });

    let guard = error.as_ref().lock().unwrap();

    return guard.clone().map_err(|e| e.into());
}

pub fn play(uri: &str, volume: Arc<RwLock<f64>>, token: CancellationToken) {
    if let Ok(player) = load(&uri, token.clone()) {
        let player_clone = player.clone();
        thread::spawn(move || {
            loop {
                player_clone.set_volume(*volume.read().unwrap());
                if token.is_cancelled() {
                    player_clone.stop();
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        player.play();
    }
}
