use crate::{
    APP_ID, settings,
    shelf::{self, Entry, Shelf},
    storage::{Mode, Preferences, Storage, Theme},
    transfer,
};
use gtk::{gdk, gdk_pixbuf::Pixbuf, gio, glib, pango, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    path::Path,
    rc::Rc,
    time::Duration,
};

const LOGO: &[u8] = include_bytes!("../data/io.github.shelfdrop.Shelfdrop.svg");
const EMPTY: &[u8] = include_bytes!("../data/empty.svg");
const DROP: &[u8] = include_bytes!("../data/drop.svg");

#[derive(Clone, Copy)]
enum Action {
    Open,
    Reveal,
    Copy,
    Remove,
}

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    pub prefs: Cell<Preferences>,
    flow: gtk::FlowBox,
    stack: gtk::Stack,
    drag: gtk::Box,
    counter: gtk::Label,
    status: gtk::Label,
    clear: gtk::Button,
    layout: gtk::Button,
    appearance: gtk::CssProvider,
    shelf: RefCell<Shelf>,
    order: RefCell<Vec<String>>,
    recent: RefCell<HashSet<String>>,
    flash: Cell<u64>,
    storage: Storage,
    drops: RefCell<VecDeque<transfer::DropData>>,
    draining: Cell<bool>,
    generation: Cell<u64>,
    refreshing: Cell<bool>,
    menu: RefCell<Option<gtk::PopoverMenu>>,
}

impl Ui {
    pub fn new(app: &gtk::Application) -> Rc<Self> {
        let storage = Storage::new();
        let (prefs, entries, startup_status) = storage.load();
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Shelfdrop")
            .icon_name(APP_ID)
            .default_width(prefs.width)
            .default_height(prefs.height)
            .build();
        window.add_css_class("shelf-window");
        let appearance = gtk::CssProvider::new();
        let style = gtk::CssProvider::new();
        style.load_from_data(include_str!("style.css"));
        let display = gtk::prelude::WidgetExt::display(&window);
        gtk::style_context_add_provider_for_display(
            &display,
            &style,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        gtk::style_context_add_provider_for_display(
            &display,
            &appearance,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );

        let counter = gtk::Label::new(None);
        counter.add_css_class("counter");
        counter.set_valign(gtk::Align::Center);
        let header = header("Shelfdrop", Some(&counter));
        let preferences = icon_button("emblem-system-symbolic", "Settings");
        let layout = icon_button("view-grid-symbolic", "Grid view");
        header.pack_end(&preferences);
        header.pack_end(&layout);
        window.set_titlebar(Some(&header));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("shelf-root");
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_css_class("drop-stack");
        stack.add_named(
            &placeholder(
                EMPTY,
                "Your shelf is empty.",
                "Drag files here to keep them within reach.",
            )
            .0,
            Some("empty"),
        );
        let (drag, _) = placeholder(DROP, "Drop files or text to add", "");
        drag.add_css_class("drop-active");
        drag.set_can_target(false);
        drag.set_visible(false);
        let drop_zone = gtk::Overlay::new();
        drop_zone.set_child(Some(&stack));
        drop_zone.add_overlay(&drag);
        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::Multiple)
            .activate_on_single_click(false)
            .column_spacing(8)
            .row_spacing(3)
            .valign(gtk::Align::Start)
            .build();
        flow.add_css_class("files");
        let viewport = gtk::Viewport::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        viewport.set_scroll_to_focus(true);
        viewport.set_child(Some(&flow));
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&viewport)
            .build();
        flow.set_hadjustment(&scroller.hadjustment());
        flow.set_vadjustment(&scroller.vadjustment());
        stack.add_named(&scroller, Some("items"));
        root.append(&drop_zone);

        let status = gtk::Label::builder()
            .accessible_role(gtk::AccessibleRole::Status)
            .ellipsize(pango::EllipsizeMode::End)
            .xalign(0.0)
            .visible(false)
            .build();
        status.add_css_class("status");
        root.append(&status);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("footer");
        let clear = gtk::Button::new();
        clear.add_css_class("flat");
        let clear_title = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        clear_title.append(&gtk::Image::from_icon_name("user-trash-symbolic"));
        clear_title.append(&gtk::Label::new(Some("Clear All")));
        clear.set_child(Some(&clear_title));
        footer.append(&clear);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        let sort = gtk::DropDown::from_strings(&["Recently added", "Name"]);
        sort.set_selected(u32::from(prefs.sort_by_name));
        set_label(&sort, "Sort files");
        footer.append(&sort);
        root.append(&footer);
        window.set_child(Some(&root));

