use systemd::dbus::{self, UnitState};
use systemd::analyze::Analyze;
use gtk;
use gtk::prelude::*;

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use gdk::keys::constants as key;
use gtk::{IconSize, Image};

/// Updates the status icon for the selected unit
fn get_updated_icon(state: bool) -> Image {
    if state {
        return Image::from_icon_name(Some("gtk-yes"), IconSize::Button);
    }

    Image::from_icon_name(Some("gtk-no"), IconSize::Button)
}

/// Create a `gtk::ListboxRow` and add it to the `gtk::ListBox`, and then add the `gtk::Image` to a vector so that we can later modify
/// it when the state changes.
fn create_row(row: &mut gtk::ListBoxRow, path: &Path, state: UnitState, state_icons: &mut Vec<gtk::Image>) {
    let filename = path.file_stem().unwrap().to_str().unwrap();
    let unit_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let unit_label = gtk::Label::new(Some(filename));
    let image = if state == UnitState::Enabled {
        Image::from_icon_name(Some("gtk-yes"), IconSize::Button)
    } else {
        Image::from_icon_name(Some("gtk-no"), IconSize::Button)
    };
    unit_box.add(&unit_label);
    unit_box.pack_end(&image, false, false, 15);
    row.add(&unit_box);
    state_icons.push(image);
}

/// Read the unit file and return it's contents so that we can display it in the `gtk::TextView`.
fn get_unit_info<P: AsRef<Path>>(path: P) -> String {
    let mut file = fs::File::open(path).unwrap();
    let mut output = String::new();
    let _ = file.read_to_string(&mut output);
    output
}

/// Use `systemd-analyze blame` to fill out the information for the Analyze `gtk::Stack`.
fn setup_systemd_analyze(builder: &gtk::Builder) {
    let analyze_tree: gtk::TreeView = builder.get_object("analyze_tree").unwrap();
    let analyze_store = gtk::ListStore::new(&[glib::Type::U32, glib::Type::String]);

    // A simple macro for adding a column to the preview tree.
    macro_rules! add_column {
        ($preview_tree:ident, $title:expr, $id:expr) => {{
            let column   = gtk::TreeViewColumn::new();
            let renderer = gtk::CellRendererText::new();
            column.set_title($title);
            column.set_resizable(true);
            column.pack_start(&renderer, true);
            column.add_attribute(&renderer, "text", $id);
            analyze_tree.append_column(&column);
        }}
    }

    add_column!(analyze_store, "Time (ms)", 0);
    add_column!(analyze_store, "Unit", 1);

    let units = Analyze::blame();
    for value in units.clone() {
        analyze_store.insert_with_values(None, &[0, 1], &[&value.time, &value.service]);
    }

    analyze_tree.set_model(Some(&analyze_store));

    let total_time_label: gtk::Label = builder.get_object("time_to_boot").unwrap();
    let time = (units.iter().last().unwrap().time as f32) / 1000f32;
    total_time_label.set_label(format!("{} seconds", time).as_str());
}

/// Updates the associated journal `TextView` with the contents of the unit's journal log.
fn update_journal(journal: &gtk::TextView, unit_path: &str) {
    journal.get_buffer().unwrap().set_text(get_unit_journal(unit_path).as_str());
}

/// Obtains the journal log for the given unit.
fn get_unit_journal(unit_path: &str) -> String {
    let log = String::from_utf8(Command::new("journalctl").arg("-b").arg("-u")
        .arg(Path::new(unit_path).file_stem().unwrap().to_str().unwrap())
        .output().unwrap().stdout).unwrap();
    log.lines().rev().map(|x| x.trim()).fold(String::with_capacity(log.len()), |acc, x| acc + "\n" + x)
}

// TODO: Fix clippy error and start using this everywhere
fn get_filename<'a>(path: &'a str) -> &str {
    Path::new(path).file_name().unwrap().to_str().unwrap()
}

