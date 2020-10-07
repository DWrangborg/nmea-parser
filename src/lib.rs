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

#![allow(dead_code)]

#[macro_use] extern crate log;
extern crate env_logger;


extern crate chrono;

mod ais_vdm_t1t2t3;
mod ais_vdm_t5;
mod ais_vdm_t18;
mod ais_vdm_t19;
mod ais_vdm_t24;
mod gnss_gga;
mod gnss_gsa;
mod gnss_gsv;
mod gnss_rmc;
mod gnss_vtg;
mod gnss_gll;
mod types;
mod util;

pub use types::*;
use util::*;

use std::collections::{HashMap};
use bitvec::prelude::*;
use chrono::{DateTime};
use chrono::prelude::*;

/// Decode NMEA sentence into ParsedSentence string. In case of multi-fragment sentences up to
/// two two fragments are supported. Notice that in case of class B AIVDM VesselStaticData 
/// results you have to merge them with a possible existing VesselStaticData of the same same MMSI.
/// See `VesselStaticData::merge` for more information.
pub fn decode_sentence(sentence: &str, nmea_store: &mut NmeaStore) -> Result<ParsedSentence, String> {
    // https://gpsd.gitlab.io/gpsd/AIVDM.html#_aivdmaivdo_sentence_layer
    // http://www.allaboutais.com/jdownloads/AIS%20standards%20documentation/itu-m.1371-4-201004.pdf

    // Calculace NMEA checksum and compare it to the given one. Also, remove the checksum part
    // from the sentence to simplify next processing steps.
    let mut checksum = 0;
    let (sentence, checksum_hex_given) = { 
        if let Some(pos) = sentence.rfind('*') {
            (sentence[0..pos].to_string(), sentence[(pos+1)..sentence.len()].to_string())
        } else {
            debug!("No checksum found for sentence: {}", sentence);
            (sentence.to_string(), "".to_string())
        }
    };
    for c in sentence.as_str().chars().skip(1) {
        checksum = checksum ^ (c as u8);
    }
    let checksum_hex_calculated = format!("{:02X?}", checksum);
    if checksum_hex_calculated != checksum_hex_given && checksum_hex_given != "" {
        return Err(format!("Corrupted NMEA sentence: {:02X?} != {:02X?}", 
                           checksum_hex_calculated, checksum_hex_given));
    }
    
    // Pick sentence type
    let mut sentence_type: String = {
        if let Some(i) = sentence.find(',') {
            sentence[0..i].into()
        } else {
            return Err(format!("Invalid NMEA sentence: {}", sentence));
        }
    };

    // Recognize GNSS system by talker ID.
    let nav_system = {
        if &sentence_type[0..1] == "$" {
            match &sentence_type[1..3] {
                "GN" => Some(NavigationSystem::Combination),
                "GP" => Some(NavigationSystem::Gps),
                "GL" => Some(NavigationSystem::Glonass),
                "GA" => Some(NavigationSystem::Galileo),
                "BD" => Some(NavigationSystem::Beidou),
                "GI" => Some(NavigationSystem::Navic),
                "QZ" => Some(NavigationSystem::Qzss),
                _ => Some(NavigationSystem::Other),
            }
        } else {
            None
        }
    };
    if nav_system != None {
        // Shorten the GNSS setence types to three letters
        if sentence_type.len() <= 6 {
            sentence_type = format!("${}", &sentence_type[3..6]);
        }
    }

    // Recognize AIS station
    let station = {
        if &sentence_type[0..1] == "!" {
            match &sentence_type[1..3] {
                "AB" => Some(Station::BaseAisStation),
                "AD" => Some(Station::DependentAisBaseStation),
                "AI" => Some(Station::MobileAisStation),
                "AN" => Some(Station::AidToNavigationAisStation),
                "AR" => Some(Station::AisReceivingStation),
                "AS" => Some(Station::LimitedBaseStation),
                "AT" => Some(Station::AisTransmittingStation),
                "AX" => Some(Station::RepeaterAisStation),
                _ => Some(Station::Other),
            }
        } else {
            None
        }
    };
    if station != None {
        // Shorten the AIS setence types to three letters
        if sentence_type.len() <= 6 {
            sentence_type = format!("!{}", &sentence_type[3..6]);
        }
    }

    // Handle sentence types
    match sentence_type.as_str() {
        // $xxGGA - Global Positioning System Fix Data
        "$GGA" => {
            return gnss_gga::handle(sentence.as_str(), nav_system.unwrap_or(NavigationSystem::Other));
        },
        // $xxRMC - Recommended minimum specific GPS/Transit data
        "$RMC" => {
            return gnss_rmc::handle(sentence.as_str(), nav_system.unwrap_or(NavigationSystem::Other));
        },
        // $xxGSA - GPS DOP and active satellites 
        "$GSA" => {
            return gnss_gsa::handle(sentence.as_str(), nav_system.unwrap_or(NavigationSystem::Other));
        },
        // $xxGSV - GPS Satellites in view
        "$GSV" => {
            return gnss_gsv::handle(sentence.as_str(), nav_system.unwrap_or(NavigationSystem::Other), 
                                    nmea_store);
        },
        // $xxVTG - Track made good and ground speed
        "$VTG" => {
            return gnss_vtg::handle(sentence.as_str(), nav_system.unwrap_or(NavigationSystem::Other), 
                                    nmea_store);
        },
        // $xxGLL - Geographic position, latitude / longitude
        "$GLL" => {
            return gnss_gll::handle(sentence.as_str(), nav_system.unwrap_or(NavigationSystem::Other), 
                                    nmea_store);
        },


        // $xxALM - Almanac Data
        "$ALM" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxHDT - Heading, True
        "$HDT" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxTRF - Transit Fix Data
        "$TRF" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxSTN - Multiple Data ID
        "$STN" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxVBW - Dual Ground / Water Speed
        "$VBW" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxXTC - Cross track error
        "$XTC" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxXTE - Cross-track error, Measured
        "$XTE" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxZDA - Date & Time
        "$ZDA" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },



        // $xxBOD Bearing Origin to Destination 
        "$BOD" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },
        // $xxRMA - Recommended minimum specific Loran-C data
        "$RMA" => {
            return Err(format!("Unimplemented NMEA sentence: {}", sentence_type)); // TODO
        },


        // Received AIS data from other or own vessel
        "!VDM" | "!VDO" => {
            let own_vessel = sentence_type.as_str() == "!VDO";
            let mut num = 0;
            let mut fragment_count = 0;
            let mut fragment_number = 0;
            let mut message_id = None;
            let mut radio_channel_code = None;
            let mut payload_string: String = "".into();
            for s in sentence.split(",") {
                match num {
                    1 => {
                        match s.parse::<u8>() {
                            Ok(i) => { fragment_count = i; },
                            Err(_) => { return Err(format!("Failed to parse fragment count: {}", s)); }
                        };
                    },
                    2 => {
                        match s.parse::<u8>() {
                            Ok(i) => { fragment_number = i; },
                            Err(_) => { return Err(format!("Failed to parse fragment count: {}", s)); }
                        };
                    },
                    3 => {
                        message_id = s.parse::<u64>().ok();
                    },
                    4 => {
                        // Radio channel code
                        radio_channel_code = Some(s);
                    },
                    5 => {
                        payload_string = s.to_string();
                    },
                    6 => {
                        // fill bits
                    },
                    _ => {
                    }
                }
                num += 1;
            }

            // Try parse the payload
            let mut bv: Option<BitVec> = None;
            if fragment_count == 1 {
                bv = parse_payload(&payload_string).ok();
            } else if fragment_count == 2 {
                if let Some(msg_id) = message_id {
                    let key1 = make_fragment_key(&sentence_type.to_string(), msg_id, fragment_count, 
                                                 1, radio_channel_code.unwrap_or(""));
                    let key2 = make_fragment_key(&sentence_type.to_string(), msg_id, fragment_count, 
                                                 2, radio_channel_code.unwrap_or(""));
                    if fragment_number == 1 {
                        if let Some(p) = nmea_store.pull_string(key2.into()) {
                            let mut payload_string_combined = payload_string;
                            payload_string_combined.push_str(p.as_str());
                            bv = parse_payload(&payload_string_combined). ok();
                        } else {
                            nmea_store.push_string(key1.into(), payload_string);
                        }
                    } else if fragment_number == 2 {
                        if let Some(p) = nmea_store.pull_string(key1.into()) {
                            let mut payload_string_combined = p.clone();
                            payload_string_combined.push_str(payload_string.as_str());
                            bv = parse_payload(&payload_string_combined).ok();
                        } else {
                            nmea_store.push_string(key2.into(), payload_string);
                        }
                    } else {
                        warn!("Unexpected NMEA fragment number: {}/{}", fragment_number, fragment_count);
                    }
                } else {
                    warn!("NMEA message_id missing from {} than supported 2", sentence_type);
                }
            } else {
                warn!("NMEA sentence fragment count greater ({}) than supported 2", fragment_count);
            }

            if let Some(bv) = bv {
                // https://www.trimble.com/OEM_ReceiverHelp/V4.44/en/NMEA-0183messages_MessageOverview.html
                // http://aprs.gids.nl/nmea/
                // https://gpsd.gitlab.io/gpsd/AIVDM.html#_type_5_static_and_voyage_related_data
                let message_type = pick_u64(&bv, 0, 6);
                match message_type {
                    // Position Report with SOTDMA/ITDMA
                    1 | 2 | 3 => {
                        return ais_vdm_t1t2t3::handle(&bv, station.unwrap_or(Station::Other), 
                                                      own_vessel);
                    },
                    // Base Station Report
                    4 => {
                        // TODO: implementation
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Ship static voyage related data
                    5 => {
                        return ais_vdm_t5::handle(&bv, station.unwrap_or(Station::Other), 
                                                  own_vessel);
                    },
                    // Addressed Binary Message 
                    6 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Binary Acknowledge
                    7 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Binary Broadcast Message 
                    8 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Standard SAR Aircraft position report 
                    9 => {
                        // TODO: implementation
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // UTC and Date inquiry 
                    10 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // UTC and Date response 
                    11 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Addressed safety related message 
                    12 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Safety related Acknowledge 
                    13 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Safety related Broadcast Message 
                    14 => {
                        // TODO: implementation (Class B)
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Interrogation 
                    15 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Assigned Mode Command 
                    16 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // GNSS Binary Broadcast Message  
                    17 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Standard Class B CS Position Report 
                    18 => {
                        return ais_vdm_t18::handle(&bv, station.unwrap_or(Station::Other), 
                                                   own_vessel);
                    },
                    // Extended Class B Equipment Position Report
                    19 => {
                        return ais_vdm_t19::handle(&bv, station.unwrap_or(Station::Other), 
                                                   own_vessel);
                    },
                    // Data Link Management 
                    20 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Aids-to-navigation Report 
                    21 => {
                        // TODO: implementation
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Channel Management 
                    22 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Group Assignment Command 
                    23 => {
                        return Err(format!("Unsupported {} message type: {}", 
                                            sentence_type, message_type));
                    },
                    // Class B CS Static Data Report
                    24 => {
                        return ais_vdm_t24::handle(&bv, station.unwrap_or(Station::Other), 
                                                   nmea_store, own_vessel);
                    },
                    _ => {
                        return Err(format!("Unrecognized {} message type: {}", 
                                            sentence_type, message_type));
                    }
                }
            } else {
                Ok(ParsedSentence::Incomplete)
            }
        },
        _ => {
            return Err(format!("Unsupported sentence: {}", sentence_type));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_corrupted() {
        // Try a sentence with mismatching checksum
        assert!(decode_sentence("!AIVDM,1,1,,A,38Id705000rRVJhE7cl9n;160000,0*41", 
                                &mut NmeaStore::new()).ok().is_none());
    }

    #[test]
    fn test_parse_missing_checksum() {
        // Try a sentence without checksum
        assert!(decode_sentence("!AIVDM,1,1,,A,38Id705000rRVJhE7cl9n;160000,0", 
                                &mut NmeaStore::new()).ok().is_some());
    }
}