        let ui = Rc::new(Self {
            window,
            prefs: Cell::new(prefs),
            flow,
            stack,
            drag,
            counter,
            status,
            clear,
            layout,
            appearance,
            shelf: RefCell::new(Shelf { entries }),
            order: RefCell::new(Vec::new()),
            recent: RefCell::new(HashSet::new()),
            flash: Cell::new(0),
            storage,
            drops: RefCell::new(VecDeque::new()),
            draining: Cell::new(false),
            generation: Cell::new(0),
            refreshing: Cell::new(false),
            menu: RefCell::new(None),
        });
        ui.bind(preferences, sort);
        let settings = gtk::Settings::for_display(&display);
        for property in [
            "gtk-interface-color-scheme",
            "gtk-application-prefer-dark-theme",
            "gtk-theme-name",
        ] {
            if settings.find_property(property).is_some() {
                settings.connect_notify_local(
                    Some(property),
                    glib::clone!(
                        #[weak]
                        ui,
                        move |settings, _| {
                            if ui.prefs.get().theme == Theme::System {
                                ui.update_palette(system_dark(settings));
                            }
                        }
                    ),
                );
            }
        }
        ui.apply_appearance();
        ui.render();
        ui.show_status(&startup_status);
        ui
    }

    fn bind(self: &Rc<Self>, preferences: gtk::Button, sort: gtk::DropDown) {
        preferences.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| settings::show(&ui, button)
        ));
        self.layout.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| ui.change_preferences(false, |prefs| prefs.grid = !prefs.grid)
        ));
        sort.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |sort| ui
                .change_preferences(false, |prefs| prefs.sort_by_name = sort.selected() == 1)
        ));
        self.clear.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| {
                ui.generation.set(ui.generation.get().wrapping_add(1));
                ui.drops.borrow_mut().clear();
                ui.shelf.borrow_mut().entries.clear();
                ui.recent.borrow_mut().clear();
                ui.render();
                ui.show_status("Shelf cleared");
                ui.persist_entries();
            }
        ));
        self.window.connect_is_active_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |window| {
                if window.is_active() {
                    ui.refresh_missing();
                }
            }
        ));

        let drop = gtk::DropTargetAsync::new(None, gdk::DragAction::COPY);
        // Wayland may report the old MOVE selection before this target negotiates COPY.
        drop.connect_accept(|_, incoming| {
            if incoming.drag().is_some() {
                return false;
            }
            transfer::can_receive(&incoming.formats())
        });
        drop.connect_drag_motion(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, x, y| {
                let inside = ui.in_drop_area(x, y);
                ui.drag.set_visible(inside);
                if inside {
                    gdk::DragAction::COPY
                } else {
                    gdk::DragAction::empty()
                }
            }
        ));
        drop.connect_drag_enter(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, _, _| {
                ui.drag.set_visible(true);
                gdk::DragAction::COPY
            }
        ));
        drop.connect_drag_leave(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _| ui.drag.set_visible(false)
        ));
        drop.connect_drop(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            false,
            move |_, incoming, x, y| {
                ui.drag.set_visible(false);
                if !ui.in_drop_area(x, y) {
                    return false;
                }
                let incoming = incoming.clone();
                let generation = ui.generation.get();
                glib::spawn_future_local(async move {
                    match transfer::receive(&incoming).await {
                        Ok(data) if generation == ui.generation.get() => {
                            incoming.finish(gdk::DragAction::COPY);
                            ui.enqueue(data);
                        }
                        Ok(_) => incoming.finish(gdk::DragAction::empty()),
                        Err(error) => {
                            incoming.finish(gdk::DragAction::empty());
                            ui.show_status(&format!("Could not read dropped items ({error})"));
                        }
                    }
                });
                true
            }
        ));
        self.window.add_controller(drop);

        self.flow.connect_child_activated(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, child| {
                if let Some(uri) = ui.uri(child) {
                    ui.action(Action::Open, vec![uri]);
                }
            }
        ));
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| ui.key(key, modifiers)
        ));
        self.flow.add_controller(keys);
        self.bind_drag_source();
    }

    fn bind_drag_source(self: &Rc<Self>) {
        let drag = gtk::DragSource::new();
        // Start file drags before FlowBox can claim the gesture for rubber-band selection.
        drag.set_propagation_phase(gtk::PropagationPhase::Capture);
        drag.set_actions(gdk::DragAction::COPY);
        let preview = Rc::new(RefCell::new(None));
        drag.connect_prepare(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[strong]
            preview,
            #[upgrade_or]
            None,
            move |_, x, y| {
                if ui.stack.visible_child_name().as_deref() != Some("items")
                    || !ui.in_drop_area(x, y)
                {
                    return None;
                }
                let point = gtk::graphene::Point::new(x as f32, y as f32);
                let point = ui.window.compute_point(&ui.flow, &point)?;
                let row = ui.flow.child_at_pos(point.x() as i32, point.y() as i32)?;
                let local = ui.flow.compute_point(&row, &point)?;
                if row
                    .pick(
                        f64::from(local.x()),
                        f64::from(local.y()),
                        gtk::PickFlags::DEFAULT,
                    )
                    .is_some_and(|widget| {
                        widget.is::<gtk::Button>()
                            || widget.ancestor(gtk::Button::static_type()).is_some()
                    })
                {
                    return None;
                }
                let uri = ui.uri(&row)?;
                let selected = if row.is_selected() {
                    ui.selected()
                } else {
                    vec![uri.clone()]
                };
                let entries = ui.entries_for(&selected);
                let lead = entries.iter().find(|entry| entry.uri == uri)?;
                let entries: Vec<_> = entries
                    .iter()
                    .filter(|entry| entry.text.is_some() == lead.text.is_some())
                    .collect();
                if entries.is_empty() || entries.iter().any(|entry| entry.missing) {
                    ui.show_status("Could not start drag. A selected file is unavailable.");
                    return None;
                }
                preview.replace(Some(drag_preview(lead, entries.len())));
                if lead.text.is_some() {
                    let text = entries
                        .iter()
                        .filter_map(|entry| entry.text.as_deref())
                        .collect::<Vec<_>>()
                        .join("\n");
                    Some(transfer::text_provider(&text))
                } else {
                    let files: Vec<_> = entries.iter().map(|entry| entry.file()).collect();
                    Some(transfer::provider(&files))
                }
            }
        ));
        drag.connect_drag_begin(move |_, drag| {
            let preview = preview.borrow_mut().take();
            gtk::DragIcon::for_drag(drag).set_child(preview.as_ref());
        });
        self.window.add_controller(drag);
    }

    pub fn change_preferences(
        self: &Rc<Self>,
        resize: bool,
        change: impl FnOnce(&mut Preferences),
    ) {
        let old = self.prefs.get();
        let mut prefs = old;
        change(&mut prefs);
        self.prefs.set(prefs);
        if resize {
            let display = gtk::prelude::WidgetExt::display(&self.window);
            let monitor = self
                .window
                .surface()
                .and_then(|surface| display.monitor_at_surface(&surface));
            let (width, height) = monitor.map_or((prefs.width, prefs.height), |monitor| {
                let area = monitor.geometry();
                (
                    prefs.width.min(area.width()),
                    prefs.height.min(area.height()),
                )
            });
            self.window.set_default_size(width, height);
        }
        if old.theme != prefs.theme || old.opacity != prefs.opacity {
            self.apply_appearance();
        }
        if old.grid != prefs.grid
            || old.mode != prefs.mode
            || old.sort_by_name != prefs.sort_by_name
            || (old.width <= 340) != (prefs.width <= 340)
        {
            self.render();
        }
        if let Err(error) = self.storage.save_settings(&prefs) {
            self.show_status(&format!("Could not save settings ({error})"));
        }
        if old.clear_on_exit != prefs.clear_on_exit {
            self.persist_entries();
        }
    }

    fn apply_appearance(&self) {
        let prefs = self.prefs.get();
        let settings = gtk::Settings::for_display(&gtk::prelude::WidgetExt::display(&self.window));
        self.update_palette(apply_theme(&settings, prefs.theme));
    }

    fn update_palette(&self, dark: bool) {
        let prefs = self.prefs.get();
        let (background, header, surface, text, muted, border, accent, selected, danger) = if dark {
            (
                "#202326", "#24272a", "#282c2e", "#edf1f0", "#b3bdbb", "#414849", "#9be5c6",
                "#30413a", "#f4acaa",
            )
        } else {
            (
                "#f5f7f6", "#ebefed", "#ffffff", "#202a26", "#4a5f55", "#bbc9c2", "#23634d",
                "#dcebe4", "#922b37",
            )
        };
        self.appearance.load_from_data(&format!(
            "@define-color shelf_bg {background};\n@define-color shelf_header {header};\n
             @define-color shelf_surface {surface};\n@define-color shelf_text {text};\n
             @define-color shelf_muted {muted};\n@define-color shelf_border {border};\n
             @define-color shelf_accent {accent};\n@define-color shelf_selected {selected};\n
             @define-color shelf_danger {danger};\n
             @define-color shelf_chrome alpha({header}, {});",
            f64::from(prefs.opacity) / 100.0
        ));
    }

    fn show_status(&self, message: &str) {
        self.status.set_text(message);
        self.status
            .set_tooltip_text((!message.is_empty()).then_some(message));
        self.status.set_visible(!message.is_empty());
    }

    fn persist_entries(&self) {
        if let Err(error) = self
            .storage
            .save_entries(self.prefs.get().clear_on_exit, &self.shelf.borrow().entries)
        {
            self.show_status(&format!("Could not save shelf ({error})"));
        }
    }

    fn in_drop_area(&self, x: f64, y: f64) -> bool {
        let point = gtk::graphene::Point::new(x as f32, y as f32);
        self.stack
            .compute_bounds(&self.window)
            .is_some_and(|bounds| bounds.contains_point(&point))
    }

    fn show_shelf(&self) {
        let empty = self.shelf.borrow().entries.is_empty();
        self.stack
            .set_visible_child_name(if empty { "empty" } else { "items" });
    }

    fn enqueue(self: &Rc<Self>, data: transfer::DropData) {
        self.drops.borrow_mut().push_back(data);
        if self.draining.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            loop {
                let Some(data) = ui.drops.borrow_mut().pop_front() else {
                    break;
                };
                let generation = ui.generation.get();
                let mut entries = Vec::new();
                let files = match data {
                    transfer::DropData::Files(files) if files.is_empty() => continue,
                    transfer::DropData::Files(files) => files,
                    transfer::DropData::Text(entry) => {
                        entries.push(Ok(entry));
                        Vec::new()
                    }
                    transfer::DropData::Image(image) => {
                        if ui.shelf.borrow().entries.len() >= ui.prefs.get().max_items {
                            ui.show_status("The shelf is full.");
                            continue;
                        }
                        ui.show_status("Adding browser image…");
                        match image.save().await {
                            Ok(file) => vec![file],
                            Err(error) => {
                                if generation == ui.generation.get() {
                                    ui.show_status(&error);
                                }
                                continue;
                            }
                        }
                    }
                };
                for file in files {
                    if generation != ui.generation.get() {
                        break;
                    }
                    entries.push(shelf::inspect(&file).await);
                }
                if generation != ui.generation.get() {
                    continue;
                }
                let added = ui.shelf.borrow_mut().add(entries, ui.prefs.get().max_items);
                ui.show_status(&added.message());
                if !added.uris.is_empty() {
                    ui.recent.borrow_mut().extend(added.uris);
                    ui.render();
                    ui.persist_entries();
                    let current = ui.flash.get().wrapping_add(1);
                    ui.flash.set(current);
                    let weak = Rc::downgrade(&ui);
                    glib::timeout_add_local_once(Duration::from_millis(950), move || {
                        if let Some(ui) = weak.upgrade().filter(|ui| ui.flash.get() == current) {
                            ui.recent.borrow_mut().clear();
                            let mut child = ui.flow.first_child();
                            while let Some(row) = child {
                                child = row.next_sibling();
                                row.remove_css_class("recent");
                            }
                        }
                    });
                }
            }
            ui.draining.set(false);
        });
    }

    fn refresh_missing(self: &Rc<Self>) {
        if self.refreshing.replace(true) {
            return;
        }
        let entries = self.shelf.borrow().entries.clone();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let mut changed = false;
            for entry in entries {
                let missing = entry.text.is_none() && shelf::inspect(&entry.file()).await.is_err();
                if missing != entry.missing
                    && let Some(current) = ui
                        .shelf
                        .borrow_mut()
                        .entries
                        .iter_mut()
                        .find(|current| current.uri == entry.uri)
                {
                    current.missing = missing;
                    changed = true;
                }
            }
            ui.refreshing.set(false);
            if changed {
                ui.render();
            }
        });
    }

    fn uri(&self, child: &gtk::FlowBoxChild) -> Option<String> {
        usize::try_from(child.index())
            .ok()
            .and_then(|index| self.order.borrow().get(index).cloned())
    }

    fn selected(&self) -> Vec<String> {
        let mut children = self.flow.selected_children();
        children.sort_by_key(gtk::FlowBoxChild::index);
        children
            .iter()
            .filter_map(|child| self.uri(child))
            .collect()
    }

    fn focused_row(&self) -> Option<gtk::FlowBoxChild> {
        let focus = gtk::prelude::GtkWindowExt::focus(&self.window)?;
        focus
            .clone()
            .downcast::<gtk::FlowBoxChild>()
            .ok()
            .or_else(|| {
                focus
                    .ancestor(gtk::FlowBoxChild::static_type())
                    .and_downcast()
            })
    }

    fn key(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let Some(row) = self.focused_row() else {
            return glib::Propagation::Proceed;
        };
        let Some(uri) = self.uri(&row) else {
            return glib::Propagation::Proceed;
        };
        let selected = if row.is_selected() {
            self.selected()
        } else {
            vec![uri.clone()]
        };
        if key == gdk::Key::Menu
            || (key == gdk::Key::F10 && modifiers.contains(gdk::ModifierType::SHIFT_MASK))
        {
            self.context_menu(&row, uri, selected, None);
        } else if key == gdk::Key::Delete {
            self.action(Action::Remove, selected);
        } else if matches!(key, gdk::Key::c | gdk::Key::C)
            && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
        {
            self.action(Action::Copy, selected);
        } else {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    }

    fn render(self: &Rc<Self>) {
        if let Some(menu) = self.menu.borrow_mut().take() {
            menu.unparent();
        }
        let selected: HashSet<String> = self.selected().into_iter().collect();
        let focused = self.focused_row().and_then(|row| self.uri(&row));
        let had_focus = focused.is_some();
        while let Some(row) = self.flow.first_child() {
            self.flow.remove(&row);
        }
        let prefs = self.prefs.get();
        let mut entries = self.shelf.borrow().entries.clone();
        if prefs.sort_by_name {
            entries.sort_by_cached_key(|entry| entry.name.to_lowercase());
        }
        self.order
            .replace(entries.iter().map(|entry| entry.uri.clone()).collect());
        self.flow
            .set_max_children_per_line(if prefs.grid { 8 } else { 1 });
        self.flow.set_homogeneous(prefs.grid);
        self.flow.set_row_spacing(if prefs.grid { 8 } else { 3 });
        let count = entries.len();
        self.counter.set_text(&count.to_string());
        set_label(&self.counter, &item_count(count));
        self.clear.set_sensitive(count > 0);
        let mut refocus = None;
        for entry in &entries {
            let row = self.row(entry, prefs);
            self.flow.insert(&row, -1);
            if selected.contains(&entry.uri) {
                self.flow.select_child(&row);
            }
            if focused.as_deref() == Some(entry.uri.as_str()) {
                refocus = Some(row);
            }
        }
        self.show_shelf();
        if had_focus {
            if let Some(row) = refocus.or_else(|| self.flow.child_at_index(0)) {
                row.grab_focus();
            } else {
                self.layout.grab_focus();
            }
        }
    }

    fn row(self: &Rc<Self>, entry: &Entry, prefs: Preferences) -> gtk::FlowBoxChild {
        let row = gtk::FlowBoxChild::new();
        row.set_focusable(true);
        row.set_tooltip_text(Some(&entry.name));
        set_label(&row, &entry.name);
        row.add_css_class("file-row");
        let compact = !prefs.grid && prefs.mode == Mode::Compact && prefs.width <= 340;
        if prefs.grid {
            row.add_css_class("grid-file");
            row.set_size_request(172, -1);
        }
        if self.recent.borrow().contains(&entry.uri) {
            row.add_css_class("recent");
        }
        let body = gtk::Box::new(
            if prefs.grid {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            },
            8,
        );
        let tops = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let artwork = file_artwork(entry, compact, prefs.grid);
        tops.append(&artwork);
        let text = gtk::Box::new(
            if compact {
                gtk::Orientation::Horizontal
            } else {
                gtk::Orientation::Vertical
            },
            3,
        );
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        let split = Path::new(&entry.name);
        let shown_name = if compact || entry.text.is_some() {
            entry.name.as_str()
        } else {
            split
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&entry.name)
        };
        let name = gtk::Label::new(Some(shown_name));
        name.set_xalign(0.0);
        name.set_ellipsize(pango::EllipsizeMode::End);
        name.set_max_width_chars(1);
        name.set_hexpand(true);
        name.add_css_class("file-name");
        text.append(&name);
        let size = entry.size.map(|size| glib::format_size(size).to_string());
        let extension = split
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_uppercase);
        let details = if entry.missing {
            "File unavailable".into()
        } else if entry.text.is_some() {
            "Plain text".into()
        } else if compact {
            size.unwrap_or_else(|| "Unknown size".into())
        } else {
            extension
                .into_iter()
                .chain(size)
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let meta = gtk::Label::new(Some(&details));
        meta.set_xalign(0.0);
        meta.set_ellipsize(pango::EllipsizeMode::End);
        meta.add_css_class(if entry.missing { "missing" } else { "muted" });
        text.append(&meta);
        let remove = icon_button(
            "window-close-symbolic",
            &format!("Remove {} from Shelf", entry.name),
        );
        remove.add_css_class("remove-file");
        let uri = entry.uri.clone();
        remove.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| ui.action(Action::Remove, vec![uri.clone()])
        ));
        if prefs.grid {
            artwork.set_hexpand(true);
            artwork.set_halign(gtk::Align::Start);
            tops.append(&remove);
            body.append(&tops);
            body.append(&text);
        } else {
            body.append(&tops);
            body.append(&text);
            body.append(&remove);
        }
        row.set_child(Some(&body));

        let secondary = gtk::GestureClick::new();
        secondary.set_button(3);
        let uri = entry.uri.clone();
        secondary.connect_pressed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[weak]
            row,
            move |gesture, _, x, y| {
                if !row.is_selected() {
                    ui.flow.unselect_all();
                    ui.flow.select_child(&row);
                }
                row.grab_focus();
                let selected = ui.selected();
                ui.context_menu(
                    &row,
                    uri.clone(),
                    selected,
                    Some(gdk::Rectangle::new(x as i32, y as i32, 1, 1)),
                );
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        ));
        row.add_controller(secondary);

        row
    }

    fn entries_for(&self, uris: &[String]) -> Vec<Entry> {
        let shelf = self.shelf.borrow();
        uris.iter()
            .filter_map(|uri| {
                shelf
                    .entries
                    .iter()
                    .find(|entry| &entry.uri == uri)
                    .cloned()
            })
            .collect()
    }

    fn context_menu(
        self: &Rc<Self>,
        row: &gtk::FlowBoxChild,
        uri: String,
        selected: Vec<String>,
        pointing_to: Option<gdk::Rectangle>,
    ) {
        if let Some(old) = self.menu.borrow_mut().take() {
            old.unparent();
        }
        let group = gio::SimpleActionGroup::new();
        let items = gio::Menu::new();
        let removal = gio::Menu::new();
        let is_text = self
            .entries_for(std::slice::from_ref(&uri))
            .first()
            .is_some_and(|entry| entry.text.is_some());
        for (name, label, icon, accel, action, single) in [
            (
                "open",
                "Open",
                "document-open-symbolic",
                Some("Return"),
                Action::Open,
                true,
            ),
            (
                "reveal",
                "Reveal in File Manager",
                "folder-symbolic",
                None,
                Action::Reveal,
                true,
            ),
            (
                "copy",
                "Copy",
                "edit-copy-symbolic",
                Some("<Control>c"),
                Action::Copy,
                false,
            ),
            (
                "remove",
                "Remove from Shelf",
                "window-close-symbolic",
                Some("Delete"),
                Action::Remove,
                false,
            ),
        ] {
            if single && is_text {
                continue;
            }
            let item = gio::SimpleAction::new(name, None);
            let targets = if single {
                vec![uri.clone()]
            } else {
                selected.clone()
            };
            item.connect_activate(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |_, _| {
                    let menu = ui.menu.borrow_mut().take();
                    if let Some(menu) = menu {
                        menu.unparent();
                    }
                    ui.action(action, targets.clone());
                }
            ));
            group.add_action(&item);
            let section = if name == "remove" { &removal } else { &items };
            let menu_item = gio::MenuItem::new(Some(label), Some(&format!("file.{name}")));
            menu_item.set_icon(&gio::ThemedIcon::new(icon));
            if let Some(accel) = accel {
                menu_item.set_attribute_value("accel", Some(&accel.to_variant()));
            }
            section.append_item(&menu_item);
        }
        items.append_section(None, &removal);
        let menu = gtk::PopoverMenu::from_model(Some(&items));
        menu.insert_action_group("file", Some(&group));
        menu.set_has_arrow(false);
        menu.set_pointing_to(pointing_to.as_ref());
        menu.set_parent(row);
        menu.connect_closed(glib::clone!(
            #[weak]
            row,
            move |_| {
                let row = row.downgrade();
                glib::idle_add_local_once(move || {
                    if let Some(row) = row.upgrade().filter(|row| row.root().is_some()) {
                        row.grab_focus();
                    }
                });
            }
        ));
        self.menu.replace(Some(menu.clone()));
        menu.popup();
    }

    fn action(self: &Rc<Self>, action: Action, uris: Vec<String>) {
        if matches!(action, Action::Remove) {
            self.shelf.borrow_mut().remove(&uris);
            self.render();
            self.show_status("");
            self.persist_entries();
            return;
        }
        let entries = self.entries_for(&uris);
        if entries.is_empty() {
            return;
        }
        if entries[0].text.is_some() && !matches!(action, Action::Copy) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let mut files = Vec::new();
            let mut texts = Vec::new();
            let verb = match action {
                Action::Open => "open",
                Action::Reveal => "reveal",
                Action::Copy => "copy",
                Action::Remove => unreachable!(),
            };
            for entry in entries {
                if let Some(text) = entry.text {
                    texts.push(text);
                    continue;
                }
                let file = entry.file();
                if shelf::inspect(&file).await.is_err() {
                    if let Some(current) = ui
                        .shelf
                        .borrow_mut()
                        .entries
                        .iter_mut()
                        .find(|current| current.uri == entry.uri)
                    {
                        current.missing = true;
                    }
                    ui.render();
                    ui.show_status(&format!(
                        "Could not {verb} {}. File unavailable.",
                        entry.name
                    ));
                    return;
                }
                files.push(file);
            }
            let display = gtk::prelude::WidgetExt::display(&ui.window);
            let context = display.app_launch_context();
            let result = match action {
                Action::Open => transfer::open(&files[0], context.upcast_ref())
                    .await
                    .map_err(|error| error.to_string()),
                Action::Reveal => transfer::reveal(&files[0], context.upcast_ref())
                    .await
                    .map_err(|error| error.to_string()),
                Action::Copy => {
                    let mut providers = Vec::new();
                    if !files.is_empty() {
                        providers.push(transfer::clipboard_provider(&files));
                    }
                    if !texts.is_empty() {
                        providers.push(transfer::text_provider(&texts.join("\n")));
                    }
                    let content = gdk::ContentProvider::new_union(&providers);
                    display
                        .clipboard()
                        .set_content(Some(&content))
                        .map_err(|error| error.to_string())
                }
                Action::Remove => unreachable!(),
            };
            if let Err(error) = result {
                ui.show_status(&format!("Could not {verb} ({error})"));
            } else if matches!(action, Action::Copy) {
                ui.show_status(&format!("Copied {}", item_count(files.len() + texts.len())));
            }
        });
    }
}

