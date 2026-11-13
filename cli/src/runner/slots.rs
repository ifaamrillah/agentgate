//! Batas job paralel runner (`runner_prefs.max_parallel`, "Job paralel maksimal"). Job siap di
//! atas batas menunggu di antrean lokal (urutan datang) dan dimulai saat ada slot kosong.
//! `None` = tanpa batas (spec: tidak ada batas default).

use std::collections::VecDeque;

use agentgate_core::runner::protocol::JobSummary;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct Slots {
    max: Option<usize>,
    running: Vec<Uuid>,
    pending: VecDeque<JobSummary>,
}

impl Slots {
    /// `max` 0 diperlakukan sebagai 1 (runner yang menyala selalu bisa menjalankan satu job).
    #[must_use]
    pub fn new(max: Option<u32>) -> Self {
        Self {
            max: max.map(|m| usize::try_from(m.max(1)).unwrap_or(usize::MAX)),
            running: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    fn has_room(&self) -> bool {
        self.max.is_none_or(|m| self.running.len() < m)
    }

    /// Job siap masuk. Ada slot → dikembalikan untuk dimulai sekarang; penuh → diantrekan.
    pub fn offer(&mut self, job: JobSummary) -> Option<JobSummary> {
        if self.running.contains(&job.id) || self.pending.iter().any(|p| p.id == job.id) {
            return None;
        }
        if self.has_room() && self.pending.is_empty() {
            self.running.push(job.id);
            Some(job)
        } else {
            self.pending.push_back(job);
            None
        }
    }

    /// Job selesai (atau gagal dimulai): bebaskan slot, kembalikan job antrean yang kini boleh
    /// dimulai.
    pub fn finish(&mut self, id: Uuid) -> Vec<JobSummary> {
        self.running.retain(|r| *r != id);
        self.drain()
    }

    /// Job antrean dibatalkan sebelum dimulai.
    pub fn drop_pending(&mut self, id: Uuid) -> bool {
        let before = self.pending.len();
        self.pending.retain(|p| p.id != id);
        before != self.pending.len()
    }

    /// Server menolak `start` karena runner penuh: job kembali ke depan antrean.
    pub fn requeue(&mut self, job: JobSummary) {
        self.running.retain(|r| *r != job.id);
        self.pending.push_front(job);
    }

    fn drain(&mut self) -> Vec<JobSummary> {
        let mut out = Vec::new();
        while self.has_room() {
            let Some(job) = self.pending.pop_front() else {
                break;
            };
            self.running.push(job.id);
            out.push(job);
        }
        out
    }

    #[must_use]
    pub fn running(&self) -> usize {
        self.running.len()
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use agentgate_core::runner::protocol::{JobKind, JobMode, JobStatus};

    use super::*;

    fn job(n: u128) -> JobSummary {
        JobSummary {
            id: Uuid::from_u128(n),
            kind: JobKind::Stage,
            mode: JobMode::Auto,
            status: JobStatus::Claimed,
            ticket: format!("KSR-{n}"),
            title: "x".into(),
            stage: "dev".into(),
            stage_name: "Development".into(),
            persona: "dev".into(),
            priority: 3,
            mine: true,
            can_claim: true,
            created_at: String::new(),
        }
    }

    #[test]
    fn limit_one_runs_jobs_one_at_a_time_in_order() {
        let mut s = Slots::new(Some(1));
        assert!(s.offer(job(1)).is_some());
        assert!(s.offer(job(2)).is_none());
        assert!(s.offer(job(3)).is_none());
        assert_eq!((s.running(), s.pending()), (1, 2));
        let next = s.finish(Uuid::from_u128(1));
        assert_eq!(
            next.iter().map(|j| j.id).collect::<Vec<_>>(),
            [Uuid::from_u128(2)]
        );
        assert_eq!((s.running(), s.pending()), (1, 1));
        assert_eq!(s.finish(Uuid::from_u128(2))[0].id, Uuid::from_u128(3));
        assert!(s.finish(Uuid::from_u128(3)).is_empty());
        assert_eq!((s.running(), s.pending()), (0, 0));
    }

    #[test]
    fn limit_two_never_exceeds_two() {
        let mut s = Slots::new(Some(2));
        let started: usize = (1..=5).filter_map(|n| s.offer(job(n))).count();
        assert_eq!(started, 2);
        assert_eq!(s.pending(), 3);
        let next = s.finish(Uuid::from_u128(1));
        assert_eq!(next.len(), 1);
        assert_eq!(s.running(), 2);
    }

    #[test]
    fn unlimited_starts_everything_and_dedups() {
        let mut s = Slots::new(None);
        assert_eq!((1..=10).filter_map(|n| s.offer(job(n))).count(), 10);
        assert!(
            s.offer(job(3)).is_none(),
            "job yang sama tidak dimulai dua kali"
        );
        assert_eq!(s.running(), 10);
    }

    #[test]
    fn zero_means_one_and_requeue_goes_first() {
        let mut s = Slots::new(Some(0));
        let a = s.offer(job(1)).unwrap();
        assert!(s.offer(job(2)).is_none());
        s.requeue(a);
        assert_eq!(s.running(), 0);
        let next = s.finish(Uuid::from_u128(99));
        assert_eq!(next[0].id, Uuid::from_u128(1));
        assert!(s.drop_pending(Uuid::from_u128(2)));
        assert_eq!(s.pending(), 0);
    }
}
