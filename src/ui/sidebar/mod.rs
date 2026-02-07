use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Orientation, Stack, StackTransitionType, Align
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;

// サブモジュールの公開
pub mod thumbnail;
pub mod outline;
pub mod annotation;
pub mod search;

// 構造体をインポート
use thumbnail::ThumbnailWidget;
use outline::OutlineWidget;
use annotation::AnnotationWidget;
use search::SearchWidget;

// 非同期通信などで使う型を再エクスポート
pub use thumbnail::ThumbnailResult;
// pub use search::SearchResult; // 必要に応じて

pub struct SidebarWidgets {
    pub stack: Stack,
    pub container: GtkBox,
    pub thumbnails: ThumbnailWidget,
    pub outline: OutlineWidget,
    pub annotations: AnnotationWidget,
    pub search: SearchWidget,
}


pub fn build(
    engine: Rc<RefCell<PdfEngine>>,
    drawing_area: &gtk4::DrawingArea,
) -> SidebarWidgets {
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.set_width_request(250);
    container.set_hexpand(false);

    // --- 1. Tab Header ---
    let tab_box = GtkBox::new(Orientation::Horizontal, 0);
    tab_box.add_css_class("linked");
    tab_box.set_halign(Align::Center);
    tab_box.set_margin_top(5);
    tab_box.set_margin_bottom(5);

    let btn_thumbs = create_tab_button("📄", "thumbs");
    let btn_outline = create_tab_button("📑", "outline");
    let btn_annots = create_tab_button("📝", "annots");
    let btn_search = create_tab_button("🔍", "search");

    tab_box.append(&btn_thumbs);
    tab_box.append(&btn_outline);
    tab_box.append(&btn_annots);
    tab_box.append(&btn_search);
    container.append(&tab_box);

    // --- 2. Main Stack ---
    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.set_transition_type(StackTransitionType::SlideLeftRight);

    // 各ウィジェットの初期化
    let thumbnails = ThumbnailWidget::new(engine.clone(), drawing_area);
    let outline = OutlineWidget::new(engine.clone(), drawing_area);
    let annotations = AnnotationWidget::new(engine.clone(), drawing_area);
    let search = SearchWidget::new(engine.clone(), drawing_area);

    // スタックに追加 (各ウィジェットはスクロールウィンドウやコンテナを持っている)
    stack.add_named(&thumbnails.scroll, Some("thumbs"));
    stack.add_named(&outline.scroll, Some("outline"));
    stack.add_named(&annotations.scroll, Some("annots"));
    stack.add_named(&search.box_container, Some("search"));

    container.append(&stack);

    // --- Tab Switching Logic ---
    let s = stack.clone(); btn_thumbs.connect_clicked(move |_| s.set_visible_child_name("thumbs"));
    let s = stack.clone(); btn_outline.connect_clicked(move |_| s.set_visible_child_name("outline"));
    let s = stack.clone(); btn_annots.connect_clicked(move |_| s.set_visible_child_name("annots"));
    let s = stack.clone(); btn_search.connect_clicked(move |_| s.set_visible_child_name("search"));

    SidebarWidgets {
        stack,
        container,
        thumbnails,
        outline,
        annotations,
        search,
    }
}

fn create_tab_button(label: &str, _name: &str) -> Button {
    Button::builder().label(label).has_frame(false).build()
}