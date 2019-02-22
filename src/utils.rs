use ordered_float::OrderedFloat;

pub fn bbox_to_rect(bbox: &geo_types::Rect<f64>) -> gst::rtree::Rect {
    // rust-geo bbox algorithm returns `Bbox`,
    // while gst RTree uses `Rect` as index.
    gst::rtree::Rect {
        xmin: OrderedFloat(down(bbox.min.x as f32)),
        xmax: OrderedFloat(up(bbox.max.x as f32)),
        ymin: OrderedFloat(down(bbox.min.y as f32)),
        ymax: OrderedFloat(up(bbox.max.y as f32)),
    }
}

// the goal is that f in [down(f as f32) as f64, up(f as f32) as f64]
fn down(f: f32) -> f32 {
    f - (f * ::std::f32::EPSILON).abs()
}
fn up(f: f32) -> f32 {
    f + (f * ::std::f32::EPSILON).abs()
}
