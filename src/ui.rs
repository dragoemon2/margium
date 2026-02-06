use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Label, Orientation, Separator, FileChooserDialog, FileChooserAction, ResponseType};
use std::cell::RefCell;
use std::rc::Rc;
use crate::engine::PdfEngine;

// モジュール宣言 (フォルダ構成に合わせて配置してください)
mod toolbar;
mod main_content;
mod popover_menu;
mod button_event;
mod sidebar;

pub struct UiState {
    pub scale: f64,
    pub last_click_pos: Option<(f64, f64)>,
}

pub fn build(app: &Application) {
    // 1. 初期化
    let engine = Rc::new(RefCell::new(PdfEngine::new()));
    let ui_state = Rc::new(RefCell::new(UiState {
        scale: 1.0,
        last_click_pos: None,
    }));

    // 2. ウィンドウ構築
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Margium")
        .default_width(1200)
        .default_height(800)
        .build();

    // 3. メインビュー (DrawingArea + TextView) の構築
    let (view_container, drawing_area, text_buffer) = 
        main_content::build(engine.clone(), ui_state.clone());

    // 4. ツールバーの構築
    let filename_label = Label::new(Some("No File Selected"));
    let widgets = toolbar::build(&filename_label);
    
    // ポップオーバー (アノテーション用)
    popover_menu::setup(&window, &drawing_area, engine.clone(), ui_state.clone());

    // サイドバー
    let sidebar = Rc::new(sidebar::build(
        engine.clone(), 
        &drawing_area
    ));

    // --- レイアウト配置 ---

    let main_layout = gtk4::Box::new(Orientation::Horizontal, 0);
    window.set_child(Some(&main_layout));

    main_layout.append(&sidebar.container);
    main_layout.append(&Separator::new(Orientation::Vertical));

    let content_box = gtk4::Box::new(Orientation::Vertical, 0);
    content_box.set_hexpand(true);
    content_box.append(&widgets.container);
    content_box.append(&view_container);

    main_layout.append(&content_box);

    // ロジック接続
    button_event::setup(
        &window,
        engine.clone(),
        ui_state.clone(),
        &widgets,
        &drawing_area,
        &sidebar,
        &text_buffer,
        &filename_label
    );

    window.present();
}