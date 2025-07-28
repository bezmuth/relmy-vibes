use std::sync::{Arc, Mutex};

use anyhow::Error;
use gstreamer_player::{Player, gst::prelude::*};
use relm4::AsyncComponentSender;

pub fn load(sender: AsyncComponentSender<crate::Radio>) -> Result<Player, Error> {
    gstreamer::init()?;

    let dispatcher = gstreamer_player::PlayerGMainContextSignalDispatcher::new(None);
    let player = gstreamer_player::Player::new(
        None::<gstreamer_player::PlayerVideoRenderer>,
        Some(dispatcher.upcast::<gstreamer_player::PlayerSignalDispatcher>()),
    );

    player.set_volume(1.0);
    let error = Arc::new(Mutex::new(Ok(player.clone())));
    // Connect to the player's "end-of-stream" signal, which will tell us when the
    // currently played media stream reached its end.
    player.connect_end_of_stream(move |player| {
        player.stop();
    });

    let error_clone = Arc::clone(&error);
    // Connect to the player's "error" signal, which will inform us about eventual
    // errors (such as failing to retrieve a http stream).
    player.connect_error(move |player, err| {
        let error = &error_clone;

        *error.lock().unwrap() = Err(err.clone());

        player.stop();
    });

    player.connect_volume_changed(move |player| {
        sender.input(crate::Msg::VolumeChanged(player.volume()))
    });

    let guard = error.as_ref().lock().unwrap();

    guard.clone().map_err(|e| e.into())
}
