use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::error::{CoreError, Result};
use crate::image_prep::normalize_dynamic_image;
use crate::models::PageImage;

#[derive(Debug, Clone)]
pub struct InkBrush {
    pub color: [u8; 4],
    pub width: f32,
}

impl Default for InkBrush {
    fn default() -> Self {
        Self {
            color: [15, 23, 42, 255], // crisp dark ink
            width: 2.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InkStroke {
    pub points: Vec<(f32, f32)>,
    pub brush: InkBrush,
}

/// Parses an InkML XML document (including OneNote's `inkml:trace` and `brush` definitions).
pub fn parse_inkml_strokes(xml: &str) -> Result<Vec<InkStroke>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut brushes: HashMap<String, InkBrush> = HashMap::new();
    let mut current_brush_id: Option<String> = None;
    let mut current_brush = InkBrush::default();
    let mut in_brush = false;
    let mut in_brush_prop: Option<String> = None;

    let mut in_trace = false;
    let mut trace_brush_ref: Option<String> = None;
    let mut trace_text = String::new();
    let mut strokes = Vec::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "brush" => {
                        in_brush = true;
                        current_brush = InkBrush::default();
                        current_brush_id = attr_value(e, "id")
                            .or_else(|| attr_value(e, "xml:id"));
                    }
                    "brushProperty" if in_brush => {
                        let prop_name = attr_value(e, "name");
                        let prop_val = attr_value(e, "value");
                        if let (Some(pn), Some(pv)) = (prop_name.as_deref(), prop_val.as_deref()) {
                            apply_brush_prop(&mut current_brush, pn, pv);
                        }
                        in_brush_prop = prop_name;
                    }
                    "trace" => {
                        in_trace = true;
                        trace_text.clear();
                        trace_brush_ref = attr_value(e, "brushRef");
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = local_name(e.name().as_ref());
                if name == "brushProperty" && in_brush {
                    if let (Some(pn), Some(pv)) = (attr_value(e, "name"), attr_value(e, "value")) {
                        apply_brush_prop(&mut current_brush, &pn, &pv);
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                let txt = e.as_ref();
                if in_trace {
                    trace_text.push_str(txt);
                } else if in_brush {
                    if let Some(prop) = &in_brush_prop {
                        apply_brush_prop(&mut current_brush, prop, txt.trim());
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "brush" => {
                        if let Some(id) = current_brush_id.take() {
                            brushes.insert(format!("#{id}"), current_brush.clone());
                            brushes.insert(id, current_brush.clone());
                        }
                        in_brush = false;
                    }
                    "brushProperty" => {
                        in_brush_prop = None;
                    }
                    "trace" => {
                        let points = parse_trace_points(&trace_text);
                        if !points.is_empty() {
                            let brush = trace_brush_ref
                                .as_deref()
                                .and_then(|r| brushes.get(r).or_else(|| brushes.get(r.trim_start_matches('#'))))
                                .cloned()
                                .unwrap_or_default();
                            strokes.push(InkStroke { points, brush });
                        }
                        in_trace = false;
                        trace_brush_ref = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(CoreError::InkMl(format!("XML parse error: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(strokes)
}

fn local_name(raw: &str) -> String {
    raw.rsplit(':').next().unwrap_or(raw).to_string()
}

fn attr_value(e: &quick_xml::events::BytesStart<'_>, target: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        let key = attr.key.as_ref();
        if key == target || key.rsplit(':').next() == Some(target) {
            return Some(attr.value.into_owned());
        }
    }
    None
}

fn apply_brush_prop(brush: &mut InkBrush, name: &str, value: &str) {
    match name {
        "color" => {
            if let Some(rgba) = parse_hex_color(value) {
                brush.color = rgba;
            }
        }
        "width" => {
            if let Ok(w) = value.parse::<f32>() {
                // OneNote InkML widths are often in himetric (~0.01mm); clamp sensibly
                brush.width = (w / 40.0).clamp(1.5, 10.0);
            }
        }
        _ => {}
    }
}

fn parse_hex_color(val: &str) -> Option<[u8; 4]> {
    let hex = val.trim().trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([r, g, b, 255])
    } else if hex.len() == 8 {
        let a = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some([r, g, b, a])
    } else {
        None
    }
}

fn parse_trace_points(raw: &str) -> Vec<(f32, f32)> {
    raw.split(',')
        .filter_map(|sample| {
            let mut nums = sample.split_whitespace();
            let x = nums.next()?.parse::<f32>().ok()?;
            let y = nums.next()?.parse::<f32>().ok()?;
            Some((x, y))
        })
        .collect()
}

/// Rasterizes InkML strokes onto a high-contrast white canvas PNG (`PageImage`).
pub fn render_inkml_to_page_image(
    xml: &str,
    page_index: usize,
    label: String,
) -> Result<PageImage> {
    let strokes = parse_inkml_strokes(xml)?;
    if strokes.is_empty() {
        return Err(CoreError::InkMl(
            "No vector ink strokes found in InkML payload.".to_string(),
        ));
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for s in &strokes {
        for &(x, y) in &s.points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    let span_w = (max_x - min_x).max(1.0);
    let span_h = (max_y - min_y).max(1.0);
    let target_max_dim = 1800.0f32;
    let padding = 48.0f32;
    let scale = ((target_max_dim - padding * 2.0) / span_w.max(span_h)).min(4.0);

    let canvas_w = ((span_w * scale + padding * 2.0).ceil() as u32).clamp(256, 2048);
    let canvas_h = ((span_h * scale + padding * 2.0).ceil() as u32).clamp(256, 2048);

    let mut pixmap = Pixmap::new(canvas_w, canvas_h)
        .ok_or_else(|| CoreError::InkMl("Failed to allocate InkML raster pixmap.".to_string()))?;
    pixmap.fill(Color::WHITE);

    for s in &strokes {
        if s.points.is_empty() {
            continue;
        }
        let mut pb = PathBuilder::new();
        let (x0, y0) = s.points[0];
        let sx0 = (x0 - min_x) * scale + padding;
        let sy0 = (y0 - min_y) * scale + padding;
        pb.move_to(sx0, sy0);

        if s.points.len() == 1 {
            pb.line_to(sx0 + 0.5, sy0 + 0.5);
        } else {
            for &(x, y) in &s.points[1..] {
                pb.line_to((x - min_x) * scale + padding, (y - min_y) * scale + padding);
            }
        }

        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(
                s.brush.color[0],
                s.brush.color[1],
                s.brush.color[2],
                s.brush.color[3],
            );
            paint.anti_alias = true;

            let stroke = Stroke {
                width: s.brush.width.max(2.0),
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    let png_bytes = pixmap
        .encode_png()
        .map_err(|e| CoreError::InkMl(format!("Failed to encode rendered InkML PNG: {e}")))?;

    let img = image::load_from_memory(&png_bytes)?;
    normalize_dynamic_image(img, page_index, label, 2048)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_inkml_traces() {
        let sample = r##"<?xml version="1.0" encoding="utf-8"?>
        <inkml:ink xmlns:inkml="http://www.w3.org/2003/InkML">
          <inkml:definitions>
            <inkml:brush xml:id="br1">
              <inkml:brushProperty name="color" value="#1D4ED8"/>
              <inkml:brushProperty name="width" value="120"/>
            </inkml:brush>
          </inkml:definitions>
          <inkml:trace brushRef="#br1">100 100, 200 150, 300 100</inkml:trace>
          <inkml:trace>150 200, 250 250</inkml:trace>
        </inkml:ink>"##;

        let strokes = parse_inkml_strokes(sample).unwrap();
        assert_eq!(strokes.len(), 2);
        assert_eq!(strokes[0].brush.color, [29, 78, 216, 255]);

        let page = render_inkml_to_page_image(sample, 0, "OneNote Ink".into()).unwrap();
        assert!(page.width >= 256);
        assert!(page.height >= 256);
        assert!(!page.png_bytes.is_empty());
    }
}