fn system_dark(settings: &gtk::Settings) -> bool {
    let property = "gtk-interface-color-scheme";
    if settings.find_property(property).is_some() {
        let value = settings.property_value(property);
        if let Some((_, choice)) = glib::EnumValue::from_value(&value) {
            match choice.nick() {
                "dark" => return true,
                "light" => return false,
                _ => {}
            }
        }
    }
    settings.is_gtk_application_prefer_dark_theme()
        || settings
            .gtk_theme_name()
            .is_some_and(|name| name.to_lowercase().ends_with("-dark"))
}

fn apply_theme(settings: &gtk::Settings, theme: Theme) -> bool {
    let property = "gtk-interface-color-scheme";
    if let Some(spec) = settings.find_property(property) {
        match theme {
            Theme::System => settings.reset_property(property),
            Theme::Dark | Theme::Light => {
                let choice = if theme == Theme::Dark {
                    "dark"
                } else {
                    "light"
                };
                if let Some(value) = glib::EnumClass::with_type(spec.value_type())
                    .and_then(|class| class.to_value_by_nick(choice))
                {
                    settings.set_property_from_value(property, &value);
                }
            }
        }
    } else {
        let preference = "gtk-application-prefer-dark-theme";
        match theme {
            Theme::System => settings.reset_property(preference),
            Theme::Dark | Theme::Light => settings.set_property(preference, theme == Theme::Dark),
        }
    }
    match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => system_dark(settings),
    }
}

