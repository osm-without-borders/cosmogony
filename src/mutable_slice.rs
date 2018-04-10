use zone::{Zone, ZoneIndex};

// This struct is necessary to wrap the `zones` slice
// and keep a mutable reference to a zone (and set
// its parent) while still be able to borrow another
// reference to another zone.
pub struct MutableSlice<'a> {
    pub right: &'a [Zone],
    pub left: &'a [Zone],
    pub idx: usize,
}

impl<'a> MutableSlice<'a> {
    pub fn init(zones: &'a mut [Zone], index: usize) -> (Self, &'a mut Zone) {
        let (left, temp) = zones.split_at_mut(index);
        let (z, right) = temp.split_at_mut(1);
        let s = Self {
            right: right,
            left: left,
            idx: index,
        };
        (s, &mut z[0])
    }

    pub fn get(&self, zindex: &ZoneIndex) -> &Zone {
        let idx = zindex.index;
        if idx < self.idx {
            return &self.left[idx];
        } else if idx == self.idx {
            panic!("Cannot retrieve middle index");
        } else {
            return &self.right[idx - self.idx - 1];
        }
    }
}
