use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, DrawingArea, Entry, GestureClick, Label, 
    Orientation, Popover
};
use gtk4::ApplicationWindow;
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use crate::ui::UiState;

pub fn setup(
    window: &ApplicationWindow,
    drawing_area: &DrawingArea,
    engine: Rc<RefCell<PdfEngine>>,
    ui_state: Rc<RefCell<UiState>>,
) {
    // 1. Create Popover UI
    let popover = Popover::builder().has_arrow(false).build();
    let menu_box = GtkBox::new(Orientation::Vertical, 0);
    let add_annot_btn = Button::with_label(" ➕ Add Annotation ");
    add_annot_btn.set_has_frame(false);
    menu_box.append(&add_annot_btn);
    
    popover.set_child(Some(&menu_box));
    popover.set_parent(drawing_area);

    // 2. Right Click Handler (Show Popover)
    let right_click = GestureClick::new();
    right_click.set_button(3); // Right click
    
    let ui_click = ui_state.clone();
    let popover_click = popover.clone();

    right_click.connect_pressed(move |_, _, x, y| {
        // Save click position
        ui_click.borrow_mut().last_click_pos = Some((x, y));

        // Show popover
        let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        popover_click.set_pointing_to(Some(&rect));
        popover_click.popup();
    });
    drawing_area.add_controller(right_click);


    // 3. Annotation Dialog Logic
    let engine_add = engine.clone();
    let ui_add = ui_state.clone();
    let area_add = drawing_area.clone();
    let popover_action = popover.clone();
    let window_weak = window.downgrade();

    add_annot_btn.connect_clicked(move |_| {
        popover_action.popdown();

        // Calculate Position
        let ui = ui_add.borrow();
        let (click_x, click_y) = match ui.last_click_pos {
            Some(pos) => pos,
            None => return,
        };
        
        let pdf_x = click_x / ui.scale;
        let pdf_y = click_y / ui.scale;

        // Create Dialog
        let parent = window_weak.upgrade().unwrap();
        show_annotation_dialog(&parent, engine_add.clone(), area_add.clone(), pdf_x, pdf_y);
    });
}

fn show_annotation_dialog(
    parent: &ApplicationWindow,
    engine: Rc<RefCell<PdfEngine>>,
    drawing_area: DrawingArea,
    x: f64,
    y: f64
) {
    let dialog = ApplicationWindow::builder()
        .title("Annotation Text")
        .transient_for(parent)
        .modal(true)
        .default_width(300)
        .default_height(100)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 10);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(20);
    vbox.set_margin_start(20);
    vbox.set_margin_end(20);

    let entry = Entry::new();
    entry.set_text("New Note");
    entry.set_activates_default(true);

    let btn_box = GtkBox::new(Orientation::Horizontal, 10);
    btn_box.set_halign(gtk4::Align::Center);
    
    let btn_cancel = Button::with_label("Cancel");
    let btn_ok = Button::with_label("OK");
    dialog.set_default_widget(Some(&btn_ok));

    btn_box.append(&btn_cancel);
    btn_box.append(&btn_ok);
    vbox.append(&Label::new(Some("Enter text:")));
    vbox.append(&entry);
    vbox.append(&btn_box);
    dialog.set_child(Some(&vbox));

    // Actions
    let dialog_close = dialog.clone();
    btn_cancel.connect_clicked(move |_| dialog_close.close());

    let dialog_ok = dialog.clone();
    let entry_clone = entry.clone();
    
    btn_ok.connect_clicked(move |_| {
        let text = entry_clone.text();
        if !text.is_empty() {
            let mut eng = engine.borrow_mut();
            // Note: In continuous scroll mode, you might need to find which page 
            // this Y coordinate belongs to. For now, we just pass raw coords.
            if let Err(e) = eng.add_annotation(&text, x, y) {
                eprintln!("Error: {}", e);
            } else {
                drawing_area.queue_draw();
            }
        }
        dialog_ok.close();
    });

    dialog.present();
}