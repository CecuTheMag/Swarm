//! Live images of path-tracer jobs (spec 0003).
//!
//! Each job's accumulator holds the radiance sum and sample count of every pixel, and the
//! pixels as they are shown. It is created on the first image request, filled from the
//! database, and fed by every accepted unit. Units are counted once by id, so a unit
//! accepted while the image is being filled is neither lost nor counted twice.
//!
//! The lock over all images is only held for short updates: a unit changes its own tile,
//! filling from the database and PNG encoding happen outside it, and the encoded image is
//! shared by every viewer until the next unit changes it.

use crate::templates::pt::unit_rgb;
use axum::body::Bytes;
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
    /// What the image shows: mean radiance, clamped and gamma-encoded; pixels without
    /// samples are transparent.
    rgba: Vec<u8>,
    counted: HashSet<Uuid>,
    viewed: Instant,
    /// Changes with every unit added.
    version: u64,
    /// The PNG of `version`, once someone asked for it.
    png: Option<(u64, Bytes)>,
}

impl Accumulator {
    fn new(p: &JobParams) -> Self {
        let pixels = p.width as usize * p.height as usize;
        Self {
            width: p.width,
            height: p.height,
            sum: vec![0.0; pixels * 3],
            samples: vec![0; pixels],
            rgba: vec![0; pixels * 4],
            counted: HashSet::new(),
            viewed: Instant::now(),
            version: 0,
            png: None,
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
                let n = self.samples[pixel] as f32;
                for c in 0..3 {
                    let mean = self.sum[pixel * 3 + c] / n;
                    self.rgba[pixel * 4 + c] =
                        (mean.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8;
                }
                self.rgba[pixel * 4 + 3] = 255;
            }
        }
        self.version += 1;
    }
}

/// Encodes RGBA pixels as a PNG.
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Bytes {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().expect("in-memory PNG header");
    writer.write_image_data(rgba).expect("in-memory PNG data");
    writer.finish().expect("in-memory PNG");
    out.into()
}

impl Renders {
    /// Adds an accepted unit to its job's image if the image is being viewed.
    pub async fn unit_accepted(&self, job_id: Uuid, unit_id: Uuid, params: &Value, output: &Value) {
        if let Some(acc) = self.jobs.lock().await.get_mut(&job_id) {
            acc.add(unit_id, params, output);
        }
    }

    /// Drops a job's image so the next request rebuilds it from the database: some of
    /// its units were reopened (spec 0004).
    pub async fn forget(&self, job_id: Uuid) {
        self.jobs.lock().await.remove(&job_id);
    }

    /// The job's current image, or `None` if there is no such path-tracer job.
    pub async fn png(&self, db: &PgPool, job_id: Uuid) -> sqlx::Result<Option<Bytes>> {
        // Copy the pixels under the lock, encode outside it. The image may be dropped
        // between loading and copying (reopened units, or too many images); load again.
        let mut copied = None;
        for _ in 0..3 {
            if !self.jobs.lock().await.contains_key(&job_id) && !self.load(db, job_id).await? {
                return Ok(None);
            }
            let mut jobs = self.jobs.lock().await;
            let Some(acc) = jobs.get_mut(&job_id) else {
                continue;
            };
            acc.viewed = Instant::now();
            if let Some((version, png)) = &acc.png
                && *version == acc.version
            {
                return Ok(Some(png.clone()));
            }
            copied = Some((acc.version, acc.width, acc.height, acc.rgba.clone()));
            break;
        }
        let Some((version, width, height, rgba)) = copied else {
            return Ok(None);
        };
        let png = tokio::task::spawn_blocking(move || encode_png(width, height, &rgba))
            .await
            .expect("PNG encoding does not panic");
        if let Some(acc) = self.jobs.lock().await.get_mut(&job_id)
            && acc.version == version
        {
            acc.png = Some((version, png.clone()));
        }
        Ok(Some(png))
    }

    /// Starts an image for the job and fills it from the database without holding the
    /// lock; units accepted meanwhile go straight in. False if there is no such
    /// path-tracer job.
    async fn load(&self, db: &PgPool, job_id: Uuid) -> sqlx::Result<bool> {
        let Some(params) = job_params(db, job_id).await? else {
            return Ok(false);
        };
        {
            let mut jobs = self.jobs.lock().await;
            if jobs.contains_key(&job_id) {
                // Another request got here first.
                return Ok(true);
            }
            if jobs.len() >= MAX_CACHED
                && let Some(oldest) = jobs.iter().min_by_key(|(_, a)| a.viewed).map(|(id, _)| *id)
            {
                jobs.remove(&oldest);
            }
            jobs.insert(job_id, Accumulator::new(&params));
        }
        let units: Vec<(Uuid, Json<Value>, Json<Value>)> = sqlx::query_as(
            "SELECT id, params, output FROM units WHERE job_id = $1 AND status = 'done'",
        )
        .bind(job_id)
        .fetch_all(db)
        .await?;
        if let Some(acc) = self.jobs.lock().await.get_mut(&job_id) {
            for (unit_id, Json(params), Json(output)) in units {
                acc.add(unit_id, &params, &output);
            }
        }
        Ok(true)
    }
}

async fn job_params(db: &PgPool, job_id: Uuid) -> sqlx::Result<Option<JobParams>> {
    let job: Option<(String, Json<Value>)> =
        sqlx::query_as("SELECT template, params FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(db)
            .await?;
    Ok(match job {
        Some((template, Json(params))) if template == ID => serde_json::from_value(params).ok(),
        _ => None,
    })
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

        let decoder = png::Decoder::new(std::io::Cursor::new(encode_png(
            acc.width, acc.height, &acc.rgba,
        )));
        let mut reader = decoder.read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut rgba).unwrap();
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]);
        // Mean (1.0, 0.25, 0.0) → white-clamped red, gamma-encoded green, black blue.
        assert_eq!(&rgba[8..12], &[255, 136, 0, 255]);
    }
}
