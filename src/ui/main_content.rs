use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, DrawingArea, Orientation, Paned, ScrolledWindow, 
    TextView, TextBuffer, Separator, Label,
    GestureClick, EventControllerScroll, EventControllerScrollFlags,
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use crate::ui::UiState;

// 戻り値:
// 1. GtkBox: レイアウト全体の親コンテナ
// 2. DrawingArea: PDF描画用（再描画指示などで使う）
// 3. TextBuffer: テキスト更新用
// 4. EventControllerScroll: スクロールイベント用（ui.rsでページ送りロジックを接続するため）
pub fn build(
    engine: Rc<RefCell<PdfEngine>>,
    ui_state: Rc<RefCell<UiState>>,
) -> (GtkBox, DrawingArea, TextBuffer) {
    
    // --- レイアウト作成 ---
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.set_hexpand(true);

    // ツールバーとの境界線
    let h_sep = Separator::new(Orientation::Horizontal);
    container.append(&h_sep);

    // 左右分割 (Paned)
    let paned = Paned::new(Orientation::Horizontal);
    paned.set_position(600);
    paned.set_vexpand(true);
    container.append(&paned);

    // [左] PDFエリア
    let drawing_area = DrawingArea::new();
    drawing_area.set_content_width(600);
    drawing_area.set_content_height(800);
    // キーボード入力を受け取るためにフォーカス可能にする
    drawing_area.set_focusable(true);
    drawing_area.set_can_focus(true);

    let pdf_scroll_window = ScrolledWindow::builder()
        .child(&drawing_area)
        .build();
    paned.set_start_child(Some(&pdf_scroll_window));

    // [右] テキストエリア
    let text_view = TextView::new();
    text_view.set_editable(false);
    text_view.set_wrap_mode(gtk4::WrapMode::WordChar);
    text_view.set_left_margin(10);
    text_view.set_right_margin(10);
    text_view.set_top_margin(10);
    text_view.set_bottom_margin(10);
    
    let text_buffer = text_view.buffer();
    
    let text_scroll = ScrolledWindow::builder()
        .child(&text_view)
        .build();
    text_scroll.set_size_request(200, -1);
    paned.set_end_child(Some(&text_scroll));


    // ============================================================
    // ロジック設定
    // ============================================================

    // 1. 描画ロジック (単一ページ用)
    let eng_draw = engine.clone();
    let ui_draw = ui_state.clone();
    
    drawing_area.set_draw_func(move |area, ctx, w, h| {
        let eng = eng_draw.borrow();
        let ui = ui_draw.borrow();
        
        // エンジンに描画させる
        eng.draw(ctx, w as f64, h as f64, ui.scale);

        // ★重要: 単一ページモードにおけるサイズ調整
        // ズーム倍率に合わせて DrawingArea のサイズ（content_size）を更新する。
        // これにより、拡大時に自動的にスクロールバーが表示されるようになる。
        if let Some((pdf_w, pdf_h)) = eng.get_page_size() {
            let req_w = (pdf_w * ui.scale) as i32;
            let req_h = (pdf_h * ui.scale) as i32 + 40; // 上下余白分
            
            // 無限ループを防ぐため、サイズが異なるときだけセット
            if area.content_width() != req_w || area.content_height() != req_h {
                area.set_content_width(req_w);
                area.set_content_height(req_h);
            }
        }
    });

    // 2. クリックでフォーカス取得 (キーボード操作用)
    let click_ctrl = GestureClick::new();
    let area_focus = drawing_area.clone();
    click_ctrl.connect_pressed(move |_, _, _, _| {
        area_focus.grab_focus();
    });
    drawing_area.add_controller(click_ctrl);

    // 3. スクロールコントローラーの作成
    // ロジック（ページ送り）はここには書かず、コントローラーだけ作って親(ui.rs)に渡す
    let scroll_ctrl = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    pdf_scroll_window.add_controller(scroll_ctrl.clone());

    (container, drawing_area, text_buffer)
}

// サイドバー作成用ヘルパー
pub fn build_sidebar() -> GtkBox {
    let sidebar = GtkBox::new(Orientation::Vertical, 0);
    sidebar.set_width_request(250);
    
    let label = Label::new(Some("Sidebar"));
    label.set_margin_top(10);
    sidebar.append(&label);
    
    sidebar
}