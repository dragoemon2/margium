use poppler::{Document};
use std::path::PathBuf;
use cairo::Context;
use crate::annotations::{AnnotationData};

use mathjax_svg::convert_to_svg;
use gtk4::glib::Bytes;
use gtk4::gio::{MemoryInputStream, Cancellable};
use rsvg::{Loader, CairoRenderer};

pub struct PdfEngine {
    doc: Option<Document>,
    filename: String,
    current_page: i32,
    total_pages: i32,
    filepath: Option<PathBuf>,

    pub annotations: Vec<AnnotationData>,
}

impl PdfEngine {
    pub fn new() -> Self {
        Self {
            doc: None,
            filename: String::new(),
            current_page: 0,
            total_pages: 0,
            filepath: None,
            annotations: Vec::new(),
        }
    }

    pub fn jump_to_page(&mut self, page_index: i32) -> bool {
        if page_index >= 0 && page_index < self.total_pages {
            self.current_page = page_index;
            return true;
        }
        false
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), String> {
        let uri = format!("file://{}", path.to_str().unwrap_or(""));
        
        match Document::from_file(&uri, None) {
            Ok(doc) => {
                self.total_pages = doc.n_pages();
                self.filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.current_page = 0;
                self.doc = Some(doc);
                self.filepath = Some(path);
                Ok(())
            }
            Err(e) => Err(format!("PDF Error: {}", e)),
        }
    }

    pub fn set_annotations(&mut self, annots: Vec<AnnotationData>) {
        self.annotations = annots;
    }

