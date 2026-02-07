use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, 
    SelectionMode, Align
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;

pub struct AnnotationWidget {
    pub list: ListBox,
    pub scroll: ScrolledWindow,
}

impl AnnotationWidget {
    pub fn new(
        engine: Rc<RefCell<PdfEngine>>,
        drawing_area: &gtk4::DrawingArea,
    ) -> Self {
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::None);

        let scroll = ScrolledWindow::builder().child(&list).build();

        // --- Click Logic ---
        let eng_annot = engine.clone();
        let area_annot = drawing_area.clone();
        
        list.connect_row_activated(move |_, row| {
            let name = row.widget_name();
            let s = name.as_str();
            let parts: Vec<&str> = s.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(p) = parts[0].parse::<i32>() {
                    if eng_annot.borrow_mut().jump_to_page(p) {
                        area_annot.queue_draw();
                    }
                }
            }
        });

        Self { list, scroll }
    }

    pub fn update_annotations(&self, engine: &PdfEngine) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        if engine.annotations.is_empty() {
            let l = Label::new(Some("No annotations"));
            l.set_margin_top(10);
            self.list.append(&l);
            return;
        }

        for ann in &engine.annotations {
            let row = ListBoxRow::new();
            // クリック用のメタデータ (pageは内部0-basedに変換)
            row.set_widget_name(&format!("{},{}", ann.page as i32 - 1, ann.y));

            let vbox = GtkBox::new(Orientation::Vertical, 2);

            let page_lbl = Label::new(Some(&format!("Page {}", ann.page)));
            page_lbl.set_halign(Align::Start);
            page_lbl.add_css_class("caption-heading");

            let content_lbl = Label::new(Some(&ann.content));
            content_lbl.set_halign(Align::Start);
            content_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            content_lbl.set_max_width_chars(20);

            vbox.append(&page_lbl);
            vbox.append(&content_lbl);
            row.set_child(Some(&vbox));
            self.list.append(&row);
        }
    }
}