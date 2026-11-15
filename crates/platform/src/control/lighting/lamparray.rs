//! HID LampArray reports (HID Usage Tables, "Lighting And Illumination" page 0x59), encoded
//! and decoded from the layout the device declares. Portable: the macOS backend builds the
//! layout from the device's I/O Kit elements; the tests use the layout of Microsoft's
//! reference LampArray descriptor.
//!
//! A layout lists the fields of one feature report in report order (usage, bit size, count).
//! Values are packed little-endian bit by bit, as HID requires. A non-zero report id is the
//! first byte of the buffer (macOS `IOHIDDeviceSetReport` / `GetReport` expect it there).

/// Usage page "Lighting And Illumination".
pub const USAGE_PAGE: u32 = 0x59;

/// Usages of page 0x59 the app reads or writes.
pub mod usage {
    pub const LAMP_ARRAY: u32 = 0x01;
    pub const LAMP_COUNT: u32 = 0x03;
    pub const BOUNDING_BOX_WIDTH: u32 = 0x04;
    pub const BOUNDING_BOX_HEIGHT: u32 = 0x05;
    pub const BOUNDING_BOX_DEPTH: u32 = 0x06;
    pub const LAMP_ARRAY_KIND: u32 = 0x07;
    pub const MIN_UPDATE_INTERVAL: u32 = 0x08;
    pub const LAMP_ID: u32 = 0x21;
    pub const POSITION_X: u32 = 0x23;
    pub const POSITION_Y: u32 = 0x24;
    pub const POSITION_Z: u32 = 0x25;
    pub const RED_LEVEL_COUNT: u32 = 0x28;
    pub const GREEN_LEVEL_COUNT: u32 = 0x29;
    pub const BLUE_LEVEL_COUNT: u32 = 0x2A;
    pub const INTENSITY_LEVEL_COUNT: u32 = 0x2B;
    pub const IS_PROGRAMMABLE: u32 = 0x2C;
    pub const RED_UPDATE: u32 = 0x51;
    pub const GREEN_UPDATE: u32 = 0x52;
    pub const BLUE_UPDATE: u32 = 0x53;
    pub const INTENSITY_UPDATE: u32 = 0x54;
    pub const LAMP_UPDATE_FLAGS: u32 = 0x55;
    pub const LAMP_ID_START: u32 = 0x61;
    pub const LAMP_ID_END: u32 = 0x62;
    pub const AUTONOMOUS_MODE: u32 = 0x71;
}

/// `LampUpdateFlags` bit 0: the host finished this update, the device shows it.
pub const LAMP_UPDATE_COMPLETE: u32 = 0x01;

/// One main item of a report: `count` values of `bit_size` bits, all of `usage`
/// (0 for padding or usages of other pages).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub usage: u32,
    pub bit_size: u32,
    pub count: u32,
}

