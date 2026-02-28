use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, Orientation, Separator
};


// 戻り値を「構造体」にして整理してもいいですが、
// ここではタプルで (Toolbarウィジェット, Prevボタン, Nextボタン, Openボタン, ZoomIn, ZoomOut, ページラベル) を返します
// 数が多いので、必要なものだけ返します。
pub struct ToolbarWidgets {
    pub container: GtkBox,
    pub btn_open: Button,
    pub btn_save: Button,
    pub btn_save_as: Button,
    pub btn_prev: Button,
    pub btn_next: Button,
    pub btn_zoom_in: Button,
    pub btn_zoom_out: Button,
    pub label_page: Label,
}

pub fn build(filename_label: &Label) -> ToolbarWidgets {
    let toolbar = GtkBox::new(Orientation::Horizontal, 10);
    toolbar.set_margin_top(8);
    toolbar.set_margin_bottom(8);
    toolbar.set_margin_start(10);
    toolbar.set_margin_end(10);

    // ファイル名
    toolbar.append(filename_label);

    // スペーサー
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    toolbar.append(&spacer);

    // --- ボタン作成 ---
    let btn_prev = Button::with_label("◀");
    let label_page = Label::new(Some(" - / - "));
    let btn_next = Button::with_label("▶");
    let btn_open = Button::with_label("📂 Open");
    let btn_save = Button::with_label("💾 Save");
    let btn_save_as = Button::with_label("💾 Save As");
    let btn_zoom_in = Button::with_label("🔍 Zoom In");
    let btn_zoom_out = Button::with_label("🔍 Zoom Out");

    // 配置
    toolbar.append(&btn_open);
    toolbar.append(&btn_save);
    toolbar.append(&btn_save_as);
    toolbar.append(&Separator::new(Orientation::Vertical));
    toolbar.append(&btn_prev);
    toolbar.append(&label_page);
    toolbar.append(&btn_next);
    toolbar.append(&Separator::new(Orientation::Vertical));
    toolbar.append(&btn_zoom_out);
    toolbar.append(&btn_zoom_in);

    ToolbarWidgets {
        container: toolbar,
        btn_open,
        btn_save,
        btn_save_as,
        btn_prev,
        btn_next,
        btn_zoom_in,
        btn_zoom_out,
        label_page,
    }
}
