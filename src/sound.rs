use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use std::sync::mpsc::{self, SyncSender};
use std::thread;

#[link(name = "asound")]
extern "C" {
    fn snd_pcm_open(p: *mut *mut c_void, name: *const c_char, stream: c_int, mode: c_int) -> c_int;
    fn snd_pcm_set_params(
        p: *mut c_void,
        format: c_int,
        access: c_int,
        channels: c_uint,
        rate: c_uint,
        soft: c_int,
        latency: c_uint,
    ) -> c_int;
    fn snd_pcm_writei(p: *mut c_void, data: *const c_void, frames: c_ulong) -> isize;
    fn snd_pcm_prepare(p: *mut c_void) -> c_int;
    fn snd_pcm_close(p: *mut c_void) -> c_int;
}

pub struct Sound {
    tx: SyncSender<(f64, usize)>,
}
impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}
impl Sound {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<(f64, usize)>(4);
        thread::spawn(move || {
            let samples = WAVS.iter().map(|wav| decode_wav(wav)).collect::<Vec<_>>();
            let mut pcm: *mut c_void = std::ptr::null_mut();
            for (volume, kind) in rx {
                let Some(samples) = samples.get(kind.saturating_sub(1)) else {
                    continue;
                };
                unsafe {
                    if pcm.is_null() {
                        if snd_pcm_open(&mut pcm, c"default".as_ptr(), 0, 0) < 0 {
                            pcm = std::ptr::null_mut();
                            continue;
                        }
                        if snd_pcm_set_params(pcm, 2, 3, 1, 44100, 1, 35_000) < 0 {
                            snd_pcm_close(pcm);
                            pcm = std::ptr::null_mut();
                            continue;
                        }
                    }
                    let scaled = samples
                        .iter()
                        .map(|s| {
                            (*s as f64 * volume).clamp(i16::MIN as f64, i16::MAX as f64) as i16
                        })
                        .collect::<Vec<_>>();
                    let mut offset = 0;
                    while offset < scaled.len() {
                        let n = snd_pcm_writei(
                            pcm,
                            scaled[offset..].as_ptr().cast(),
                            (scaled.len() - offset) as c_ulong,
                        );
                        if n < 0 {
                            snd_pcm_prepare(pcm);
                            break;
                        }
                        if n == 0 {
                            break;
                        }
                        offset += n as usize;
                    }
                }
            }
            if !pcm.is_null() {
                unsafe {
                    snd_pcm_close(pcm);
                }
            }
        });
        Self { tx }
    }
    pub fn tap(&self, volume: f64, kind: usize) {
        if kind > 0 {
            let _ = self.tx.try_send((volume, kind));
        }
    }
}

const WAVS: [&[u8]; 6] = [
    include_bytes!("../assets/sounds/tap_thock.wav"),
    include_bytes!("../assets/sounds/tap_click.wav"),
    include_bytes!("../assets/sounds/tap_pop.wav"),
    include_bytes!("../assets/sounds/tap_wood.wav"),
    include_bytes!("../assets/sounds/tap_boop.wav"),
    include_bytes!("../assets/sounds/tap_squeak.wav"),
];

fn decode_wav(wav: &[u8]) -> Vec<i16> {
    let mut offset = 12;
    while offset + 8 <= wav.len() {
        let length = u32::from_le_bytes(wav[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let Some(end) = start.checked_add(length).filter(|end| *end <= wav.len()) else {
            break;
        };
        if &wav[offset..offset + 4] == b"data" {
            return wav[start..end]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b))
                .collect();
        }
        offset = end + length % 2;
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_sound_choices_have_pcm_audio() {
        for wav in WAVS {
            let pcm = decode_wav(wav);
            assert!(!pcm.is_empty());
            assert!(pcm.iter().any(|s| *s != 0));
        }
    }
}
