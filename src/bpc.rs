use chrono::{Datelike, Timelike, Utc, FixedOffset, DateTime};
use rodio::Source;
use std::f32::consts::PI;

#[derive(Clone, Debug)]
pub struct BPC { }

type ZonedDateTime = DateTime<FixedOffset>;
impl BPC {
    pub fn new() -> Self {
        Self {}
    }

    // signal_width in ms
    pub fn signal_width(&self, t: ZonedDateTime) -> Option<u32> {
        match self.code(t) {
            Some(0b00) => { Some(100) }
            Some(0b01) => { Some(200) }
            Some(0b10) => { Some(300) }
            Some(0b11) => { Some(400) }
            None => { None }
            _ => { unreachable!() }
        }
    }

    pub fn pulse(&self) -> bool {
        let now = Self::cst();
        let millis = now.timestamp_subsec_millis();
        let sw = self.signal_width(now);
        sw.map_or(true, |v| { millis > v })
    }

    fn cst() -> ZonedDateTime {
        // china standard time
        Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
    }

    fn code(&self, now: ZonedDateTime) -> Option<u8> {
        let year = now.year() - 2000;
        let month = now.month();
        let day = now.day();
        let weekday = now.weekday().number_from_monday();
        let (pm, hour) = now.hour12();
        let minute = now.minute();
        let second = now.second();

        let fragment = second % 20;
        match fragment {
            0 => { // empty 
                None
            }
            1 => {
                // seconds, 01 / 21 / 41
                let v: u32 = match second {
                    1 => {
                        0
                    }
                    21 => {
                        1
                    }
                    41 => {
                        2
                    }
                    _ => { unreachable!("unreachable seconds") }
                };
                Some(v as u8)
            }
            2 => {
                // reserved
                Some(0)
            }
            3 => {
                // hour high
                let high = hour >> 2;
                Some(high as u8)
            }
            4 => {
                // hour low
                let low = hour & 0b11;
                Some(low as u8)
            }
            5 => {
                // minute high
                let high = minute >> 4;
                Some(high as u8)
            }
            6 => {
                // minute middle
                let middle = (minute >> 2) & 0b11;
                Some(middle as u8)
            }
            7 => {
                // minute low
                let low = minute & 0b11;
                Some(low as u8)
            }
            8 => {
                // weekday high
                let high = weekday >> 2;
                Some(high as u8)
            }
            9 => {
                // weekday low
                let low = weekday & 0b11;
                Some(low as u8)
            }
            10 => {
                // check & am/pm
                let mut v: u8 = if pm { 0b10 } else { 0b00 };
                // second range: 0 - 59
                let s = match second {
                    1..=20 => { 0b00 }
                    21..=40 => { 0b01 }
                    41..=59 => { 0b11 }
                    _ => { unreachable!() }
                };
                let c = vec![s, hour, minute, weekday].into_iter().reduce(|acc, e| acc + e.count_ones()).unwrap();
                v |= if c % 2 == 0 { 0b0 } else { 0b1 };
                Some(v)
            }
            11 => {
                // day high
                let high = day >> 4;
                Some(high as u8)
            }
            12 => {
                // day middle
                let middle = (day >> 2) & 0b11;
                Some(middle as u8)
            }
            13 => {
                // day low
                let low = day & 0b11;
                Some(low as u8)
            }
            14 => {
                // month high
                let high = month >> 2;
                Some(high as u8)
            }
            15 => {
                // month low
                let low = month & 0b11;
                Some(low as u8)
            }
            16 => {
                // year high
                let high = (year >> 4) & 0b11;
                Some(high as u8)
            }
            17 => {
                // year middle
                let middle = (year >> 2) & 0b11;
                Some(middle as u8)
            }
            18 => {
                // year low
                let low = year & 0b11;
                Some(low as u8)
            }
            19 => {
                // check & year highest bit
                let year_highest = (year >> 6) & 0b1;
                let mut v = year_highest << 1;
                let c = vec![day, month, (year & 0b111111) as u32].into_iter().reduce(|acc, e| acc + e.count_ones()).unwrap();
                v |= if c % 2 == 0 { 0b0 } else { 0b1 };
                Some(v as u8)
            }
            _ => { unreachable!("unreachable fragment") }
        }
    }
}

const BPC_FREQ: u32 = 68500;
const SAMPLE_RATE: u32 = 48000;

#[derive(Clone, Debug)]
pub struct BPCWave {
    bpc: BPC,
    num_samples: usize,
}

impl BPCWave {
    pub fn new() -> Self {
        Self {
            bpc: BPC::new(),
            num_samples: 0
        }
    }
}

impl Iterator for BPCWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let v = if self.bpc.pulse() { 1. } else { 0. };

        let fc = BPC_FREQ / 5; // normally speakers only produce sound frequency under 20khz

        self.num_samples = self.num_samples.wrapping_add(1);
        let value = 2.0 * PI * fc as f32 * self.num_samples as f32 / SAMPLE_RATE as f32;
        Some(value.sin() * v)
    }
}

impl Source for BPCWave {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> u16 {
        1
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    #[inline]
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn hour_test() {
        let hour: u32 = 9;
        let high = hour >> 2;
        let low = hour & 0b11;
        assert_eq!(high as u8, 0b10);
        assert_eq!(low as u8, 0b01);
    }

    #[test]
    fn minute_test() {
        let minute: u32 = 15;
        let high = minute >> 4;
        let middle = (minute >> 2) & 0b11;
        let low = minute & 0b11;
        assert_eq!(high as u8, 0b00);
        assert_eq!(middle as u8, 0b11);
        assert_eq!(low as u8, 0b11);
    }
}