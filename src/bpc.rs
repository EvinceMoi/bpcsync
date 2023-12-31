use chrono::{DateTime, Datelike, FixedOffset, Timelike, Utc};
use rodio::Source;
use std::{
    f32::consts::PI,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

type ZonedDateTime = DateTime<FixedOffset>;
pub fn cst() -> ZonedDateTime {
    // china standard time
    Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
}

pub struct BPC {}

impl BPC {
    pub fn new() -> Self {
        Self {}
    }

    // signal_width in ms
    pub fn signal_width(&self, t: ZonedDateTime) -> Option<u32> {
        match self.code(t) {
            Some(0b00) => Some(100),
            Some(0b01) => Some(200),
            Some(0b10) => Some(300),
            Some(0b11) => Some(400),
            None => None,
            _ => {
                unreachable!()
            }
        }
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
            0 => {
                // empty
                None
            }
            1 => {
                // seconds, 01 / 21 / 41
                let v: u32 = match second {
                    1 => 0,
                    21 => 1,
                    41 => 2,
                    _ => {
                        unreachable!("unreachable seconds")
                    }
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
                    1..=20 => 0b00,
                    21..=40 => 0b01,
                    41..=59 => 0b11,
                    _ => {
                        unreachable!()
                    }
                };
                let c = vec![s, hour, minute, weekday]
                    .into_iter()
                    .reduce(|acc, e| acc + e.count_ones())
                    .unwrap();
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
                let c = vec![day, month, (year & 0b111111) as u32]
                    .into_iter()
                    .reduce(|acc, e| acc + e.count_ones())
                    .unwrap();
                v |= if c % 2 == 0 { 0b0 } else { 0b1 };
                Some(v as u8)
            }
            _ => {
                unreachable!("unreachable fragment")
            }
        }
    }
}

const BPC_FREQ: u32 = 68500;
const SAMPLE_RATE: u32 = 44100; //48000;

struct BPCWaveInner {
    bpc: BPC,
    num_samples: usize,
    pivot: usize,
    updating: Arc<AtomicBool>,
}

impl BPCWaveInner {
    pub fn new() -> Self {
        Self {
            bpc: BPC::new(),
            num_samples: 0,
            pivot: 0,
            updating: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn update(&mut self, t: ZonedDateTime) {
        self.updating.store(true, Ordering::SeqCst);
        self.pivot = (self.bpc.signal_width(t).unwrap_or(0) * SAMPLE_RATE / 1000) as usize;
        self.num_samples = 0;
        self.updating.store(false, Ordering::SeqCst);
    }
}

impl Iterator for BPCWaveInner {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.updating.load(Ordering::SeqCst) {
            return Some(1.);
        }

        self.num_samples += 1;

        let fc = BPC_FREQ / 5; // normally speakers only produce sound frequency under 20khz
        let value = 2.0 * PI * fc as f32 * self.num_samples as f32 / SAMPLE_RATE as f32;
        if self.num_samples >= self.pivot {
            Some(value.sin())
        } else {
            Some(0.)
        }
    }
}

pub struct BPCWave {
    inner: Arc<Mutex<BPCWaveInner>>,
}

impl BPCWave {
    pub fn new() -> Self {
        let inner = Arc::new(Mutex::new(BPCWaveInner::new()));
        thread::spawn({
            let inner = inner.clone();
            move || loop {
                {
                    let now = cst();
                    let delta = 1_000_000 - now.timestamp_subsec_micros();
                    thread::sleep(Duration::from_micros(delta as u64));
                }

                let now = cst();
                inner.lock().unwrap().update(now);
            }
        });
        Self { inner }
    }
}

impl Iterator for BPCWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.inner.lock().unwrap().next()
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
