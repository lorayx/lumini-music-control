//! Allocation-free, stereo-safe filter-bank analysis at 44.1 kHz.
//! 10 ms updates; 30 ms power integration rejects steady sub-bass ripple.
const HOP: u32 = 441;
const DT: f32 = 0.01;

#[derive(Default)]
struct AutoLevel { reference: f32, value: f32 }
impl AutoLevel {
    fn update(&mut self, rms: f32) -> f32 {
        // Follow loudness without pinning the visual envelope to its ceiling.
        let rate = if rms > self.reference { 0.08 } else { 0.0025 };
        self.reference += (rms - self.reference) * rate;
        let target = if rms < 0.00035 { 0.0 } else {
            1.0 - (-0.85 * rms / (self.reference * 0.8 + 0.0015)).exp()
        };
        let rate = if target > self.value { 0.35 } else { 0.045 };
        self.value += (target - self.value) * rate;
        self.value
    }
}

pub(super) struct KickDetector {
    filters: [[f32; 5]; 2], coefficients: [f32; 5],
    sum: [f32; 5], wide_sum: f32, powers: [[f32; 5]; 3],
    power_index: usize, count: u32, smoothed: [f32; 3], baseline: [f32; 3],
    levels: [AutoLevel; 4], pub pulse: f32, pub level: f32,
    pub sub: f32, pub mid: f32, pub treble: f32,
    samples: u64, last_hit: u64, pub hits: u64, blocked: bool,
}
impl Default for KickDetector {
    fn default() -> Self {
        Self { filters: [[0.0; 5]; 2],
            coefficients: [28.0, 80.0, 220.0, 2000.0, 10000.0]
                .map(|hz| 1.0 - (-std::f32::consts::TAU * hz / 44100.0).exp()),
            sum: [0.0; 5], wide_sum: 0.0, powers: [[0.0; 5]; 3],
            power_index: 0, count: 0, smoothed: [0.0; 3], baseline: [0.0; 3],
            levels: std::array::from_fn(|_| AutoLevel::default()), pulse: 0.0,
            level: 0.0, sub: 0.0, mid: 0.0, treble: 0.0,
            samples: 0, last_hit: 0, hits: 0, blocked: false }
    }
}
impl KickDetector {
    pub fn sample(&mut self, input: f32) { self.sample_stereo(input, input); }
    pub fn sample_stereo(&mut self, left: f32, right: f32) {
        for (channel, input) in [left, right].into_iter().enumerate() {
            let input = if input.is_finite() { input.clamp(-1.0, 1.0) } else { 0.0 };
            let f = &mut self.filters[channel];
            for (i, v) in f.iter_mut().enumerate() { *v += (input - *v) * self.coefficients[i]; }
            // Keep channel powers separate: opposite phases must not cancel.
            let bands = [f[2]-f[0], f[1]-f[0], f[2]-f[1], f[3]-f[2], f[4]-f[3]];
            for (i, value) in bands.into_iter().enumerate() { self.sum[i] += value * value * 0.5; }
            self.wide_sum += input * input * 0.5;
        }
        self.count += 1; self.samples += 1;
        if self.count < HOP { return; }
        let bass_share = self.sum[0] / self.wide_sum.max(1e-12);
        let wide_rms = (self.wide_sum / HOP as f32).sqrt();
        self.powers[self.power_index] = self.sum.map(|v| v / HOP as f32);
        self.power_index = (self.power_index + 1) % 3;
        let rms: [f32; 5] = std::array::from_fn(|i|
            ((self.powers[0][i]+self.powers[1][i]+self.powers[2][i])/3.0).sqrt());
        self.sum = [0.0; 5]; self.wide_sum = 0.0; self.count = 0;

        // Positive, locally normalised spectral changes in the low-frequency
        // filter bank distinguish attacks from an already loud bass bed.
        let mut flux = [0.0; 3];
        for i in 0..3 {
            let floor = self.baseline[i].max(0.0004);
            flux[i] = (rms[i]-self.smoothed[i]).max(0.0)
                / (self.smoothed[i]*0.22+floor*0.08+0.0003);
        }
        let novelty = flux[0].max(flux[1]*0.8).max(flux[2]*0.8);
        self.pulse *= (-DT/0.15).exp();
        if novelty < 0.25 { self.blocked = false; }
        if !self.blocked && novelty > 0.9 && rms[0] > 0.001 && bass_share > 0.06
            && (self.last_hit == 0 || self.samples.saturating_sub(self.last_hit) >= 3969) {
            // Soft knee preserves strength differences instead of clipping to 1.
            self.pulse = self.pulse.max(0.22+0.74*(1.0-(-novelty*0.28).exp()));
            self.last_hit = self.samples; self.hits += 1; self.blocked = true;
        }
        for i in 0..3 {
            self.smoothed[i] += (rms[i]-self.smoothed[i])*0.38;
            self.baseline[i] += (rms[i]-self.baseline[i])*0.016;
        }
        // Per-band gain control must not amplify filter leakage into a false
        // full-strength band. Preserve each band's share of the actual mix.
        let share = |value: f32| (value / (wide_rms * 0.28 + 0.0001)).min(1.0);
        self.level = self.levels[0].update(rms[0]) * share(rms[0]);
        self.sub = self.levels[1].update(rms[1]) * share(rms[1]);
        self.mid = self.levels[2].update(rms[3]) * share(rms[3]);
        self.treble = self.levels[3].update(rms[4]) * share(rms[4]);
    }
    pub fn idle(&mut self, seconds: f32) {
        let decay = (-seconds.max(0.0)/0.15).exp();
        self.pulse *= decay;
        // Treat missing packets as silence so stale normalisation does not stick.
        let steps = (seconds.max(0.0)/DT).ceil().min(100.0) as usize;
        for _ in 0..steps {
            self.level = self.levels[0].update(0.0); self.sub = self.levels[1].update(0.0);
            self.mid = self.levels[2].update(0.0); self.treble = self.levels[3].update(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(d: &mut KickDetector, hz: f32, amplitude: f32, seconds: f32) {
        for n in 0..(44100.0*seconds) as usize { d.sample((n as f32/44100.0*hz*std::f32::consts::TAU).sin()*amplitude); }
    }
    fn sequence(amplitude:f32,frequency:f32,interval:f32,bed:f32,opposite:bool)->KickDetector {
        let mut d=KickDetector::default();
        for n in 0..88200 { let t=n as f32/44100.0;let phase=t%interval;
            let env=if t>0.15&&phase<0.065{(-phase/0.022).exp()}else{0.0};
            let kick=(t*frequency*std::f32::consts::TAU).sin()*amplitude*env;
            let bass=(t*48.0*std::f32::consts::TAU).sin()*bed;let l=kick+bass;
            d.sample_stereo(l,if opposite{-l}else{l}); } d
    }
    #[test] fn silence_has_no_kicks(){let mut d=KickDetector::default();tone(&mut d,65.0,0.0,1.0);assert_eq!(d.hits,0);assert_eq!(d.level,0.0);}
    #[test] fn low_frequency_attacks_trigger_and_decay(){let mut d=KickDetector::default();tone(&mut d,65.0,0.7,0.15);tone(&mut d,65.0,0.0,1.0);assert!(d.hits>0);assert!(d.pulse<0.03);}
    #[test] fn sustained_bass_is_not_a_repeated_kick(){for hz in [28.0,35.0,48.0,65.0,110.0,180.0]{let mut d=KickDetector::default();tone(&mut d,hz,0.7,2.0);assert!(d.hits<=2,"hz={hz} hits={}",d.hits);}}
    #[test] fn high_frequency_only_does_not_trigger(){let mut d=KickDetector::default();tone(&mut d,6000.0,0.1,1.0);assert_eq!(d.hits,0);}
    #[test] fn quiet_short_kicks_are_detected(){let d=sequence(0.025,65.0,0.25,0.0,false);assert!((6..=8).contains(&d.hits),"hits={}",d.hits);}
    #[test] fn rapid_kicks_are_detected(){let d=sequence(0.4,100.0,0.125,0.0,false);assert!((13..=16).contains(&d.hits),"hits={}",d.hits);}
    #[test] fn kicks_over_sustained_bass_rearm(){let d=sequence(0.5,85.0,0.25,0.16,false);assert!((6..=9).contains(&d.hits),"hits={}",d.hits);}
    #[test] fn opposite_stereo_does_not_cancel_kicks(){let d=sequence(0.2,65.0,0.25,0.0,true);assert!((6..=8).contains(&d.hits),"hits={}",d.hits);}
    #[test] fn different_kick_pitches_are_detected(){for hz in [35.0,65.0,110.0,180.0]{let d=sequence(0.15,hz,0.25,0.0,false);assert!((6..=8).contains(&d.hits),"hz={hz} hits={}",d.hits);}}
    #[test] fn near_silence_does_not_trigger(){let d=sequence(0.0002,65.0,0.25,0.0,false);assert_eq!(d.hits,0);}
    #[test] fn sustained_loud_music_has_headroom(){let mut d=KickDetector::default();tone(&mut d,65.0,0.85,3.0);assert!((0.35..0.85).contains(&d.level),"{}",d.level);tone(&mut d,65.0,0.0,2.0);assert!(d.level<0.005);}
    #[test] fn levels_adapt_to_recording_gain(){let mut a=KickDetector::default();let mut b=KickDetector::default();tone(&mut a,65.0,0.04,4.0);tone(&mut b,65.0,0.8,4.0);assert!((a.level-b.level).abs()<0.16,"{} {}",a.level,b.level);}
    #[test] fn bands_distinguish_bass_and_treble(){let mut low=KickDetector::default();let mut high=KickDetector::default();tone(&mut low,55.0,0.4,2.0);tone(&mut high,6000.0,0.4,2.0);assert!(low.sub>high.sub+0.2,"{} {}",low.sub,high.sub);assert!(high.treble>low.treble+0.2,"{} {}",high.treble,low.treble);}
    #[test] fn idle_and_bad_samples_are_safe(){let mut d=sequence(0.4,65.0,0.25,0.0,false);d.idle(2.0);assert!(d.pulse<0.001);for _ in 0..4410{d.sample_stereo(f32::NAN,f32::INFINITY);}assert!(d.level.is_finite()&&d.mid.is_finite()&&d.treble.is_finite());}
    #[test] fn attack_is_detected_without_waiting_for_a_visual_frame(){for hz in [35.0,65.0,110.0,180.0]{let mut d=KickDetector::default();tone(&mut d,hz,0.0,0.2);let start=d.samples;let mut delay=None;for n in 0..2205{let t=n as f32/44100.0;d.sample((t*hz*std::f32::consts::TAU).sin()*0.3*(-t/0.03).exp());if d.hits>0{delay=Some(d.samples-start);break;}}assert!(delay.is_some_and(|n|n<=1764),"hz={hz} delay={delay:?}");}}
}
