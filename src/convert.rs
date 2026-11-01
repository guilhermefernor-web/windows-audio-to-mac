//! Interleaved N-channel f32 at any rate -> interleaved stereo f32 at 48 kHz.

pub struct ToStereo48k {
    in_ch: usize,
    ratio: f64, // input frames consumed per output frame
    pos: f64,   // fractional position inside `prev`..`cur`
    prev: [f32; 2],
    cur: [f32; 2],
}

impl ToStereo48k {
    pub fn new(in_rate: u32, in_ch: usize) -> Self {
        Self {
            in_ch: in_ch.max(1),
            ratio: in_rate as f64 / crate::proto::RATE as f64,
            pos: 0.0,
            prev: [0.0; 2],
            cur: [0.0; 2],
        }
    }

    fn frame(&self, f: &[f32]) -> [f32; 2] {
        match self.in_ch {
            1 => [f[0], f[0]],
            2 => [f[0], f[1]],
            n => {
                // Simple downmix: fronts + half of everything else folded to the matching side.
                let (mut l, mut r) = (f[0], f[1]);
                if n >= 3 {
                    let c = f[2] * 0.707;
                    l += c;
                    r += c;
                }
                for (i, s) in f.iter().enumerate().skip(4) {
                    if i % 2 == 0 { l += s * 0.5 } else { r += s * 0.5 }
                }
                [l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0)]
            }
        }
    }

    /// Appends converted audio to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for chunk in input.chunks_exact(self.in_ch) {
            let next = self.frame(chunk);
            self.prev = self.cur;
            self.cur = next;
            // Emit all output samples whose position falls before this input frame.
            while self.pos < 1.0 {
                let t = self.pos as f32;
                out.push(self.prev[0] + (self.cur[0] - self.prev[0]) * t);
                out.push(self.prev[1] + (self.cur[1] - self.prev[1]) * t);
                self.pos += self.ratio;
            }
            self.pos -= 1.0;
        }
    }
}
