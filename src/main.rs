// TODO
// Show dialog when a stream fails
//
// Integrate with system volume controls and mpris
//
// ensure only one radio-browser search is running at a time, maybe keep the api
// around or set up another task that is fired whenever the search changes to
// check if the last search has finished and if store the new search (and overwrite it) until it does finish

use gstreamer_player::Player;
use gtk::prelude::*;
use relm4::{
    RelmObjectExt,
    binding::StringBinding,
    gtk::{PolicyType, gdk::Rectangle, glib::Propagation, pango},
    prelude::*,
    typed_view::{
        TypedListItem,
        list::{RelmListItem, TypedListView},
    },
};
use serde::{Deserialize, Serialize};

mod icon_names {
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}

mod saver;
mod search;
mod streamer;

#[derive(Debug)]
struct SearchItem {
    station: Station,
    sender: AsyncComponentSender<Radio>,
}

impl SearchItem {
    fn new(station: Station, sender: AsyncComponentSender<Radio>) -> Self {
        Self { station, sender }
    }
}

struct SearchWidgets {
    label: gtk::Label,
    add_button: gtk::Button,
}

impl RelmListItem for SearchItem {
    type Root = gtk::Box;
    type Widgets = SearchWidgets;

    fn setup(_item: &gtk::ListItem) -> (gtk::Box, SearchWidgets) {
        relm4::view! {
            my_box = gtk::Box {
                set_spacing: 2,
                set_margin_all: 2,
                set_orientation: gtk::Orientation::Horizontal,
                set_hexpand: false,
                #[name = "label"]
                gtk::Label {
                    set_wrap: true,
                    set_ellipsize: pango::EllipsizeMode::End,
                },
                #[name = "add_button"]
                gtk::Button {
                    set_halign: gtk::Align::End,
                    set_hexpand: true,
                },
            },
        }

        let widgets = SearchWidgets { label, add_button };

        (my_box, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        let SearchWidgets { label, add_button } = widgets;

        let sender = self.sender.clone();
        let station = self.station.clone();
        label.set_text(&self.station.name);

        add_button.set_icon_name(icon_names::PLUS);
        add_button.connect_clicked(move |_| {
            sender.input(Msg::StationNameChanged(station.name.clone()));
            sender.input(Msg::StationUrlChanged(station.url.clone()));
            sender.input(Msg::AddStation);
        });
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    Error,
}

#[derive(Debug)]
struct StationListItem {
    station: Station,
    id: usize,
    sender: AsyncComponentSender<Radio>,
    active: bool,
    labelbinding: StringBinding,
}

impl StationListItem {
    fn new(station: Station, id: usize, sender: AsyncComponentSender<Radio>) -> Self {
        Self {
            station: station.clone(),
            id,
            sender,
            active: false,
            labelbinding: StringBinding::new(station.name),
        }
    }
    pub fn active(&mut self) {
        self.active = true;
        self.labelbinding
            .set_value(format!("<b>{}</b>", self.station.name));
    }
    pub fn inactive(&mut self) {
        self.active = false;
        self.labelbinding.set_value(self.station.name.clone());
    }
}

struct StationWidgets {
    label: gtk::Label,
}

impl RelmListItem for StationListItem {
    type Root = gtk::Box;
    type Widgets = StationWidgets;

    fn setup(_item: &gtk::ListItem) -> (gtk::Box, StationWidgets) {
        relm4::view! {
            my_box = gtk::Box {
                #[name = "label"]
                gtk::Label,
            },
        }

        let widgets = StationWidgets { label };

        (my_box, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        let StationWidgets { label } = widgets;

        let motion = gtk::EventControllerMotion::new();

        let sender = self.sender.clone();
        let id = self.id;
        // When pointer enters or moves inside
        motion.connect_enter(move |_, _, _| {
            sender.input(Msg::SetHoverId(Some(id)));
        });
        let sender = self.sender.clone();
        // When pointer leaves the row widget
        motion.connect_leave(move |_| {
            sender.input(Msg::SetHoverId(None));
        });

        let click = gtk::GestureClick::new();
        click.set_button(0);
        let sender = self.sender.clone();
        let station = self.station.clone();
        click.connect_pressed(move |controller, _, _, _| {
            if controller.current_button() == gtk::gdk::BUTTON_PRIMARY {
                sender.input(Msg::Play(station.clone(), id));
            }
        });

        root.add_controller(motion);
        root.add_controller(click);

        if self.active {
            self.active() // ensure we dont loose boldness when rebinding
        }

        label.set_use_markup(true);
        label.add_binding(&self.labelbinding, "label");
    }
}

#[derive(Debug)]
struct StationList {
    list_view_wrapper: TypedListView<StationListItem, gtk::NoSelection>,
    sender: AsyncComponentSender<Radio>,
}

impl StationList {
    fn new(sender: AsyncComponentSender<Radio>) -> Self {
        Self {
            list_view_wrapper: TypedListView::new(),
            sender,
        }
    }

    fn append(&mut self, station: Station) {
        let id = self.list_view_wrapper.len();
        self.list_view_wrapper.append(StationListItem::new(
            station,
            id as usize,
            self.sender.clone(),
        ));
        self.save();
    }

    fn get_by_id(&self, id: Option<usize>) -> Option<TypedListItem<StationListItem>> {
        if let Some(id_target) = id {
            for x in 0..self.list_view_wrapper.len() {
                if let Some(indexed_item) = self.list_view_wrapper.get(x)
                    && indexed_item.borrow().id == id_target
                {
                    return Some(indexed_item);
                }
            }
        }
        None
    }

    fn remove_by_id(&mut self, id: usize) {
        for x in 0..self.list_view_wrapper.len() {
            if let Some(item) = self.list_view_wrapper.get(x)
                && item.borrow().id == id
            {
                self.list_view_wrapper.remove(x);
            }
        }
        self.save();
    }

    fn save(&self) {
        let mut stations = vec![];
        for x in 0..self.list_view_wrapper.len() {
            if let Some(item) = self.list_view_wrapper.get(x) {
                stations.push(item.borrow().station.clone());
            }
        }
        saver::save_stations(stations).unwrap();
    }

    fn load(&mut self) {
        let stations = saver::load_stations();
        let _: Vec<_> = stations
            .iter()
            .map(|station| self.append(station.clone()))
            .collect();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    name: String,
    url: String, // https://www.radio-browser.info/
}

#[derive(Debug)]
struct Radio {
    station_list: StationList,
    ctx_menu_handle: gtk::Popover,
    search_results_handle: TypedListView<SearchItem, gtk::NoSelection>,
    title: String,
    new_station_name: String,
    new_station_url: String,
    hover_id: Option<usize>,
    menu_id: usize,
    playing_id: Option<usize>,
    player: Player,
}

#[derive(Debug)]
enum Msg {
    Play(Station, usize),
    Stop,
    VolumeChanged(f64),
    StationNameChanged(String),
    StationUrlChanged(String),
    AddStation,
    ShowMenu(f64, f64),
    DeleteStation,
    SetHoverId(Option<usize>),
    SearchQuery(String),
}

#[relm4::component(async)]
impl AsyncComponent for Radio {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::Window {
            #[watch]
            set_title: Some(&model.title),
            set_default_size: (200, 250),
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 5,

                // using a center box so I can maybe have something in the
                // middle in the future
                gtk::CenterBox {
                    #[wrap(Some)]
                    set_start_widget = &gtk::Box {
                        set_spacing: 5,
                        set_orientation: gtk::Orientation::Horizontal,
                        // Stop button
                        gtk::Button {
                            set_icon_name: icon_names::STOP_LARGE,
                            connect_clicked => Msg::Stop,
                        },
                        gtk::Image {
                            set_icon_name: Some(icon_names::SPEAKER_3),
                        },
                        gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.1){
                            set_width_request: 120,
                            connect_change_value[sender] => move |_, _, val| {
                                sender.input(Msg::VolumeChanged(val));
                                Propagation::Proceed
                            },
                            set_value: 1.0,
                        },
                    },

                    #[wrap(Some)]
                    set_end_widget = &gtk::Box{
                        set_halign: gtk::Align::End,
                        set_spacing: 5,
                        // Search button
                        gtk::MenuButton {
                            set_icon_name: icon_names::SEARCH_GLOBAL,
                            set_direction: gtk::ArrowType::Down,
                            #[wrap(Some)]
                            set_popover: search_popover = &gtk::Popover{
                                set_position: gtk::PositionType::Right,
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    gtk::SearchEntry {
                                        connect_search_changed[sender] => move |entry| {
                                            let query = entry.text();
                                            sender.input(Msg::SearchQuery(query.into()));
                                        }
                                    },

                                    gtk::ScrolledWindow {
                                        set_height_request: 200,
                                        set_width_request: 300,
                                        set_hscrollbar_policy: PolicyType::Never,
                                        #[local_ref]
                                        search_results -> gtk::ListView {
                                        }
                                    }
                                }
                            }
                        },
                        // Add button
                        gtk::MenuButton {
                            set_icon_name: icon_names::PLUS,
                            set_direction: gtk::ArrowType::Down,
                            #[wrap(Some)]
                            set_popover: popover = &gtk::Popover {
                                set_position: gtk::PositionType::Right,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 5,
                                    gtk::Label {
                                        set_halign: gtk::Align::Start,
                                        set_label: "Name:",
                                    },
                                    gtk::Entry {
                                        connect_changed[sender] => move |entry| {
                                            let buffer = entry.buffer();
                                            sender.input(Msg::StationNameChanged(buffer.text().into()));
                                        }
                                    },
                                    gtk::Label {
                                        set_halign: gtk::Align::Start,
                                        set_label: "URL:",
                                    },
                                    gtk::Entry {
                                        connect_changed[sender] => move |entry| {
                                            let buffer = entry.buffer();
                                            sender.input(Msg::StationUrlChanged(buffer.text().into()));
                                        }
                                    },
                                    gtk::Separator {},
                                    gtk::Button {
                                        set_label: "Add Station",
                                        connect_clicked => Msg::AddStation,
                                    },

                                },
                            },
                        },
                    },
                },

                #[local_ref]
                // right click menu
                ctx_menu -> gtk::Popover {
                    gtk::Button {
                        set_label: "Delete Station",
                        connect_clicked => Msg::DeleteStation,
                    },
                },

                // station list
                gtk::ScrolledWindow {
                    set_vexpand: true,

                    #[local_ref]
                    station_list_view -> gtk::ListView {
                        add_controller = gtk::GestureClick {
                            set_button: 0,
                            connect_pressed[sender] => move |controller, _, x, y| {
                                if controller.current_button() == gtk::gdk::BUTTON_SECONDARY {
                                    sender.input(Msg::ShowMenu(x, y));
                                }
                            }
                        },
                    }
                }
            }
        }
    }

    async fn init(
        _: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Initialize the StationList
        let mut station_list = StationList::new(sender.clone());
        station_list.load();

        let ctx_menu_handle = gtk::Popover::new();
        let search_results_handle = TypedListView::new();

        let model = Self {
            station_list,
            ctx_menu_handle,
            search_results_handle,
            title: "RelmyVibes".to_string(),
            new_station_name: String::new(),
            new_station_url: String::new(),
            hover_id: None,
            menu_id: 0,
            playing_id: None,
            player: streamer::load().unwrap(),
        };

        let station_list_view = &model.station_list.list_view_wrapper.view;
        let ctx_menu = &model.ctx_menu_handle;
        let search_results = &model.search_results_handle.view;

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            Msg::Play(station, id) => {
                if let Some(station_item) = self.station_list.get_by_id(self.playing_id) {
                    station_item.borrow_mut().inactive();
                }
                if let Some(station_item) = self.station_list.get_by_id(self.hover_id) {
                    station_item.borrow_mut().active();
                }
                self.playing_id = Some(id);
                self.title = station.name.clone();
                self.player.set_uri(Some(&station.url));
                self.player.play();
            }
            Msg::Stop => {
                if let Some(station_item) = self.station_list.get_by_id(self.playing_id) {
                    station_item.borrow_mut().inactive();
                }
                self.playing_id = None;
                self.title = "RelmyVibes".to_string();
                self.player.stop();
            }
            Msg::VolumeChanged(val) => self.player.set_volume(val),
            Msg::StationNameChanged(name) => self.new_station_name = name,
            Msg::StationUrlChanged(url) => self.new_station_url = url,
            Msg::AddStation => {
                if !self.new_station_name.is_empty() && !self.new_station_url.is_empty() {
                    let new_station = Station {
                        name: self.new_station_name.clone(),
                        url: self.new_station_url.clone(),
                    };
                    self.station_list.append(new_station);
                }
            }
            Msg::ShowMenu(x, y) => {
                if let Some(hover_id) = self.hover_id {
                    let rect = Rectangle::new(x as i32, (y as i32) + 45, 0, 0);
                    self.ctx_menu_handle.set_pointing_to(Some(&rect));
                    self.ctx_menu_handle.popup();
                    self.menu_id = hover_id;
                }
            }
            Msg::DeleteStation => {
                self.ctx_menu_handle.popdown();
                self.station_list.remove_by_id(self.menu_id);
                self.ctx_menu_handle.popdown();
            }
            Msg::SetHoverId(id) => {
                self.hover_id = id;
            }
            Msg::SearchQuery(query) => {
                self.search_results_handle.clear();
                let results = search::search(query).await.unwrap();
                for result in results {
                    self.search_results_handle
                        .append(SearchItem::new(result, sender.clone()));
                }
            }
        }
    }
}

fn main() {
    relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
    let app = RelmApp::new("relm4.example.typed-list-view");
    app.run_async::<Radio>(());
}
