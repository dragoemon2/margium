use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, DrawingArea, Orientation, Paned, ScrolledWindow, 
    TextView, TextBuffer, Separator, 
    GestureClick, EventControllerScroll, EventControllerScrollFlags, GestureDrag
};
use std::rc::Rc;
use std::cell::RefCell;
use crate::engine::PdfEngine;
use crate::ui::UiState;

// 戻り値:
// 1. GtkBox: レイアウト全体の親コンテナ
// 2. DrawingArea: PDF描画用（再描画指示などで使う）
// 3. TextBuffer: テキスト更新用
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

    // UI -> PDF座標変換のヘルパー関数
    let convert_to_pdf_coords = |ui_x: f64, ui_y: f64, eng: &PdfEngine, ui_scale: f64, area_w: f64| -> (f64, f64) {
        if let Some((pdf_w, _)) = eng.get_page_size() {
            let draw_w = pdf_w * ui_scale;
            let offset_x = if area_w > draw_w { (area_w - draw_w) / 2.0 } else { 0.0 };
            let offset_y = 20.0;
            
            let pdf_x = (ui_x - offset_x) / ui_scale;
            let pdf_y = (ui_y - offset_y) / ui_scale;
            (pdf_x, pdf_y)
        } else {
            (0.0, 0.0)
        }
    };

    // 2. クリック
    let click_ctrl = GestureClick::new();
    let eng_click = engine.clone();
    let ui_click = ui_state.clone();
    let area_click = drawing_area.clone();

    click_ctrl.connect_pressed(move |_, _, x, y| {
        let mut eng = eng_click.borrow_mut();
        let scale = ui_click.borrow().scale;
        let area_w = area_click.width() as f64;
        
        let (pdf_x, pdf_y) = convert_to_pdf_coords(x, y, &eng, scale, area_w);

        // 1. まずクリックした位置にアノテーションがあるか判定
        if let Some(hit_id) = eng.hit_test_annotation(pdf_x, pdf_y) {
            eng.active_annotation_id = Some(hit_id); // 選択
        } else {
            eng.active_annotation_id = None; // 選択解除
        }
        area_click.queue_draw();
        area_click.grab_focus();
    });
    drawing_area.add_controller(click_ctrl);

    // 3. ドラッグ＆ドロップ (移動)
    let drag_ctrl = GestureDrag::new();
    let eng_drag = engine.clone();
    let ui_drag = ui_state.clone();
    let area_drag = drawing_area.clone();

    // ドラッグ開始時の元の座標を記憶する用
    let start_pos = Rc::new(RefCell::new((0.0, 0.0)));
    let start_pos_clone = start_pos.clone();

    drag_ctrl.connect_drag_begin(move |_, x, y| {
        let mut eng = eng_drag.borrow_mut();
        let scale = ui_drag.borrow().scale;
        let area_w = area_drag.width() as f64;
        
        let (pdf_x, pdf_y) = convert_to_pdf_coords(x, y, &eng, scale, area_w);

        // ドラッグ開始位置にアノテーションがあれば選択し、その初期座標を記憶
        if let Some(hit_id) = eng.hit_test_annotation(pdf_x, pdf_y) {
            eng.active_annotation_id = Some(hit_id.clone());
            if let Some(ann) = eng.annotations.iter().find(|a| a.id == hit_id) {
                *start_pos_clone.borrow_mut() = (ann.x, ann.y);
            }
        }
    });

    let eng_drag_update = engine.clone();
    let ui_drag_update = ui_state.clone();
    let area_drag_update = drawing_area.clone();
    let start_pos_update = start_pos.clone();

    drag_ctrl.connect_drag_update(move |_, offset_x, offset_y| {
        let mut eng = eng_drag_update.borrow_mut();
        let scale = ui_drag_update.borrow().scale;

        // UI上の移動量をPDF上の移動量にスケール変換
        let pdf_dx = offset_x / scale;
        let pdf_dy = offset_y / scale;

        if let Some(id) = eng.active_annotation_id.clone() {
            let (start_x, start_y) = *start_pos_update.borrow();
            eng.move_annotation(&id, start_x + pdf_dx, start_y + pdf_dy);
            area_drag_update.queue_draw();
        }
    });
    drawing_area.add_controller(drag_ctrl);

    // 4. スクロールコントローラーの作成
    // ロジック（ページ送り）はここには書かず、コントローラーだけ作って親(ui.rs)に渡す
    let scroll_ctrl = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    pdf_scroll_window.add_controller(scroll_ctrl.clone());

    (container, drawing_area, text_buffer)
}