    pub fn next_page(&mut self) -> bool {
        if self.current_page < self.total_pages - 1 {
            self.current_page += 1;
            return true;
        }
        false
    }

    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            return true;
        }
        false
    }

    pub fn status_text(&self) -> String {
        if self.total_pages > 0 {
            format!("{}", self.filename)
        } else {
            " ".to_string()
        }
    }

    pub fn page_info(&self) -> String {
        if self.total_pages > 0 {
            format!("{} / {}", self.current_page + 1, self.total_pages)
        } else {
            " - ".to_string()
        }
    }

    pub fn get_page_size(&self) -> Option<(f64, f64)> {
        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(self.current_page) {
                return Some(page.size());
            }
        }
        None
    }

    pub fn get_total_pages(&self) -> i32 {
        self.total_pages
    }

    pub fn get_current_text(&self) -> Option<String> {
        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(self.current_page) {
                return page.text().map(|s| s.to_string());
            }
        }
        None
    }


    pub fn add_annotation(&mut self, text: &str, x: f64, y: f64) -> Result<(), String> {
        // 1. リストに追加
        self.annotations.push(AnnotationData {
            page: (self.current_page + 1) as u32, // lopdfは1-based
            x,
            y,
            content: text.to_string(),
            font_size: Some(14.0),
            id: uuid::Uuid::nil().to_string(),
            object_id: None,
        });

        Ok(())
    }

    pub fn draw(&self, context: &Context, area_width: f64, area_height: f64, scale: f64) {
        // ---------------------- PDF描画処理 ----------------------

        // 1. 背景をダークグレーで塗りつぶす
        context.set_source_rgb(0.2, 0.2, 0.2);
        context.paint().expect("Painting failed");

        if let Some(doc) = &self.doc {
            if let Some(page) = doc.page(self.current_page) {
                let (pdf_w, pdf_h) = page.size();
                
                // 2. 描画後のサイズを計算
                let draw_w = pdf_w * scale;
                let draw_h = pdf_h * scale;

                // 3. 中央寄せのためのオフセット計算 (X軸)
                // 画面幅の方が広い場合のみ中央に寄せる。画面の方が狭いなら左端(0)から。
                let offset_x = if area_width > draw_w {
                    (area_width - draw_w) / 2.0
                } else {
                    0.0
                };
                
                // 上部は少し余白(20px)を空ける
                let offset_y = 20.0;

                // 4. 座標系を変換 (移動 -> 拡大)
                context.save().unwrap(); // 状態保存
                
                context.translate(offset_x, offset_y);

                // PDFの影を描画 (オプション: ちょっと立体的に見える)
                context.set_source_rgba(0.0, 0.0, 0.0, 0.5);
                context.rectangle(5.0, 5.0, draw_w, draw_h);
                context.fill().unwrap();

                // 用紙の白背景を描画
                context.set_source_rgb(1.0, 1.0, 1.0);
                context.rectangle(0.0, 0.0, draw_w, draw_h);
                context.fill().unwrap();

                // 拡大適用
                context.scale(scale, scale);
                
                // PDFの中身を描画
                page.render(context);

                // アノテーションを描画
                self.draw_custom_annotations(context, scale);
                
                context.restore().unwrap(); // 状態復帰
            }
        } else {
            // PDFがない時のメッセージ（中央寄せ）
            context.set_source_rgb(0.7, 0.7, 0.7);
            context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            context.set_font_size(24.0);
            
            let text = "Drag & Drop or Click 'Open'";
            let extents = context.text_extents(text).unwrap();
            
            let x = (area_width - extents.width()) / 2.0;
            let y = area_height / 2.0;
            
            context.move_to(x, y);
            context.show_text(text).unwrap();
        }

    }

    fn draw_custom_annotations(&self, context: &Context, scale: f64) {
        let current_page_u32 = (self.current_page + 1) as u32;
        
        // 現在のページのアノテーションのみ抽出
        for ann in self.annotations.iter().filter(|a| a.page == current_page_u32) {
            
            context.save().unwrap();
            
            // 座標変換: アノテーションの位置へ移動 (scaleも考慮)
            context.translate(ann.x * scale, ann.y * scale);

            // 数式かどうかの判定 ($$で囲まれているか)
            if ann.content.starts_with("$$") && ann.content.ends_with("$$") {
                let latex = &ann.content[2..ann.content.len()-2];
                
                // MathJaxによるSVG変換
                if let Ok(svg_string) = convert_to_svg(latex) {
                    let loader = Loader::new();
                    let stream = MemoryInputStream::from_bytes(&Bytes::from(svg_string.as_bytes()));
                    
                    if let Ok(handle) = loader.read_stream(
                        &stream, 
                        None::<&gtk4::gio::File>, // ← ここを修正
                        None::<&Cancellable>
                    ) {
                        let renderer = CairoRenderer::new(&handle);
                        
                        let rect = renderer.intrinsic_dimensions();
                        let w = rect.width.length;
                        let h = rect.height.length;
                        
                        // フォントサイズに合わせてスケール調整 (目標高さ: 30px程度)
                        let target_h = 30.0; // 本来は ann.font_size から計算しても良い
                        let s = if h > 0.0 { target_h / h } else { 1.0 };
                        
                        context.scale(s, s);
                        
                        // 背景 (白の透過) - 数式を見やすくする
                        context.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                        context.rectangle(0.0, 0.0, w, h);
                        context.fill().unwrap();

                        // 数式描画
                        let _ = renderer.render_document(
                                context,
                                &cairo::Rectangle::new(0.0, 0.0, w, h) // ← newメソッドを使う
                            );
                    }
                } else {
                    // SVG変換エラー時のフォールバック
                    context.set_source_rgb(1.0, 0.0, 0.0);
                    context.show_text("Math Error").unwrap();
                }
            } else {
                // 通常のテキスト描画 (Pangoを使用するのがベストですが、簡易的にCairoで)
                context.set_source_rgba(1.0, 1.0, 0.8, 0.8); // 付箋っぽい背景
                context.rectangle(0.0, 0.0, 100.0, 20.0);    // 仮のサイズ
                context.fill().unwrap();

                context.set_source_rgb(0.0, 0.0, 0.0);
                context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                context.set_font_size(14.0);
                context.move_to(5.0, 15.0);
                context.show_text(&ann.content).unwrap();
            }
            context.restore().unwrap();
        }
    }
}