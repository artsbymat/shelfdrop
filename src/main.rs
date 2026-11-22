mod browser_image;
mod settings;
mod shelf;
mod storage;
mod transfer;
mod ui;

use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

const APP_ID: &str = "io.github.shelfdrop.Shelfdrop";

fn main() -> gtk::glib::ExitCode {
    let app = gtk::Application::new(Some(APP_ID), Default::default());
    let session = Rc::new(RefCell::new(None::<Rc<ui::Ui>>));
    app.connect_activate({
        let session = session.clone();
        move |app| {
            let mut session = session.borrow_mut();
            let ui = session.get_or_insert_with(|| ui::Ui::new(app));
            ui.window.present();
        }
    });
    let result = app.run();
    session.borrow_mut().take();
    result
}
