extern crate geo;

use gst::rtree::Rect;
use ordered_float::OrderedFloat;
use geo::Bbox;

pub fn bbox_to_rect(bbox: Bbox<f64>) -> Rect {
    // rust-geo bbox algorithm returns `Bbox`,
    // while gst RTree uses `Rect` as index.
    Rect {
        xmin: OrderedFloat(down(bbox.xmin as f32)),
        xmax: OrderedFloat(up(bbox.xmax as f32)),
        ymin: OrderedFloat(down(bbox.ymin as f32)),
        ymax: OrderedFloat(up(bbox.ymax as f32)),
    }
}

// the goal is that f in [down(f as f32) as f64, up(f as f32) as f64]
fn down(f: f32) -> f32 {
    f - (f * ::std::f32::EPSILON).abs()
}
fn up(f: f32) -> f32 {
    f + (f * ::std::f32::EPSILON).abs()
}