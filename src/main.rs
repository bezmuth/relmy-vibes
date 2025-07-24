// TODO:
// Favicon rendering!
// Error dialog when a stream fails (lets actually use that result)
// Station browser from radio-browser?
use iced::{
    Alignment::Center,
    Element, Font, Length, Size, Subscription, Task, Theme,
    widget::{
        button, column, container, horizontal_space, mouse_area, row, scrollable, slider, text,
        text_input, vertical_space,
    },
    window,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio_util::sync::CancellationToken;

mod saver;
mod streamer;

#[derive(Debug, Clone)]
pub enum Error {
    Error,
}

fn main() -> iced::Result {
    iced::daemon("IcyVibes", Radio::update, Radio::view)
        .subscription(Radio::subscription)
        .theme(|_, _| Theme::Dark)
        .font(include_bytes!("../fonts/icons.ttf").as_slice())
        .run_with(Radio::new)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Station {
    name: String,
    url: String, // https://www.radio-browser.info/
}

#[derive(Debug, Clone)]
struct Radio {
    stations: Vec<Station>,
    volume: Arc<RwLock<f32>>,
    token: CancellationToken,
    main_window: window::Id,
    dialog_window: Option<window::Id>,
    new_station_name: String,
    new_station_url: String,
    editing: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Play(String),
    Stop,
    Stopped(Result<(), Error>),
    VolumeChanged(f32),
    AddStationDialog,
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    StationNameChanged(String),
    StationUrlChanged(String),
    AddNewStation,
    DeleteStation(usize),
    ToggleEdit,
}

impl Radio {
    fn new() -> (Self, Task<Message>) {
        let (first_id, open) = window::open(window::Settings {
            size: Size::new(430.0, 400.0),
            position: window::Position::Centered,
            ..window::Settings::default()
        });
        (
            Self {
                stations: saver::load_stations(),
                volume: Arc::new(RwLock::new(1.0)),
                token: CancellationToken::new(),
                main_window: first_id,
                new_station_name: String::new(),
                new_station_url: String::new(),
                dialog_window: None,
                editing: false,
            },
            open.map(Message::WindowOpened),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Play(url) => {
                self.token.cancel();
                self.token = CancellationToken::new();
                Task::perform(
                    streamer::play(url, self.volume.clone(), self.token.clone()),
                    Message::Stopped,
                )
            }
            Message::Stopped(_result) => Task::none(),
            Message::Stop => {
                self.token.cancel();
                Task::none()
            }
            Message::VolumeChanged(new_vol) => {
                *self.volume.write().unwrap() = new_vol / 100.0;
                Task::none()
            }
            Message::AddStationDialog => {
                if self.dialog_window.is_none() {
                    let (id, open) = window::open(window::Settings {
                        size: Size::new(400.0, 170.0),
                        resizable: false,
                        position: window::Position::Centered,
                        ..window::Settings::default()
                    });
                    self.dialog_window = Some(id);
                    open.map(Message::WindowOpened)
                } else {
                    Task::none()
                }
            }
            Message::WindowClosed(id) => {
                if id == self.main_window {
                    self.token.cancel();
                    iced::exit()
                } else {
                    self.new_station_name = String::new();
                    self.new_station_url = String::new();
                    self.dialog_window = None;
                    Task::none()
                }
            }
            Message::WindowOpened(_id) => Task::none(),
            Message::StationNameChanged(name) => {
                self.new_station_name = name;
                Task::none()
            }
            Message::StationUrlChanged(url) => {
                self.new_station_url = url;
                Task::none()
            }
            Message::AddNewStation => {
                if self.new_station_name.is_empty() | self.new_station_url.is_empty() {
                    Task::none()
                } else {
                    self.stations.push(Station {
                        name: self.new_station_name.clone(),
                        url: self.new_station_url.clone(),
                    });
                    let window_close = window::close(self.dialog_window.unwrap()); // we know that if this is fired the window exists so an unwrap is fine
                    self.dialog_window = None;
                    saver::save_stations(self.stations.clone()).unwrap();
                    window_close
                }
            }
            Message::DeleteStation(index) => {
                self.stations.remove(index);
                saver::save_stations(self.stations.clone()).unwrap();
                Task::none()
            }
            Message::ToggleEdit => {
                self.editing = !self.editing;
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        window::close_events().map(Message::WindowClosed)
    }

    fn view(&self, window_id: window::Id) -> Element<Message> {
        if window_id == self.main_window {
            // the radio playing interface
            let radio_interface = column![
                station_list_element(self.stations.clone(), self.editing),
                global_controls(self.volume.clone(), self.editing)
            ];

            radio_interface.into()
        } else if let Some(dialog_id) = self.dialog_window
            && dialog_id == window_id
        {
            // Station adding interface
            column![
                text("Station Name:"),
                text_input("", &self.new_station_name).on_input(Message::StationNameChanged),
                text("Station Url:"),
                text_input("", &self.new_station_url).on_input(Message::StationUrlChanged),
                vertical_space(),
                button("Add").on_press(Message::AddNewStation),
            ]
            .spacing(5)
            .padding(5)
            .into()
        } else {
            horizontal_space().into()
        }
    }
}

fn global_controls<'a>(volume: Arc<RwLock<f32>>, editing: bool) -> Element<'a, Message> {
    let global_controls = container(
        row![
            button(stop_icon()).on_press(Message::Stop),
            row![
                volume_icon(),
                slider(
                    0.0..=100.0,
                    *volume.read().unwrap() * 100.0,
                    Message::VolumeChanged
                )
            ]
            .spacing(5)
            .align_y(Center)
            .width(150.0),
            horizontal_space(),
            if editing {
                row![
                    button(add_icon()).on_press(Message::AddStationDialog),
                    button(done_icon()).on_press(Message::ToggleEdit),
                ]
                .spacing(5)
            } else {
                row![button(edit_icon()).on_press(Message::ToggleEdit)]
            }
        ]
        .align_y(Center)
        .spacing(5),
    )
    .style(container::rounded_box)
    .width(iced::Length::Fill)
    .padding(10);

    global_controls.into()
}

fn station_list_element<'a>(stations: Vec<Station>, editing: bool) -> Element<'a, Message> {
    if stations.is_empty() {
        container(
            row![
                text("Please add a station: "),
                button(add_icon()).on_press(Message::AddStationDialog)
            ]
            .spacing(5)
            .align_y(Center),
        )
        .center(Length::Fill)
        .into()
    } else {
        let station_list = column(
            stations
                .into_iter()
                .enumerate()
                .map(|(index, station)| station_element(index, station, editing)),
        )
        .padding(5)
        .spacing(5);

        let station_scrollable = scrollable(station_list);
        column![station_scrollable, vertical_space()].into()
    }
}

fn station_element<'a>(index: usize, station: Station, editing: bool) -> Element<'a, Message> {
    let mut row_elements = row![text(station.name), horizontal_space()].align_y(Center);
    if editing {
        row_elements = row_elements.push(
            button(delete_icon())
                .style(button::danger)
                .on_press(Message::DeleteStation(index)),
        );
    }
    mouse_area(
        container(row_elements)
            .padding(10)
            .style(container::rounded_box),
    )
    .interaction(iced::mouse::Interaction::Pointer)
    .on_press(Message::Play(station.url))
    .into()
}

fn stop_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{E800}')
}
fn edit_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{E802}')
}
fn done_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{E804}')
}
fn delete_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{E801}')
}
fn add_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{E803}')
}
fn volume_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{E805}')
}
fn icon<'a, Message>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("icons");
    text(codepoint).font(ICON_FONT).into()
}
