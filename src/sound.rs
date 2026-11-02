use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use std::sync::mpsc::{self, Sender};
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
    tx: Sender<f64>,
}
impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}
impl Sound {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<f64>();
        thread::spawn(move || {
            // The original sample is PCM, mono, 16-bit, 44100 Hz.
            let wav = include_bytes!("../assets/sounds/tap_thock.wav");
            let samples = wav
                .chunks_exact(2)
                .skip(22)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>();
            let mut pcm: *mut c_void = std::ptr::null_mut();
            for volume in rx {
                unsafe {
                    if pcm.is_null() {
                        if snd_pcm_open(&mut pcm, b"default\0".as_ptr().cast(), 0, 0) < 0 {
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
    pub fn tap(&self, volume: f64) {
        let _ = self.tx.send(volume);
    }
}