impl Field {
    pub const fn new(usage: u32, bit_size: u32, count: u32) -> Self {
        Self {
            usage,
            bit_size,
            count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportLayout {
    pub report_id: u8,
    fields: Vec<(Field, u32)>,
    bits: u32,
}

impl ReportLayout {
    /// Builds a layout from the fields in report order.
    pub fn new(report_id: u8, fields: &[Field]) -> Self {
        let mut offset = 0;
        let mut out = Vec::with_capacity(fields.len());
        for f in fields {
            out.push((*f, offset));
            offset += f.bit_size * f.count;
        }
        Self {
            report_id,
            fields: out,
            bits: offset,
        }
    }

    /// Buffer length in bytes, report id included.
    pub fn len(&self) -> usize {
        self.bits.div_ceil(8) as usize + usize::from(self.report_id != 0)
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    pub fn has(&self, usage: u32) -> bool {
        self.occurrences(usage) > 0
    }

    /// How many values of `usage` the report carries.
    pub fn occurrences(&self, usage: u32) -> u32 {
        self.fields
            .iter()
            .filter(|(f, _)| f.usage == usage)
            .map(|(f, _)| f.count)
            .sum()
    }

    /// Bit offset (after the report id byte) and size of the `n`th value of `usage`.
    fn slot(&self, usage: u32, n: u32) -> Option<(u32, u32)> {
        let mut left = n;
        for (f, offset) in &self.fields {
            if f.usage != usage {
                continue;
            }
            if left < f.count {
                return Some((offset + left * f.bit_size, f.bit_size));
            }
            left -= f.count;
        }
        None
    }

    /// A zeroed buffer with the report id in front.
    pub fn buffer(&self) -> Vec<u8> {
        let mut b = vec![0; self.len()];
        if self.report_id != 0 {
            b[0] = self.report_id;
        }
        b
    }

    fn base(&self) -> usize {
        usize::from(self.report_id != 0)
    }

    /// Writes `value` (truncated to the field size) into the `n`th value of `usage`.
    pub fn set(&self, buf: &mut [u8], usage: u32, n: u32, value: u32) -> bool {
        let Some((offset, size)) = self.slot(usage, n) else {
            return false;
        };
        let base = self.base();
        for bit in 0..size.min(32) {
            let pos = offset + bit;
            let byte = base + (pos / 8) as usize;
            let Some(slot) = buf.get_mut(byte) else {
                return false;
            };
            let mask = 1u8 << (pos % 8);
            if (value >> bit) & 1 == 1 {
                *slot |= mask;
            } else {
                *slot &= !mask;
            }
        }
        true
    }

    /// Reads the `n`th value of `usage` (unsigned).
    pub fn get(&self, buf: &[u8], usage: u32, n: u32) -> Option<u32> {
        let (offset, size) = self.slot(usage, n)?;
        let base = self.base();
        let mut value = 0u32;
        for bit in 0..size.min(32) {
            let pos = offset + bit;
            let byte = *buf.get(base + (pos / 8) as usize)?;
            if (byte >> (pos % 8)) & 1 == 1 {
                value |= 1 << bit;
            }
        }
        Some(value)
    }
}

/// The feature reports of one LampArray.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LampArrayReports {
    pub attributes: ReportLayout,
    pub attributes_request: ReportLayout,
    pub attributes_response: ReportLayout,
    pub multi_update: ReportLayout,
    pub range_update: Option<ReportLayout>,
    pub control: Option<ReportLayout>,
}

impl LampArrayReports {
    /// Recognises the reports by the usages they carry. `None` when a required one is
    /// missing (the device is then not treated as a LampArray).
    pub fn identify(layouts: Vec<ReportLayout>) -> Option<Self> {
        let mut attributes = None;
        let mut request = None;
        let mut response = None;
        let mut multi = None;
        let mut range = None;
        let mut control = None;
        for l in layouts {
            if l.has(usage::BOUNDING_BOX_WIDTH) {
                attributes = Some(l);
            } else if l.has(usage::POSITION_X) {
                response = Some(l);
            } else if l.has(usage::LAMP_ID_START) {
                range = Some(l);
            } else if l.has(usage::RED_UPDATE) && l.has(usage::LAMP_ID) {
                multi = Some(l);
            } else if l.has(usage::AUTONOMOUS_MODE) {
                control = Some(l);
            } else if l.has(usage::LAMP_ID) {
                request = Some(l);
            }
        }
        Some(Self {
            attributes: attributes?,
            attributes_request: request?,
            attributes_response: response?,
            multi_update: multi?,
            range_update: range,
            control,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayAttributes {
    pub lamp_count: u32,
    /// Micrometres.
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub kind: u32,
    pub min_update_interval_us: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LampAttributes {
    pub id: u32,
    /// Micrometres from the bounding box origin.
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub red_levels: u32,
    pub green_levels: u32,
    pub blue_levels: u32,
    pub intensity_levels: u32,
    pub programmable: bool,
}

impl LampAttributes {
    /// The lamp shows colors (more than one level on each of red, green and blue).
    pub fn color_capable(&self) -> bool {
        self.red_levels > 1 && self.green_levels > 1 && self.blue_levels > 1
    }
}

pub fn decode_attributes(r: &LampArrayReports, buf: &[u8]) -> Option<ArrayAttributes> {
    let l = &r.attributes;
    Some(ArrayAttributes {
        lamp_count: l.get(buf, usage::LAMP_COUNT, 0)?,
        width: l.get(buf, usage::BOUNDING_BOX_WIDTH, 0).unwrap_or(0),
        height: l.get(buf, usage::BOUNDING_BOX_HEIGHT, 0).unwrap_or(0),
        depth: l.get(buf, usage::BOUNDING_BOX_DEPTH, 0).unwrap_or(0),
        kind: l.get(buf, usage::LAMP_ARRAY_KIND, 0).unwrap_or(0),
        min_update_interval_us: l.get(buf, usage::MIN_UPDATE_INTERVAL, 0).unwrap_or(0),
    })
}

pub fn encode_attributes_request(r: &LampArrayReports, lamp_id: u32) -> Vec<u8> {
    let l = &r.attributes_request;
    let mut b = l.buffer();
    l.set(&mut b, usage::LAMP_ID, 0, lamp_id);
    b
}

pub fn decode_lamp(r: &LampArrayReports, buf: &[u8]) -> Option<LampAttributes> {
    let l = &r.attributes_response;
    let g = |u| l.get(buf, u, 0).unwrap_or(0);
    Some(LampAttributes {
        id: l.get(buf, usage::LAMP_ID, 0)?,
        x: g(usage::POSITION_X),
        y: g(usage::POSITION_Y),
        z: g(usage::POSITION_Z),
        red_levels: g(usage::RED_LEVEL_COUNT),
        green_levels: g(usage::GREEN_LEVEL_COUNT),
        blue_levels: g(usage::BLUE_LEVEL_COUNT),
        intensity_levels: g(usage::INTENSITY_LEVEL_COUNT),
        programmable: g(usage::IS_PROGRAMMABLE) != 0,
    })
}

/// `LampArrayControlReport`: `true` hands the lamps back to the device's own effect.
pub fn encode_control(r: &LampArrayReports, autonomous: bool) -> Option<Vec<u8>> {
    let l = r.control.as_ref()?;
    let mut b = l.buffer();
    l.set(&mut b, usage::AUTONOMOUS_MODE, 0, u32::from(autonomous));
    Some(b)
}

/// One lamp's update: channel values already scaled to the lamp's level counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LampValue {
    pub id: u32,
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub intensity: u32,
}

/// Scales an 8-bit channel to a lamp with `levels` levels (0..=levels-1).
pub fn scale_channel(value: u8, levels: u32) -> u32 {
    if levels <= 1 {
        return 0;
    }
    let max = levels - 1;
    (u32::from(value) * max + 127) / 255
}

/// `LampMultiUpdateReport`s for `lamps`, as many as the report capacity needs; only the
/// last one carries `LampUpdateComplete`.
pub fn encode_multi_update(r: &LampArrayReports, lamps: &[LampValue]) -> Vec<Vec<u8>> {
    let l = &r.multi_update;
    let capacity = l.occurrences(usage::LAMP_ID).max(1) as usize;
    let chunks: Vec<&[LampValue]> = lamps.chunks(capacity).collect();
    let last = chunks.len().saturating_sub(1);
    chunks
        .iter()
        .enumerate()
        .map(|(ci, chunk)| {
            let mut b = l.buffer();
            l.set(&mut b, usage::LAMP_COUNT, 0, chunk.len() as u32);
            if ci == last {
                l.set(&mut b, usage::LAMP_UPDATE_FLAGS, 0, LAMP_UPDATE_COMPLETE);
            }
            for (i, v) in chunk.iter().enumerate() {
                let i = i as u32;
                l.set(&mut b, usage::LAMP_ID, i, v.id);
                l.set(&mut b, usage::RED_UPDATE, i, v.red);
                l.set(&mut b, usage::GREEN_UPDATE, i, v.green);
                l.set(&mut b, usage::BLUE_UPDATE, i, v.blue);
                l.set(&mut b, usage::INTENSITY_UPDATE, i, v.intensity);
            }
            b
        })
        .collect()
}

/// `LampRangeUpdateReport`: one color for lamps `start..=end`.
pub fn encode_range_update(
    r: &LampArrayReports,
    start: u32,
    end: u32,
    v: LampValue,
) -> Option<Vec<u8>> {
    let l = r.range_update.as_ref()?;
    let mut b = l.buffer();
    l.set(&mut b, usage::LAMP_UPDATE_FLAGS, 0, LAMP_UPDATE_COMPLETE);
    l.set(&mut b, usage::LAMP_ID_START, 0, start);
    l.set(&mut b, usage::LAMP_ID_END, 0, end);
    l.set(&mut b, usage::RED_UPDATE, 0, v.red);
    l.set(&mut b, usage::GREEN_UPDATE, 0, v.green);
    l.set(&mut b, usage::BLUE_UPDATE, 0, v.blue);
    l.set(&mut b, usage::INTENSITY_UPDATE, 0, v.intensity);
    Some(b)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Report layout of Microsoft's reference LampArray descriptor (the "LampArray" sample of
    /// the HID Usage Tables 1.3 / Windows Dynamic Lighting docs): 8 lamps per multi update,
    /// colors interleaved R, G, B, I per lamp.
    pub(crate) fn reference_reports() -> LampArrayReports {
        use usage::*;
        let attributes = ReportLayout::new(
            1,
            &[
                Field::new(LAMP_COUNT, 16, 1),
                Field::new(BOUNDING_BOX_WIDTH, 32, 1),
                Field::new(BOUNDING_BOX_HEIGHT, 32, 1),
                Field::new(BOUNDING_BOX_DEPTH, 32, 1),
                Field::new(LAMP_ARRAY_KIND, 32, 1),
                Field::new(MIN_UPDATE_INTERVAL, 32, 1),
            ],
        );
        let request = ReportLayout::new(2, &[Field::new(LAMP_ID, 16, 1)]);
        let response = ReportLayout::new(
            3,
            &[
                Field::new(LAMP_ID, 16, 1),
                Field::new(POSITION_X, 32, 1),
                Field::new(POSITION_Y, 32, 1),
                Field::new(POSITION_Z, 32, 1),
                Field::new(0x27, 32, 1),
                Field::new(0x26, 32, 1),
                Field::new(RED_LEVEL_COUNT, 8, 1),
                Field::new(GREEN_LEVEL_COUNT, 8, 1),
                Field::new(BLUE_LEVEL_COUNT, 8, 1),
                Field::new(INTENSITY_LEVEL_COUNT, 8, 1),
                Field::new(IS_PROGRAMMABLE, 8, 1),
                Field::new(0x2D, 8, 1),
            ],
        );
        let mut multi = vec![
            Field::new(LAMP_COUNT, 8, 1),
            Field::new(LAMP_UPDATE_FLAGS, 8, 1),
            Field::new(LAMP_ID, 16, 8),
        ];
        for _ in 0..8 {
            multi.extend([
                Field::new(RED_UPDATE, 8, 1),
                Field::new(GREEN_UPDATE, 8, 1),
                Field::new(BLUE_UPDATE, 8, 1),
                Field::new(INTENSITY_UPDATE, 8, 1),
            ]);
        }
        let multi = ReportLayout::new(4, &multi);
        let range = ReportLayout::new(
            5,
            &[
                Field::new(LAMP_UPDATE_FLAGS, 8, 1),
                Field::new(LAMP_ID_START, 16, 1),
                Field::new(LAMP_ID_END, 16, 1),
                Field::new(RED_UPDATE, 8, 1),
                Field::new(GREEN_UPDATE, 8, 1),
                Field::new(BLUE_UPDATE, 8, 1),
                Field::new(INTENSITY_UPDATE, 8, 1),
            ],
        );
        let control = ReportLayout::new(6, &[Field::new(AUTONOMOUS_MODE, 8, 1)]);
        LampArrayReports::identify(vec![control, multi, range, response, request, attributes])
            .expect("reference layout identifies")
    }

    #[test]
    fn identifies_and_sizes_the_reference_reports() {
        let r = reference_reports();
        assert_eq!(r.attributes.report_id, 1);
        assert_eq!(r.attributes_request.report_id, 2);
        assert_eq!(r.attributes_response.report_id, 3);
        assert_eq!(r.multi_update.report_id, 4);
        assert_eq!(r.range_update.as_ref().map(|l| l.report_id), Some(5));
        assert_eq!(r.control.as_ref().map(|l| l.report_id), Some(6));
        assert_eq!(r.attributes.len(), 1 + 22);
        assert_eq!(r.multi_update.len(), 1 + 2 + 16 + 32);
        assert!(LampArrayReports::identify(vec![r.control.clone().expect("control")]).is_none());
    }

    #[test]
    fn bit_packing_is_little_endian_and_round_trips() {
        let l = ReportLayout::new(
            0,
            &[
                Field::new(1, 4, 1),
                Field::new(2, 12, 1),
                Field::new(3, 3, 2),
            ],
        );
        assert_eq!(l.len(), 3);
        let mut b = l.buffer();
        assert!(l.set(&mut b, 1, 0, 0xA));
        assert!(l.set(&mut b, 2, 0, 0xBCD));
        assert!(l.set(&mut b, 3, 1, 0b101));
        assert_eq!(b, vec![0xDA, 0xBC, 0b0010_1000]);
        assert_eq!(l.get(&b, 2, 0), Some(0xBCD));
        assert_eq!(l.get(&b, 3, 1), Some(0b101));
        assert_eq!(l.get(&b, 3, 0), Some(0));
        assert!(!l.set(&mut b, 3, 2, 1));
        assert_eq!(l.get(&b, 9, 0), None);
    }

    #[test]
    fn decodes_attributes_and_lamps() {
        let r = reference_reports();
        let mut b = r.attributes.buffer();
        let a = &r.attributes;
        a.set(&mut b, usage::LAMP_COUNT, 0, 104);
        a.set(&mut b, usage::BOUNDING_BOX_WIDTH, 0, 440_000);
        a.set(&mut b, usage::LAMP_ARRAY_KIND, 0, 1);
        a.set(&mut b, usage::MIN_UPDATE_INTERVAL, 0, 16_000);
        assert_eq!(b[0], 1);
        assert_eq!(&b[1..3], &[104, 0]);
        let attrs = decode_attributes(&r, &b).expect("decode");
        assert_eq!(attrs.lamp_count, 104);
        assert_eq!(attrs.width, 440_000);
        assert_eq!(attrs.kind, 1);
        assert_eq!(attrs.min_update_interval_us, 16_000);

        assert_eq!(encode_attributes_request(&r, 0x0102), vec![2, 0x02, 0x01]);

        let l = &r.attributes_response;
        let mut b = l.buffer();
        l.set(&mut b, usage::LAMP_ID, 0, 7);
        l.set(&mut b, usage::POSITION_X, 0, 123_456);
        for u in [
            usage::RED_LEVEL_COUNT,
            usage::GREEN_LEVEL_COUNT,
            usage::BLUE_LEVEL_COUNT,
        ] {
            l.set(&mut b, u, 0, 255);
        }
        l.set(&mut b, usage::INTENSITY_LEVEL_COUNT, 0, 1);
        l.set(&mut b, usage::IS_PROGRAMMABLE, 0, 1);
        let lamp = decode_lamp(&r, &b).expect("decode lamp");
        assert_eq!(lamp.id, 7);
        assert_eq!(lamp.x, 123_456);
        assert!(lamp.color_capable());
        assert!(lamp.programmable);
    }

    #[test]
    fn multi_update_splits_by_capacity_and_flags_only_the_last() {
        let r = reference_reports();
        let lamps: Vec<LampValue> = (0..10)
            .map(|i| LampValue {
                id: i,
                red: 10 + i,
                green: 20,
                blue: 30,
                intensity: 255,
            })
            .collect();
        let reports = encode_multi_update(&r, &lamps);
        assert_eq!(reports.len(), 2);
        let first = &reports[0];
        assert_eq!(first[0], 4);
        assert_eq!(first[1], 8, "lamp count");
        assert_eq!(first[2], 0, "not complete yet");
        assert_eq!(&first[3..5], &[0, 0], "lamp id 0");
        assert_eq!(&first[17..19], &[7, 0], "lamp id 7");
        // Colors start after count, flags and 8 ids: R G B I of lamp 0.
        assert_eq!(&first[19..23], &[10, 20, 30, 255]);
        let second = &reports[1];
        assert_eq!(second[1], 2);
        assert_eq!(second[2], LAMP_UPDATE_COMPLETE as u8);
        assert_eq!(&second[3..5], &[8, 0]);
        assert_eq!(&second[19..23], &[18, 20, 30, 255]);
        assert_eq!(&second[27..31], &[0, 0, 0, 0], "unused slot stays zero");
    }

    #[test]
    fn range_and_control_reports() {
        let r = reference_reports();
        let v = LampValue {
            id: 0,
            red: 1,
            green: 2,
            blue: 3,
            intensity: 4,
        };
        assert_eq!(
            encode_range_update(&r, 0, 0x0103, v),
            Some(vec![5, 1, 0, 0, 0x03, 0x01, 1, 2, 3, 4])
        );
        assert_eq!(encode_control(&r, false), Some(vec![6, 0]));
        assert_eq!(encode_control(&r, true), Some(vec![6, 1]));
    }

    #[test]
    fn channels_scale_to_level_counts() {
        assert_eq!(scale_channel(255, 256), 255);
        assert_eq!(scale_channel(128, 256), 128);
        assert_eq!(scale_channel(255, 16), 15);
        assert_eq!(scale_channel(0, 16), 0);
        assert_eq!(scale_channel(255, 1), 0);
    }
}