pub fn header(title: &str, counter: Option<&gtk::Label>) -> gtk::HeaderBar {
    let header = gtk::HeaderBar::new();
    header.add_css_class("shelf-header");
    header.set_decoration_layout(Some(":close"));
    let group = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    group.set_valign(gtk::Align::Center);
    let image = svg_image(LOGO, 28);
    group.append(&image);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("title");
    group.append(&label);
    if let Some(counter) = counter {
        group.append(counter);
    }
    header.pack_start(&group);
    header.set_title_widget(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    header
}

pub fn set_label(widget: &impl IsA<gtk::Accessible>, text: &str) {
    widget.update_property(&[gtk::accessible::Property::Label(text)]);
}

fn icon_button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(label));
    set_label(&button, label);
    button
}

fn file_artwork(entry: &Entry, compact: bool, grid: bool) -> gtk::Widget {
    let icon = gtk::Image::from_gicon(&gio::content_type_get_icon(&entry.content_type));
    let icon_size = if compact { 24 } else { 29 };
    icon.set_pixel_size(icon_size);
    icon.set_valign(gtk::Align::Center);
    if entry.missing || !entry.content_type.starts_with("image/") {
        return icon.upcast();
    }

    let (width, height) = if grid {
        (64, 48)
    } else {
        (icon_size, icon_size)
    };
    let slot = gtk::Overlay::new();
    slot.set_size_request(width, height);
    slot.set_valign(gtk::Align::Center);
    slot.set_child(Some(&icon));
    let picture = gtk::Picture::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_fit(gtk::ContentFit::Contain)
        .can_target(false)
        .visible(false)
        .build();
    slot.add_overlay(&picture);

    let file = entry.file();
    let icon = icon.downgrade();
    let preview = picture.downgrade();
    let task = glib::spawn_future_local(async move {
        // The unmeasured overlay keeps the layout fixed while rendering at up to 2× density.
        if let Some(pixbuf) = load_thumbnail(&file, width.max(height) * 2).await
            && let (Some(picture), Some(icon)) = (preview.upgrade(), icon.upgrade())
        {
            picture.set_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));
            picture.set_visible(true);
            icon.set_opacity(0.0);
        }
    });
    picture.connect_destroy(move |_| task.abort());
    slot.upcast()
}

