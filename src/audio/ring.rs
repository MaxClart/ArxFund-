//! Ring buffer SPSC (Single-Producer / Single-Consumer) lock-free.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RingBuffer {
    buf: UnsafeCell<Box<[f32]>>,
    mask: usize,
    cap: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: RingBuffer est utilisé en SPSC : un seul producteur écrit,
// un seul consommateur lit. Les atomics head/tail établissent les
// barrières de synchronisation. L'accès au buffer sous-jacent est donc
// sound sous ces invariants.
unsafe impl Send for RingBuffer {}
unsafe impl Sync for RingBuffer {}

impl RingBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        let cap = capacity_samples.max(64).next_power_of_two();
        Self {
            buf: UnsafeCell::new(vec![0.0; cap].into_boxed_slice()),
            mask: cap - 1,
            cap,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) & self.mask
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn free(&self) -> usize {
        // Un slot reste volontairement libre : sans cela, un buffer plein
        // (head - tail == cap) serait indistinguable d'un buffer vide.
        self.buf_capacity() - 1 - self.len()
    }

    pub fn write(&self, samples: &[f32]) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let used = head.wrapping_sub(tail) & self.mask;
        let free = self.buf_capacity() - 1 - used;
        let n = samples.len().min(free);
        let buf = self.buf.get();
        unsafe {
            for i in 0..n {
                (*buf)[(head + i) & self.mask] = samples[i];
            }
        }
        self.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    pub fn read(&self, out: &mut [f32]) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let available = head.wrapping_sub(tail) & self.mask;
        let n = out.len().min(available);
        let buf = self.buf.get();
        unsafe {
            for i in 0..n {
                out[i] = (*buf)[(tail + i) & self.mask];
            }
        }
        self.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    pub fn clear_consumer(&self) {
        self.tail
            .store(self.head.load(Ordering::Acquire), Ordering::Release);
    }

    fn buf_capacity(&self) -> usize {
        self.cap
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new(1 << 18)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn write_then_read_roundtrip() {
        let rb = RingBuffer::new(1024);
        let data: Vec<f32> = (0..500).map(|i| i as f32).collect();
        assert_eq!(rb.write(&data), 500);
        let mut out = vec![0.0; 500];
        assert_eq!(rb.read(&mut out), 500);
        assert_eq!(data, out);
    }

    #[test]
    fn write_overflows_partially() {
        let rb = RingBuffer::new(128);
        let data: Vec<f32> = vec![1.0; 200];
        assert_eq!(rb.write(&data), 127);
        let mut out = vec![0.0; 200];
        assert_eq!(rb.read(&mut out), 127);
    }

    #[test]
    fn wraps_around() {
        let rb = RingBuffer::new(128);
        let mut out = vec![0.0; 64];
        for round in 0..5 {
            let data: Vec<f32> = (0..64).map(|i| (i + round * 100) as f32).collect();
            assert_eq!(rb.write(&data), 64);
            assert_eq!(rb.read(&mut out), 64);
            let expected: Vec<f32> = (0..64).map(|i| (i + round * 100) as f32).collect();
            assert_eq!(out, expected);
        }
    }

    #[test]
    fn concurrent_spsc_preserves_order() {
        let rb = Arc::new(RingBuffer::new(1024));
        let producer = Arc::clone(&rb);
        let consumer = Arc::clone(&rb);
        const N: usize = 100_000;
        let writer = thread::spawn(move || {
            let mut sent = 0usize;
            while sent < N {
                let count = (N - sent).min(256);
                let chunk: Vec<f32> = (0..count).map(|i| (sent + i) as f32).collect();
                let n = producer.write(&chunk);
                if n == 0 {
                    std::hint::spin_loop();
                } else {
                    sent += n;
                }
            }
        });
        let reader = thread::spawn(move || {
            let mut received = Vec::with_capacity(N);
            let mut buf = vec![0.0f32; 256];
            while received.len() < N {
                let n = consumer.read(&mut buf);
                if n == 0 {
                    std::hint::spin_loop();
                } else {
                    received.extend_from_slice(&buf[..n]);
                }
            }
            received
        });
        writer.join().unwrap();
        let received = reader.join().unwrap();
        for (i, v) in received.iter().enumerate() {
            assert_eq!(*v, i as f32, "ordre rompu à l'index {i}");
        }
    }

    #[test]
    fn clear_consumer_resets() {
        let rb = RingBuffer::new(256);
        rb.write(&[1.0; 100]);
        rb.clear_consumer();
        assert!(rb.is_empty());
        assert_eq!(rb.free(), 255);
    }
}
