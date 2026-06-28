// // #[inline]
// // #[track_caller]
// // pub fn spawn_with_waker<T, F>(f: F)
// //
//

mod state;

// use core::{
//     hint, mem,
//     panic::AssertUnwindSafe,
//     pin::Pin,
//     sync::atomic::{AtomicU8, Ordering},
//     task::{Context, Poll, Waker},
//     time::Duration,
// };
// use std::{io, panic::catch_unwind, sync::Arc, thread};

// use futures::task::AtomicWaker;
// use tokio::sync::oneshot::{self, error::TryRecvError};

// const NOT_STARTED: u8 = 0x00;
// const EXECUTING: u8 = 0x01;
// const FINISHED: u8 = 0x02;
// const CLOSED: u8 = 0x03;
// // const POISONED: u8 = 0x03;

// enum Inner<T> {
//     NotStarted {
//         handle: thread::JoinHandle<T>,
//         waker: Arc<AtomicWaker>,
//         state: Arc<AtomicU8>,
//     },
//     Executing {
//         handle: thread::JoinHandle<T>,
//         waker: Arc<AtomicWaker>,
//         state: Arc<AtomicU8>,
//     },
//     Finished,
// }

// pub struct Thread<T> {
//     inner: Inner<T>,
// }

// impl<T> Thread<T> {
//     pub fn from_builder<F>(
//         builder: thread::Builder,
//         f: F,
//     ) -> io::Result<Thread<T>>
//     where
//         F: 'static + Send + FnOnce() -> T,
//         T: 'static + Send,
//     {
//         let waker = Arc::new(AtomicWaker::new());
//         let state = Arc::new(AtomicU8::new(NOT_STARTED));

//         let handle = builder.spawn({
//             let waker = waker.clone();
//             let state = state.clone();
//             move || {
//                 // Wait for the first poll.
//                 loop {
//                     match state.load(Ordering::Acquire) {
//                         NOT_STARTED => {
//                             hint::spin_loop();
//                             thread::park_timeout(Duration::from_millis(10));
//                         }
//                         EXECUTING => break,
//                         CLOSED => panic!("thread was closed before first poll"),
//                         _ => unreachable!(),
//                     }
//                 }

//                 let value = f();

//                 let waker: Waker = loop {
//                     match waker.take() {
//                         Some(waker) => break waker,
//                         None => match state.load(Ordering::Acquire) {
//                             EXECUTING => {
//                                 hint::spin_loop();
//                                 thread::park_timeout(Duration::from_millis(10));
//                             }
//                             CLOSED => panic!("thread was closed after first poll"),
//                             _ => unreachable!(),
//                         },
//                     }
//                 };

//                 waker.wake();

//                 value
//             }
//         })?;

//         Ok(Thread {
//             inner: Inner::NotStarted {
//                 handle,
//                 state,
//                 waker,
//             },
//         })
//     }
// }

// impl<T> Future for Thread<T> {
//     type Output = io::Result<T>;

//     #[inline(always)]
//     fn poll(
//         self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//     ) -> Poll<Self::Output> {
//         let new_inner = match mem::replace(&mut self.get_mut().inner, Inner::Finished) {
//             Inner::NotStarted {
//                 handle,
//                 waker,
//                 state,
//             } => {
//                 waker.register(cx.waker());
//                 handle.thread().unpark();
//                 state.store(EXECUTING, Ordering::Release);

//                 Inner::Executing {
//                     handle,
//                     waker,
//                     state,
//                 }
//             }
//             Inner::Executing {
//                 handle,
//                 waker,
//                 state,
//             } => {
//                 waker.register(cx.waker());
//             }
//             Inner::Finished => {
//                 return Poll::Ready(Err(io::Error::other("thread finished executing")));
//             }
//         };

//         todo!()
//     }
// }

// impl<T> Drop for Thread<T> {
//     fn drop(&mut self) {
//         match &mut self.inner {
//             Inner::NotStarted {
//                 handle,
//                 waker,
//                 state,
//             } => {
//                 state.store(CLOSED, Ordering::Release);
//             }
//             Inner::Executing {
//                 handle,
//                 waker,
//                 state,
//             } => {
//                 state.store(CLOSED, Ordering::Release);
//             }
//             Inner::Finished => {}
//         }
//     }
// }

// struct SharedState
