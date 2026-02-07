// ui/sidebar/search.rs

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, 
    SearchEntry, Align
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use poppler::Rectangle;

// 検索結果データ
#[derive(Clone)]
pub struct SearchResult {
    pub page: i32,
    pub display_text: String,
    pub req_id: usize,
    pub rects: Vec<Rectangle>,
}

pub struct SearchWidget {
    pub box_container: GtkBox,
    pub entry: SearchEntry,
    pub list: ListBox,
    pub result_label: Label,
    pub results_data: RefCell<Vec<SearchResult>>
}

impl SearchWidget {
    pub fn new(
        engine: Rc<RefCell<PdfEngine>>,
        drawing_area: &gtk4::DrawingArea,
    ) -> Self {
        let box_container = GtkBox::new(Orientation::Vertical, 5);
        
        let entry = SearchEntry::new();
        entry.set_placeholder_text(Some("Search text..."));
        
        let result_label = Label::new(Some("Ready"));
        result_label.add_css_class("caption");
        
        let list = ListBox::new();
        let scroll = ScrolledWindow::builder().child(&list).vexpand(true).build();
        
        box_container.append(&entry);
        box_container.append(&result_label);
        box_container.append(&scroll);

        // --- Click Logic ---
        
        
        

        Self { box_container, entry, list, result_label , results_data: RefCell::new(Vec::new()) }
    }

    pub fn clear_results(&self) {
        self.results_data.borrow_mut().clear();
        
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.result_label.set_text("");
    }

    pub fn set_status(&self, text: &str) {
        self.result_label.set_text(text);
    }


    pub fn append_result(&self, res: SearchResult) {
        // ★データをリストに追加し、そのインデックスを取得
        let mut data_list = self.results_data.borrow_mut();
        let idx = data_list.len();
        data_list.push(res.clone());

        let row = ListBoxRow::new();
        // ★ウィジェット名にはインデックスを埋め込む
        row.set_widget_name(&idx.to_string());

        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_margin_top(5);
        vbox.set_margin_bottom(5);

        let page_lbl = Label::new(Some(&format!("Page {}", res.page + 1)));
        page_lbl.set_halign(Align::Start);
        page_lbl.add_css_class("caption-heading");

        // display_textの代わりにdisplay_textを表示
        let ctx_lbl = Label::new(Some(&res.display_text));
        ctx_lbl.set_halign(Align::Start);
        // ... (省略) ...
        vbox.append(&page_lbl);
        vbox.append(&ctx_lbl);
        row.set_child(Some(&vbox));

        self.list.append(&row);
    }

    // ★追加: インデックスから結果データを取得するメソッド
    pub fn get_result_data(&self, idx: usize) -> Option<SearchResult> {
        self.results_data.borrow().get(idx).cloned()
    }
}