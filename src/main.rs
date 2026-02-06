use gtk4::prelude::*;
use gtk4::Application;

// ファイルをモジュールとして登録
mod engine;
mod ui;
mod annotations;

fn main() {
    let app = Application::builder()
        .application_id("com.example.margium_separated")
        .build();

    // uiモジュールの中にある build 関数を呼ぶ
    app.connect_activate(ui::build);

    app.run();
}