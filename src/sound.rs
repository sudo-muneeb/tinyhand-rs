use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use std::sync::{Arc, Condvar, Mutex};
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
    fn snd_pcm_drop(p: *mut c_void) -> c_int;
    fn snd_pcm_prepare(p: *mut c_void) -> c_int;
    fn snd_pcm_close(p: *mut c_void) -> c_int;
}

type Pending = Arc<(Mutex<Option<(f64, usize)>>, Condvar)>;
pub struct Sound {
    pending: Pending,
}
impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}
impl Sound {
    pub fn new() -> Self {
        let pending = Arc::new((Mutex::new(None::<(f64, usize)>), Condvar::new()));
        let worker = pending.clone();
        thread::spawn(move || {
            let samples = WAVS.iter().map(|wav| decode_wav(wav)).collect::<Vec<_>>();
            let mut pcm: *mut c_void = std::ptr::null_mut();
            let mut last_use = std::time::Instant::now();
            loop {
                let (lock, wake) = &*worker;
                let mut guard = lock.lock().unwrap();
                while guard.is_none() {
                    if Arc::strong_count(&worker) == 1 {
                        if !pcm.is_null() {
                            unsafe {
                                snd_pcm_close(pcm);
                            }
                        }
                        return;
                    }
                    let (next, timeout) = wake
                        .wait_timeout(guard, std::time::Duration::from_secs(2))
                        .unwrap();
                    guard = next;
                    if timeout.timed_out()
                        && !pcm.is_null()
                        && last_use.elapsed() >= std::time::Duration::from_secs(30)
                    {
                        unsafe {
                            snd_pcm_close(pcm);
                        }
                        pcm = std::ptr::null_mut();
                    }
                }
                let mut tap = guard.take().unwrap();
                drop(guard);
                'latest: loop {
                    let (volume, kind) = tap;
                    last_use = std::time::Instant::now();
                    let Some(sample) = samples.get(kind.saturating_sub(1)) else {
                        break;
                    };
                    unsafe {
                        if pcm.is_null() {
                            if snd_pcm_open(&mut pcm, c"default".as_ptr(), 0, 0) < 0 {
                                pcm = std::ptr::null_mut();
                                break;
                            }
                            if snd_pcm_set_params(pcm, 2, 3, 1, 44100, 1, 20_000) < 0 {
                                snd_pcm_close(pcm);
                                pcm = std::ptr::null_mut();
                                break;
                            }
                        }
                        snd_pcm_drop(pcm);
                        snd_pcm_prepare(pcm);
                        if kind == 0 {
                            break;
                        }
                        let scaled = sample
                            .iter()
                            .map(|s| (*s as f64 * volume.clamp(0.0, 1.0)) as i16)
                            .collect::<Vec<_>>();
                        let mut offset = 0;
                        while offset < scaled.len() {
                            if let Some(next) = lock.lock().unwrap().take() {
                                snd_pcm_drop(pcm);
                                snd_pcm_prepare(pcm);
                                tap = next;
                                continue 'latest;
                            }
                            let end = (offset + 256).min(scaled.len());
                            let n = snd_pcm_writei(
                                pcm,
                                scaled[offset..end].as_ptr().cast(),
                                (end - offset) as c_ulong,
                            );
                            if n <= 0 {
                                snd_pcm_prepare(pcm);
                                break;
                            }
                            offset += n as usize;
                        }
                    }
                    break;
                }
            }
        });
        Self { pending }
    }
    pub fn warm(&self) {
        let (lock, wake) = &*self.pending;
        let mut pending = lock.lock().unwrap();
        if pending.is_none() {
            *pending = Some((0.0, 0));
            wake.notify_one();
        }
    }
    pub fn tap(&self, volume: f64, kind: usize) {
        if (1..=6).contains(&kind) {
            let (lock, wake) = &*self.pending;
            *lock.lock().unwrap() = Some((volume, kind));
            wake.notify_one();
        }
    }
}
impl Drop for Sound {
    fn drop(&mut self) {
        self.pending.1.notify_one();
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