async fn load_thumbnail(file: &gio::File, edge: i32) -> Option<Pixbuf> {
    let stream = file.read_future(glib::Priority::DEFAULT).await.ok()?;
    let image = Pixbuf::from_stream_at_scale_future(&stream, edge, edge, true)
        .await
        .ok()?;
    Some(image.apply_embedded_orientation().unwrap_or(image))
}

fn svg_image(bytes: &[u8], size: i32) -> gtk::Image {
    let loader = gtk::gdk_pixbuf::PixbufLoader::with_type("svg").expect("SVG loader");
    loader.connect_size_prepared(move |loader, width, height| {
        loader.set_size(size, (height * size) / width);
    });
    loader.write(bytes).expect("bundled SVG");
    loader.close().expect("complete SVG");
    let image = gtk::Image::from_pixbuf(loader.pixbuf().as_ref());
    image.set_pixel_size(size);
    image
}

fn placeholder(svg: &[u8], title: &str, description: &str) -> (gtk::Box, gtk::Label) {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("drop-area");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_valign(gtk::Align::Center);
    body.set_vexpand(true);
    let icon = svg_image(svg, 42);
    icon.set_margin_bottom(6);
    body.append(&icon);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("empty-title");
    body.append(&title);
    let description = gtk::Label::new(Some(description));
    description.set_wrap(true);
    description.set_justify(gtk::Justification::Center);
    description.add_css_class("muted");
    description.set_visible(!description.text().is_empty());
    body.append(&description);
    outer.append(&body);
    (outer, description)
}

