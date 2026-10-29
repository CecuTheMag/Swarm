//! Live images of path-tracer jobs (spec 0003).
//!
//! Each job's accumulator holds the radiance sum and sample count of every pixel. It is
//! built from the database on the first image request, then fed by every accepted unit.
//! Building and feeding share one lock, and units are counted once by id, so a unit
//! accepted while the image is being built is neither lost nor counted twice.

use crate::templates::pt::unit_rgb;
use protocol::templates::pt::{ID, JobParams, UnitParams};
use serde_json::Value;
use sqlx::{PgPool, types::Json};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Jobs whose images stay in memory; the least recently viewed is dropped first.
const MAX_CACHED: usize = 8;

#[derive(Default)]
pub struct Renders {
    jobs: Mutex<HashMap<Uuid, Accumulator>>,
}

struct Accumulator {
    width: u32,
    height: u32,
    sum: Vec<f32>,
    samples: Vec<u32>,
    counted: HashSet<Uuid>,
    viewed: Instant,
}

impl Accumulator {
    fn new(p: &JobParams) -> Self {
        let pixels = p.width as usize * p.height as usize;
        Self {
            width: p.width,
            height: p.height,
            sum: vec![0.0; pixels * 3],
            samples: vec![0; pixels],
            counted: HashSet::new(),
            viewed: Instant::now(),
        }
    }

    fn add(&mut self, unit_id: Uuid, params: &Value, output: &Value) {
        if self.counted.contains(&unit_id) {
            return;
        }
        let Ok(u) = serde_json::from_value::<UnitParams>(params.clone()) else {
            return;
        };
        let Ok(rgb) = unit_rgb(&u, output) else {
            return;
        };
        self.counted.insert(unit_id);
        for row in 0..u.h {
            for col in 0..u.w {
                let pixel = ((u.y + row) * self.width + u.x + col) as usize;
                let src = ((row * u.w + col) * 3) as usize;
                for c in 0..3 {
                    self.sum[pixel * 3 + c] += rgb[src + c];
                }
                self.samples[pixel] += u.spp;
            }
        }
    }

    /// RGBA PNG: mean radiance, clamped and gamma-encoded; pixels without samples
    /// are transparent.
    fn png(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.samples.len() * 4);
        for (pixel, &n) in self.samples.iter().enumerate() {
            if n == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            for c in 0..3 {
                let mean = self.sum[pixel * 3 + c] / n as f32;
                rgba.push((mean.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8);
            }
            rgba.push(255);
        }
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().expect("in-memory PNG header");
        writer.write_image_data(&rgba).expect("in-memory PNG data");
        writer.finish().expect("in-memory PNG");
        out
    }
}

impl Renders {
    /// Adds an accepted unit to its job's image if the image is being viewed.
    pub async fn unit_accepted(&self, job_id: Uuid, unit_id: Uuid, params: &Value, output: &Value) {
        if let Some(acc) = self.jobs.lock().await.get_mut(&job_id) {
            acc.add(unit_id, params, output);
        }
    }

    /// The job's current image, or `None` if there is no such path-tracer job.
    pub async fn png(&self, db: &PgPool, job_id: Uuid) -> sqlx::Result<Option<Vec<u8>>> {
        let mut jobs = self.jobs.lock().await;
        if !jobs.contains_key(&job_id) {
            let Some(acc) = build(db, job_id).await? else {
                return Ok(None);
            };
            if jobs.len() >= MAX_CACHED
                && let Some(oldest) = jobs.iter().min_by_key(|(_, a)| a.viewed).map(|(id, _)| *id)
            {
                jobs.remove(&oldest);
            }
            jobs.insert(job_id, acc);
        }
        let acc = jobs.get_mut(&job_id).expect("inserted above");
        acc.viewed = Instant::now();
        Ok(Some(acc.png()))
    }
}

async fn build(db: &PgPool, job_id: Uuid) -> sqlx::Result<Option<Accumulator>> {
    let job: Option<(String, Json<Value>)> =
        sqlx::query_as("SELECT template, params FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(db)
            .await?;
    let Some((template, Json(params))) = job else {
        return Ok(None);
    };
    if template != ID {
        return Ok(None);
    }
    let Ok(p) = serde_json::from_value::<JobParams>(params) else {
        return Ok(None);
    };
    let mut acc = Accumulator::new(&p);
    let units: Vec<(Uuid, Json<Value>, Json<Value>)> = sqlx::query_as(
        "SELECT id, params, output FROM units WHERE job_id = $1 AND status = 'done'",
    )
    .bind(job_id)
    .fetch_all(db)
    .await?;
    for (unit_id, Json(params), Json(output)) in units {
        acc.add(unit_id, &params, &output);
    }
    Ok(Some(acc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::templates::pt::encode_rgb;
    use serde_json::json;

    #[test]
    fn units_add_once_and_unsampled_pixels_are_transparent() {
        let mut acc = Accumulator::new(&JobParams {
            width: 4,
            height: 2,
            tile: 2,
            spp: 2,
            passes: 1,
            seed: 0,
        });
        let unit = json!({"width": 4, "height": 2, "x": 2, "y": 0, "w": 2, "h": 1, "spp": 2, "pass": 0, "seed": 0});
        let output = json!({ "rgb": encode_rgb(&[2.0, 0.5, 0.0, 0.0, 0.0, 0.0]) });
        let id = Uuid::now_v7();
        acc.add(id, &unit, &output);
        acc.add(id, &unit, &output);
        assert_eq!(acc.samples, [0, 0, 2, 2, 0, 0, 0, 0]);
        assert_eq!(&acc.sum[6..9], &[2.0, 0.5, 0.0]);

        let decoder = png::Decoder::new(std::io::Cursor::new(acc.png()));
        let mut reader = decoder.read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut rgba).unwrap();
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]);
        // Mean (1.0, 0.25, 0.0) → white-clamped red, gamma-encoded green, black blue.
        assert_eq!(&rgba[8..12], &[255, 136, 0, 255]);
    }
}