pub fn launch() {
    gtk::init().unwrap_or_else(|_| panic!("tv-renamer: failed to initialize GTK."));

    let builder = gtk::Builder::from_string(include_str!("interface.glade"));
    let window: gtk::Window = builder.get_object("main_window").unwrap();
    let unit_stack: gtk::Stack = builder.get_object("unit_stack").unwrap();
    let services_list: gtk::ListBox = builder.get_object("services_list").unwrap();
    let sockets_list: gtk::ListBox = builder.get_object("sockets_list").unwrap();
    let timers_list: gtk::ListBox = builder.get_object("timers_list").unwrap();
    let unit_info: gtk::TextView = builder.get_object("unit_info").unwrap();
    let ablement_switch: gtk::Switch = builder.get_object("ablement_switch").unwrap();
    let start_button: gtk::Button = builder.get_object("start_button").unwrap();
    let stop_button: gtk::Button = builder.get_object("stop_button").unwrap();
    let save_unit_file: gtk::Button = builder.get_object("save_button").unwrap();
    let unit_menu_label: gtk::Label = builder.get_object("unit_menu_label").unwrap();
    let unit_popover: gtk::PopoverMenu = builder.get_object("unit_menu_popover").unwrap();
    let services_button: gtk::Button = builder.get_object("services_button").unwrap();
    let sockets_button: gtk::Button = builder.get_object("sockets_button").unwrap();
    let timers_button: gtk::Button = builder.get_object("timers_button").unwrap();
    let unit_journal: gtk::TextView = builder.get_object("unit_journal_view").unwrap();
    let refresh_log_button: gtk::Button = builder.get_object("refresh_log_button").unwrap();
    let right_header: gtk::Label = builder.get_object("header_service_label").unwrap();

    { // NOTE: Services Menu Button
        let label = unit_menu_label.clone();
        let stack = unit_stack.clone();
        let popover = unit_popover.clone();
        services_button.connect_clicked(move |_| {
            stack.set_visible_child_name("Services");
            label.set_text("Services");
            popover.set_visible(false);
        });
    }

    { // NOTE: Sockets Menu Button
        let label = unit_menu_label.clone();
        let stack = unit_stack.clone();
        let popover = unit_popover.clone();
        sockets_button.connect_clicked(move |_| {
            stack.set_visible_child_name("Sockets");
            label.set_text("Sockets");
            popover.set_visible(false);
        });
    }

    { // NOTE: Timers Menu Button
        let label = unit_menu_label.clone();
        let stack = unit_stack.clone();
        let popover = unit_popover.clone();
        timers_button.connect_clicked(move |_| {
            stack.set_visible_child_name("Timers");
            label.set_text("Timers");
            popover.set_visible(false);
        });
    }

    // Setup the Analyze stack
    setup_systemd_analyze(&builder);

    // List of all unit files on the system
    let unit_files = dbus::list_unit_files();

    // NOTE: Services
    let services = dbus::collect_togglable_services(&unit_files);
    let mut services_icons = Vec::new();
    for service in services.clone() {
        let mut unit_row = gtk::ListBoxRow::new();
        create_row(&mut unit_row, Path::new(service.name.as_str()), service.state, &mut services_icons);
        services_list.insert(&unit_row, -1);
    }

    {
        let services = services.clone();
        let services_list = services_list.clone();
        let unit_info = unit_info.clone();
        let ablement_switch = ablement_switch.clone();
        let unit_journal = unit_journal.clone();
        let header = right_header.clone();
        services_list.connect_row_selected(move |_, row| {
            let index = row.clone().unwrap().get_index();
            let service = &services[index as usize];
            let description = get_unit_info(service.name.as_str());
            unit_info.get_buffer().unwrap().set_text(description.as_str());
            ablement_switch.set_active(dbus::get_unit_file_state(service.name.as_str()));
            ablement_switch.set_state(ablement_switch.get_active());
            update_journal(&unit_journal, service.name.as_str());
            header.set_label(get_filename(service.name.as_str()));
        });
    }

    // NOTE: Sockets
    let sockets = dbus::collect_togglable_sockets(&unit_files);
    let mut sockets_icons = Vec::new();
    for socket in sockets.clone() {
        let mut unit_row = gtk::ListBoxRow::new();
        create_row(&mut unit_row, Path::new(socket.name.as_str()), socket.state, &mut sockets_icons);
        sockets_list.insert(&unit_row, -1);
    }

    {
        let sockets = sockets.clone();
        let sockets_list = sockets_list.clone();
        let unit_info = unit_info.clone();
        let ablement_switch = ablement_switch.clone();
        let unit_journal = unit_journal.clone();
        let header = right_header.clone();
        sockets_list.connect_row_selected(move |_, row| {
            let index = row.clone().unwrap().get_index();
            let socket = &sockets[index as usize];
            let description = get_unit_info(socket.name.as_str());
            unit_info.get_buffer().unwrap().set_text(description.as_str());
            ablement_switch.set_active(dbus::get_unit_file_state(socket.name.as_str()));
            ablement_switch.set_state(true);
            update_journal(&unit_journal, socket.name.as_str());
            header.set_label(get_filename(socket.name.as_str()));
        });
    }

    // NOTE: Timers
    let timers = dbus::collect_togglable_timers(&unit_files);
    let mut timers_icons = Vec::new();
    for timer in timers.clone() {
        let mut unit_row = gtk::ListBoxRow::new();
        create_row(&mut unit_row, Path::new(timer.name.as_str()), timer.state, &mut timers_icons);
        timers_list.insert(&unit_row, -1);
    }

    {
        let timers = timers.clone();
        let timers_list = timers_list.clone();
        let unit_info = unit_info.clone();
        let ablement_switch = ablement_switch.clone();
        let unit_journal = unit_journal.clone();
        let header = right_header.clone();
        timers_list.connect_row_selected(move |_, row| {
            let index = row.clone().unwrap().get_index();
            let timer = &timers[index as usize];
            let description = get_unit_info(timer.name.as_str());
            unit_info.get_buffer().unwrap().set_text(description.as_str());
            ablement_switch.set_active(dbus::get_unit_file_state(timer.name.as_str()));
            ablement_switch.set_state(true);
            update_journal(&unit_journal, timer.name.as_str());
            header.set_label(get_filename(timer.name.as_str()));
        });
    }

    { // NOTE: Implement the {dis, en}able button
        let services = services.clone();
        let services_list = services_list.clone();
        let sockets = sockets.clone();
        let sockets_list = sockets_list.clone();
        let timers = timers.clone();
        let timers_list = timers_list.clone();
        let unit_stack = unit_stack.clone();
        ablement_switch.connect_state_set(move |switch, enabled| {
            match unit_stack.get_visible_child_name().unwrap().as_str() {
                "Services" => {
                    let index = services_list.get_selected_row().unwrap().get_index();
                    let service = &services[index as usize];
                    let service_path = Path::new(service.name.as_str()).file_name().unwrap().to_str().unwrap();
                    if enabled && !dbus::get_unit_file_state(service.name.as_str()) {
                        dbus::enable_unit_files(service_path);
                        switch.set_state(true);
                    } else if !enabled && dbus::get_unit_file_state(service.name.as_str()) {
                        dbus::disable_unit_files(service_path);
                        switch.set_state(false);
                    }
                }
                "Sockets" => {
                    let index = sockets_list.get_selected_row().unwrap().get_index();
                    let socket = &sockets[index as usize];
                    let socket_path = get_filename(socket.name.as_str());
                    if enabled && !dbus::get_unit_file_state(socket.name.as_str()) {
                        dbus::enable_unit_files(socket_path);
                        switch.set_state(true);
                    } else if !enabled && dbus::get_unit_file_state(socket.name.as_str()) {
                        dbus::disable_unit_files(socket_path);
                        switch.set_state(false);
                    }
                }
                "Timers" => {
                    let index = timers_list.get_selected_row().unwrap().get_index();
                    let timer = &timers[index as usize];
                    let timer_path = Path::new(timer.name.as_str()).file_name().unwrap().to_str().unwrap();
                    if enabled && !dbus::get_unit_file_state(timer.name.as_str()) {
                        dbus::enable_unit_files(timer_path);
                        switch.set_state(true);
                    } else if !enabled && dbus::get_unit_file_state(timer.name.as_str()) {
                        dbus::disable_unit_files(timer_path);
                        switch.set_state(false);
                    }
                }
                _ => unreachable!()
            }
            gtk::Inhibit(true)
        });
    }

    { // NOTE: Implement the start button
        let services = services.clone();
        let services_list = services_list.clone();
        let sockets = sockets.clone();
        let sockets_list = sockets_list.clone();
        let timers = timers.clone();
        let timers_list = timers_list.clone();
        let unit_stack = unit_stack.clone();
        let services_icons = services_icons.clone();
        let sockets_icons = sockets_icons.clone();
        let timers_icons = timers_icons.clone();

        start_button.connect_clicked(move |_| {
            let mut services_icons = Clone::clone(&services_icons);
            let mut sockets_icons = Clone::clone(&sockets_icons);
            let mut timers_icons = Clone::clone(&timers_icons);

            match unit_stack.get_visible_child_name().unwrap().as_str() {
                "Services" => {
                    let index = services_list.get_selected_row().unwrap().get_index();
                    let service = &services[index as usize];
                    if let None = dbus::start_unit(Path::new(service.name.as_str()).file_name().unwrap().to_str().unwrap()) {
                        services_icons.insert(index as usize, get_updated_icon(true));
                    }
                }
                "Sockets" => {
                    let index = sockets_list.get_selected_row().unwrap().get_index();
                    let socket = &sockets[index as usize];
                    if let None = dbus::start_unit(Path::new(socket.name.as_str()).file_name().unwrap().to_str().unwrap()) {
                        sockets_icons[index as usize] = get_updated_icon(true);
                    }
                }
                "Timers" => {
                    let index = timers_list.get_selected_row().unwrap().get_index();
                    let timer = &timers[index as usize];
                    if let None = dbus::start_unit(Path::new(timer.name.as_str()).file_name().unwrap().to_str().unwrap()) {
                        timers_icons[index as usize] = get_updated_icon(true);
                    }
                }
                _ => ()
            }
        });
    }

    { // NOTE: Implement the stop button
        let services = services.clone();
        let services_list = services_list.clone();
        let sockets = sockets.clone();
        let sockets_list = sockets_list.clone();
        let timers = timers.clone();
        let timers_list = timers_list.clone();
        let unit_stack = unit_stack.clone();
        let services_icons = services_icons.clone();
        let sockets_icons = sockets_icons.clone();
        let timers_icons = timers_icons.clone();

        stop_button.connect_clicked(move |_| {
            let mut services_icons = Clone::clone(&services_icons);
            let mut sockets_icons = Clone::clone(&sockets_icons);
            let mut timers_icons = Clone::clone(&timers_icons);

            match unit_stack.get_visible_child_name().unwrap().as_str() {
                "Services" => {
                    let index = services_list.get_selected_row().unwrap().get_index();
                    let service = &services[index as usize];
                    if let None = dbus::stop_unit(Path::new(service.name.as_str()).file_name().unwrap().to_str().unwrap()) {
                        services_icons.insert(index as usize, get_updated_icon(false));
                    }
                }
                "Sockets" => {
                    let index = sockets_list.get_selected_row().unwrap().get_index();
                    let socket = &sockets[index as usize];
                    if let None = dbus::stop_unit(Path::new(socket.name.as_str()).file_name().unwrap().to_str().unwrap()) {
                        sockets_icons.insert(index as usize, get_updated_icon(false));
                    }
                }
                "Timers" => {
                    let index = timers_list.get_selected_row().unwrap().get_index();
                    let timer = &timers[index as usize];
                    if let None = dbus::stop_unit(Path::new(timer.name.as_str()).file_name().unwrap().to_str().unwrap()) {
                        timers_icons.insert(index as usize, get_updated_icon(false));
                    }
                }
                _ => ()
            }
        });
    }

    { // NOTE: Save Button
        let unit_info = unit_info.clone();
        let services = services.clone();
        let services_list = services_list.clone();
        let sockets = sockets.clone();
        let sockets_list = sockets_list.clone();
        let timers = timers.clone();
        let timers_list = timers_list.clone();
        let unit_stack = unit_stack.clone();
        save_unit_file.connect_clicked(move |_| {
            let buffer = unit_info.get_buffer().unwrap();
            let start = buffer.get_start_iter();
            let end = buffer.get_end_iter();
            let text = buffer.get_text(&start, &end, true).unwrap();
            let path = match unit_stack.get_visible_child_name().unwrap().as_str() {
                "Services" => &services[services_list.get_selected_row().unwrap().get_index() as usize].name,
                "Sockets" => &sockets[sockets_list.get_selected_row().unwrap().get_index() as usize].name,
                "Timers" => &timers[timers_list.get_selected_row().unwrap().get_index() as usize].name,
                _ => unreachable!()
            };
            match fs::OpenOptions::new().write(true).open(&path) {
                Ok(mut file) => {
                    if let Err(message) = file.write(text.as_bytes()) {
                        println!("Unable to write to file: {:?}", message);
                    }
                }
                Err(message) => println!("Unable to open file: {:?}", message)
            }
        });
    }

    { // NOTE: Journal Refresh Button
        let services = services.clone();
        let services_list = services_list.clone();
        let sockets = sockets.clone();
        let sockets_list = sockets_list.clone();
        let timers = timers.clone();
        let timers_list = timers_list.clone();
        let unit_stack = unit_stack.clone();
        let refresh_button = refresh_log_button.clone();
        let unit_journal = unit_journal.clone();
        refresh_button.connect_clicked(move |_| {
            match unit_stack.get_visible_child_name().unwrap().as_str() {
                "Services" => {
                    let index = services_list.get_selected_row().unwrap().get_index();
                    let service = &services[index as usize];
                    update_journal(&unit_journal, service.name.as_str());
                }
                "Sockets" => {
                    let index = sockets_list.get_selected_row().unwrap().get_index();
                    let socket = &sockets[index as usize];
                    update_journal(&unit_journal, socket.name.as_str());
                }
                "Timers" => {
                    let index = timers_list.get_selected_row().unwrap().get_index();
                    let timer = &timers[index as usize];
                    update_journal(&unit_journal, timer.name.as_str());
                }
                _ => unreachable!()
            }
        });
    }

    window.show_all();

    // Quit the program when the program has been exited
    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    // Define custom actions on keypress
    window.connect_key_press_event(move |_, key| {
        if let key::Escape = key.get_keyval() { gtk::main_quit() }
        gtk::Inhibit(false)
    });

    gtk::main();
}
