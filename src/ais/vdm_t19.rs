/*
Copyright 2020 Timo Saarinen

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use super::*;

/// AIS VDM/VDO type 19: Extended Class B Equipment Position Report
pub(crate) fn handle(
    bv: &BitVec,
    station: Station,
    own_vessel: bool,
) -> Result<ParsedMessage, ParseError> {
    Ok(ParsedMessage::VesselDynamicData(VesselDynamicData {
        own_vessel: { own_vessel },
        station: { station },
        ais_type: { AisClass::ClassB },
        mmsi: { pick_u64(bv, 8, 30) as u32 },
        sog_knots: {
            let raw = pick_u64(bv, 46, 10);
            if raw < 1023 {
                Some((raw as f64) * 0.1)
            } else {
                None
            }
        },
        high_position_accuracy: pick_u64(bv, 56, 1) != 0,
        longitude: {
            let lon_raw = pick_i64(bv, 57, 28) as i32;
            if lon_raw != 0x6791AC0 {
                Some((lon_raw as f64) / 600000.0)
            } else {
                None
            }
        },
        latitude: {
            let lat_raw = pick_i64(bv, 85, 27) as i32;
            if lat_raw != 0x3412140 {
                Some((lat_raw as f64) / 600000.0)
            } else {
                None
            }
        },
        cog: {
            let cog_raw = pick_u64(bv, 112, 12);
            if cog_raw != 0xE10 {
                Some(cog_raw as f64 * 0.1)
            } else {
                None
            }
        },
        heading_true: {
            let th_raw = pick_u64(bv, 124, 9);
            if th_raw != 511 {
                Some(th_raw as f64)
            } else {
                None
            }
        },
        timestamp_seconds: pick_u64(bv, 133, 6) as u8, //same as code 18 until here
        class_b_unit_flag: { None },
        class_b_display: { None },
        class_b_dsc: { None },
        class_b_band_flag: { None },
        class_b_msg22_flag: { None },
        class_b_mode_flag: { None },
        raim_flag: pick_u64(bv, 305, 1) != 0,
        class_b_css_flag: { None },
        radio_status: { None },
        nav_status: NavigationStatus::NotDefined,
        rot: None,
        rot_direction: None,
        positioning_system_meta: None,
        current_gnss_position: None,
        special_manoeuvre: None,
    }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_vdm_type19() {
        let mut p = NmeaParser::new();
        match p.parse_sentence(
            "!AIVDM,1,1,,,C>l2oRh02mFenjw93gGjswp1kkaQkgQWc111111111jd0000002P,0*2F",
        ) {
            Ok(ps) => {
                match ps {
                    // The expected result
                    ParsedMessage::VesselDynamicData(vdd) => {
                        assert_eq!(vdd.mmsi, 994097035);
                        assert_eq!(vdd.nav_status, NavigationStatus::NotDefined);
                        assert_eq!(vdd.rot, None);
                        assert_eq!(vdd.rot_direction, None);
                        assert_eq!(vdd.sog_knots, Some(1.1));
                        assert!(!vdd.high_position_accuracy);
                        assert::close(vdd.latitude.unwrap_or(0.0), -6.0, 0.1);
                        assert::close(vdd.longitude.unwrap_or(0.0), -147.9, 0.1);
                        assert::close(vdd.cog.unwrap_or(0.0), 388.6, 0.1);
                        assert_eq!(vdd.heading_true, None);
                        assert_eq!(vdd.timestamp_seconds, 48);
                        assert_eq!(vdd.positioning_system_meta, None);
                        assert_eq!(vdd.special_manoeuvre, None);
                        assert!(!vdd.raim_flag);
                    }
                    ParsedMessage::Incomplete => {
                        assert!(false);
                    }
                    _ => {
                        assert!(false);
                    }
                }
            }
            Err(e) => {
                assert_eq!(e.to_string(), "OK");
            }
        }
    }
}
