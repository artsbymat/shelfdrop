use crate::{
    storage::{Mode, Theme},
    ui::{Ui, header, set_label},
};
use gtk::{gdk, glib, prelude::*};
use std::rc::Rc;

pub fn show(ui: &Rc<Ui>, invoking: &gtk::Button) {
    let prefs = ui.prefs.get();
    let window = gtk::Window::builder()
        .title("Shelfdrop Settings")
        .transient_for(&ui.window)
        .modal(true)
        .destroy_with_parent(true)
        .default_width(622)
        .default_height(438)
        .build();
    window.add_css_class("shelf-window");
    window.set_titlebar(Some(&header("Shelfdrop Settings", None)));

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.add_css_class("settings-root");
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let sections = gtk::Box::new(gtk::Orientation::Horizontal, 22);
    sections.set_homogeneous(true);
    sections.add_css_class("preferences");
    scroller.set_child(Some(&sections));
    layout.append(&scroller);

    let general = section("General");
    let appearance = section("Shelf appearance");
    appearance.add_css_class("appearance-settings");
    sections.append(&general);
    sections.append(&appearance);
    let (themes, theme_buttons) = choices(
        &["System", "Dark", "Light"],
        match prefs.theme {
            Theme::System => 0,
            Theme::Dark => 1,
            Theme::Light => 2,
        },
    );
    general.append(&field("Theme", &themes));
    for (button, theme) in theme_buttons
        .into_iter()
        .zip([Theme::System, Theme::Dark, Theme::Light])
    {
        button.connect_toggled(glib::clone!(
            #[weak]
            ui,
            move |button| {
                if button.is_active() {
                    ui.change_preferences(false, |prefs| prefs.theme = theme);
                }
            }
        ));
    }
    for name in ["Shelf position", "Window behavior"] {
        let picker = gtk::DropDown::from_strings(&["System default"]);
        picker.set_sensitive(false);
        set_label(&picker, name);
        general.append(&inline_field(name, &picker));
    }
    let support = gtk::Label::new(Some(
        "Position and Keep above are managed by your compositor.",
    ));
    support.set_xalign(0.0);
    support.set_wrap(true);
    support.set_max_width_chars(32);
    support.add_css_class("muted");
    general.append(&support);

    let max = spin("Maximum items", 1, 999, prefs.max_items as i32);
    general.append(&inline_field("Maximum items", &max));
    max.connect_value_changed(glib::clone!(
        #[weak]
        ui,
        move |max| ui
            .change_preferences(false, |prefs| prefs.max_items = max.value_as_int() as usize)
    ));

    let clear = gtk::Switch::new();
    clear.set_active(prefs.clear_on_exit);
    clear.set_valign(gtk::Align::Center);
    set_label(&clear, "Clear shelf on exit");
    general.append(&inline_field("Clear shelf on exit", &clear));
    clear.connect_active_notify(glib::clone!(
        #[weak]
        ui,
        move |clear| ui.change_preferences(false, |prefs| prefs.clear_on_exit = clear.is_active())
    ));

    let (modes, mode_buttons) = choices(
        &["Compact", "Expanded"],
        usize::from(prefs.mode == Mode::Expanded),
    );
    let (views, view_buttons) = choices(&["List", "Grid"], usize::from(prefs.grid));
    appearance.append(&field("Window mode", &modes));
    appearance.append(&field("File layout", &views));

    let sizes = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    sizes.set_homogeneous(true);
    sizes.set_tooltip_text(Some("The compositor can constrain the window size."));
    let width = spin("Window width in pixels", 300, 1800, prefs.width);
    let height = spin("Window height in pixels", 220, 1200, prefs.height);
    sizes.append(&width);
    sizes.append(&height);
    appearance.append(&field("Window size · px", &sizes));
    width.connect_value_changed(glib::clone!(
        #[weak]
        ui,
        move |width| ui.change_preferences(true, |prefs| prefs.width = width.value_as_int())
    ));
    height.connect_value_changed(glib::clone!(
        #[weak]
        ui,
        move |height| ui.change_preferences(true, |prefs| prefs.height = height.value_as_int())
    ));
    for (button, mode) in mode_buttons
        .into_iter()
        .zip([Mode::Compact, Mode::Expanded])
    {
        button.connect_toggled(glib::clone!(
            #[weak]
            ui,
            #[weak]
            width,
            #[weak]
            height,
            #[weak(rename_to = list)]
            view_buttons[0],
            #[weak(rename_to = grid)]
            view_buttons[1],
            move |button| {
                if button.is_active() {
                    let expanded = mode == Mode::Expanded;
                    let (w, h) = if expanded { (656, 360) } else { (310, 238) };
                    ui.change_preferences(true, |prefs| {
                        prefs.mode = mode;
                        prefs.grid = expanded;
                        prefs.width = w;
                        prefs.height = h;
                    });
                    if expanded {
                        grid.set_active(true);
                    } else {
                        list.set_active(true);
                    }
                    width.set_value(f64::from(w));
                    height.set_value(f64::from(h));
                }
            }
        ));
    }
    for (button, grid) in view_buttons.into_iter().zip([false, true]) {
        button.connect_toggled(glib::clone!(
            #[weak]
            ui,
            move |button| {
                if button.is_active() {
                    ui.change_preferences(false, |prefs| prefs.grid = grid);
                }
            }
        ));
    }

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 85.0, 100.0, 1.0);
    opacity.set_draw_value(true);
    opacity.set_value_pos(gtk::PositionType::Right);
    opacity.set_format_value_func(|_, value| format!("{value:.0}%"));
    opacity.set_value(f64::from(prefs.opacity));
    set_label(&opacity, "Window chrome opacity");
    appearance.append(&field("Opacity", &opacity));
    opacity.connect_value_changed(glib::clone!(
        #[weak]
        ui,
        move |opacity| ui.change_preferences(false, |prefs| prefs.opacity =
            opacity.value().round() as i32)
    ));

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("footer");
    let note = gtk::Label::new(Some("Changes apply immediately"));
    note.set_hexpand(true);
    note.set_xalign(0.0);
    note.add_css_class("muted");
    footer.append(&note);
    let done = gtk::Button::with_label("Done");
    done.add_css_class("done");
    done.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    footer.append(&done);
    layout.append(&footer);

    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    key.connect_key_pressed(glib::clone!(
        #[weak]
        window,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key == gdk::Key::Escape {
                window.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    window.add_controller(key);
    window.connect_close_request(glib::clone!(
        #[weak]
        invoking,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_| {
            invoking.grab_focus();
            glib::Propagation::Proceed
        }
    ));
    window.set_child(Some(&layout));
    window.present();
}

fn section(title: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 13);
    section.set_size_request(248, -1);
    section.set_valign(gtk::Align::Start);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.add_css_class("section-title");
    section.append(&label);
    section
}

fn field(label: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let title = gtk::Label::new(Some(label));
    title.set_xalign(0.0);
    row.append(&title);
    row.append(control);
    row
}

fn inline_field(label: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some(label));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    row.append(&title);
    row.append(control);
    row
}

fn spin(label: &str, min: i32, max: i32, value: i32) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(f64::from(min), f64::from(max), 1.0);
    spin.set_numeric(true);
    spin.set_width_chars(4);
    spin.set_max_width_chars(4);
    spin.set_value(f64::from(value));
    set_label(&spin, label);
    spin
}

fn choices(labels: &[&str], active: usize) -> (gtk::Box, Vec<gtk::ToggleButton>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_homogeneous(true);
    row.add_css_class("linked");
    row.add_css_class("segments");
    let buttons: Vec<_> = labels
        .iter()
        .map(|label| gtk::ToggleButton::with_label(label))
        .collect();
    for (i, button) in buttons.iter().enumerate() {
        if i > 0 {
            button.set_group(Some(&buttons[0]));
        }
        row.append(button);
    }
    buttons[active].set_active(true);
    (row, buttons)
}