fn item_count(count: usize) -> String {
    if count == 1 {
        "1 item".into()
    } else {
        format!("{count} items")
    }
}

fn drag_preview(lead: &Entry, count: usize) -> gtk::Box {
    let icon = gtk::Image::from_gicon(&gio::content_type_get_icon(&lead.content_type));
    icon.set_pixel_size(26);
    let frame = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    frame.add_css_class("drag-preview");
    let details = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let name = gtk::Label::new(Some(&lead.name));
    name.set_xalign(0.0);
    name.set_ellipsize(pango::EllipsizeMode::End);
    name.set_max_width_chars(28);
    let subtitle = if count > 1 {
        format!("+{} more · Copy", count - 1)
    } else {
        "Copy".into()
    };
    let subtitle = gtk::Label::new(Some(&subtitle));
    subtitle.add_css_class("muted");
    subtitle.set_xalign(0.0);
    details.append(&name);
    details.append(&subtitle);
    frame.append(&icon);
    frame.append(&details);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn corrupt_or_missing_files_fall_back_to_file_icons() {
        let dir =
            std::env::temp_dir().join(format!("shelfdrop-preview-{}", glib::uuid_string_random()));
        fs::create_dir(&dir).unwrap();
        let corrupt = dir.join("broken.png");
        fs::write(&corrupt, b"not an image").unwrap();

        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    assert!(
                        load_thumbnail(&gio::File::for_path(&corrupt), 32)
                            .await
                            .is_none()
                    );
                    assert!(
                        load_thumbnail(&gio::File::for_path(dir.join("missing.png")), 32)
                            .await
                            .is_none()
                    );
                })
            })
            .unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
