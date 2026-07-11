//! Screen capture utilities for linux.

use ashpd::{
    WindowIdentifier,
    desktop::{
        PersistMode, Session,
        screencast::{
            CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
            StartCastOptions, Stream, Streams,
        },
    },
    enumflags2::BitFlags,
};
use core::slice;
use libspa::param::video::VideoFormat;
use pipewire as pw;
use std::{
    borrow::Borrow,
    fmt,
    iter::FusedIterator,
    num::NonZero,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    pin::Pin,
    sync::atomic::{AtomicU32, Ordering},
};

/// A structure for selecting something to screencast.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct SelectSources<'a> {
    /// A restoration token if we're restoring another session.
    ///
    /// Defaults to [`None`].
    pub restore_token: Option<&'a str>,
    /// Whether we want to persist the current session.
    ///
    /// Defaults to [`false`].
    pub persist_session: bool,
    /// A parent window identifier to inform the compositor where we'd
    /// like to display the prompt.
    ///
    /// Defaults to [`None`].
    pub parent_window: Option<&'a WindowIdentifier>,
    /// Whether we want to show the cursor.
    ///
    /// Defaults to [`true`].
    pub show_cursor: bool,
    /// Whether we want to allow the capture of windows.
    ///
    /// Defaults to [`true`].
    pub allow_windows: bool,
    /// Whether we want to allow the capture of virtual screens.
    ///
    /// Defaults to [`true`].
    pub allow_virtual: bool,
    /// Whether we want to allow getting multiple sources.
    ///
    /// Defaults to [`false`].
    pub allow_multiple: bool,
}

impl<'a> SelectSources<'a> {
    /// Get a bitmask of the available [`CursorMode`]s for this system,
    /// optionally using a cache.
    ///
    /// It will use the specified cache, if possible.
    pub async fn cursor_modes(
        proxy: &Screencast,
        cache: Option<&AtomicU32>,
    ) -> ashpd::Result<BitFlags<CursorMode>> {
        let cached_modes = cache
            .map(|cache| cache.load(Ordering::Acquire))
            .and_then(NonZero::new);

        let available_modes = match cached_modes {
            Some(available_modes) => BitFlags::from_bits_truncate(available_modes.get()),
            None => {
                let available_modes = proxy.available_cursor_modes().await?;

                cache.map(|cache| cache.store(available_modes.bits(), Ordering::Release));

                available_modes
            }
        };

        log::debug!("Available cursor modes: {available_modes:?}");

        Ok(available_modes)
    }

    /// Select the best-fitting available cursor mode.
    pub async fn cursor_mode(
        &self,
        proxy: &Screencast,
        cursor_mode_cache: Option<&AtomicU32>,
    ) -> ashpd::Result<CursorMode> {
        let cursor_modes = SelectSources::cursor_modes(proxy, cursor_mode_cache).await?;

        let cursor_mode = if self.show_cursor && cursor_modes.contains(CursorMode::Embedded) {
            CursorMode::Embedded
        } else if cursor_modes.contains(CursorMode::Metadata) {
            CursorMode::Metadata
        } else {
            CursorMode::Hidden
        };

        log::debug!("Selected cursor mode: {cursor_mode:?}");

        if self.show_cursor && cursor_mode != CursorMode::Embedded {
            log::warn!("Cursor will not be visible despite the request for otherwise.");
        }

        Ok(cursor_mode)
    }

    /// Returns the available source types.
    #[inline(always)]
    #[must_use]
    pub fn source_types(&self) -> BitFlags<SourceType> {
        let mut source_types = BitFlags::from(SourceType::Monitor);

        source_types |= if self.allow_windows {
            SourceType::Window.into()
        } else {
            BitFlags::empty()
        };

        source_types |= if self.allow_virtual {
            SourceType::Virtual.into()
        } else {
            BitFlags::empty()
        };

        source_types
    }

    /// Return the persist mode.
    #[inline(always)]
    #[must_use]
    pub fn persist_mode(&self) -> PersistMode {
        if self.persist_session {
            PersistMode::ExplicitlyRevoked
        } else {
            PersistMode::DoNot
        }
    }

    async fn run_body(&self) -> ashpd::Result<SelectedSources> {
        static CACHE: AtomicU32 = AtomicU32::new(0);

        let proxy = Screencast::new().await?;
        let session = proxy.create_session(Default::default()).await?;

        let cursor_mode = self
            .cursor_mode(&proxy, Some(&CACHE))
            .await
            .inspect_err(|_| log::warn!("an error has occurred while picking a cursor mode"))?;

        proxy
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(cursor_mode)
                    .set_multiple(self.allow_multiple)
                    .set_persist_mode(self.persist_mode())
                    .set_sources(self.source_types())
                    .set_restore_token(self.restore_token),
            )
            .await
            .and_then(|request| request.response())
            .inspect_err(|_| log::warn!("an error has occurred while selecting sources"))?;

        let streams = proxy
            .start(&session, self.parent_window, StartCastOptions::default())
            .await
            .and_then(|request| request.response())
            .inspect_err(|_| log::warn!("an error has occurred while starting a screen cast"))?;

        let pipewire_remote = proxy
            .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
            .await
            .inspect_err(|_| log::warn!("an error has occurred while opening a pipewire remote"))?;

        Ok(SelectedSources {
            proxy,
            session,
            pipewire_remote,
            streams,
        })
    }
}

// type RunRef<'t, 'r, 'w> =
//     Pin<Box<dyn Send + Future<Output = ashpd::Result<SelectSourcesResult>> + 'r + 'w + 't>>;

impl<'a> Default for SelectSources<'a> {
    #[inline]
    fn default() -> Self {
        SelectSources {
            restore_token: None,
            parent_window: None,
            persist_session: false,
            show_cursor: true,
            allow_windows: true,
            allow_virtual: true,
            allow_multiple: false,
        }
    }
}

impl<'a> IntoFuture for SelectSources<'a> {
    type Output = ashpd::Result<SelectedSources>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    #[inline(always)]
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.run_body().await })
    }
}

impl<'a> IntoFuture for &SelectSources<'a> {
    type Output = <SelectSources<'a> as IntoFuture>::Output;
    type IntoFuture = <SelectSources<'a> as IntoFuture>::IntoFuture;

    #[inline(always)]
    fn into_future(self) -> Self::IntoFuture {
        SelectSources::into_future(*self)
    }
}

impl<'a> IntoFuture for &mut SelectSources<'a> {
    type Output = <SelectSources<'a> as IntoFuture>::Output;
    type IntoFuture = <SelectSources<'a> as IntoFuture>::IntoFuture;

    #[inline(always)]
    fn into_future(self) -> Self::IntoFuture {
        SelectSources::into_future(*self)
    }
}

/// The result of [`SelectSources`].
#[derive(Debug)]
#[non_exhaustive]
pub struct SelectedSources {
    /// The proxy that was used.
    pub proxy: Screencast,
    /// The session that was used.
    pub session: Session<Screencast>,
    /// The remote pipewire file descriptor.
    pub pipewire_remote: OwnedFd,
    /// The streams that were selected.
    pub streams: Streams,
}

impl SelectedSources {
    /// Get an iterator of the selected sources.
    #[inline(always)]
    #[must_use]
    pub fn iter(&self) -> SelectedSourcesIter<'_> {
        SelectedSourcesIter {
            selected_sources: self,
            iter: self.streams.streams().iter(),
        }
    }
}

impl<'a> IntoIterator for &'a SelectedSources {
    type Item = SelectedSource<&'a Stream, BorrowedFd<'a>>;
    type IntoIter = SelectedSourcesIter<'a>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator over selected sources.
pub struct SelectedSourcesIter<'a> {
    selected_sources: &'a SelectedSources,
    iter: slice::Iter<'a, Stream>,
}

impl<'a> SelectedSourcesIter<'a> {
    /// Get the inner parts of this iterator.
    #[inline(always)]
    #[must_use]
    pub fn into_inner(self) -> (&'a SelectedSources, slice::Iter<'a, Stream>) {
        (self.selected_sources, self.iter)
    }
}

impl<'a> From<&'a SelectedSources> for SelectedSourcesIter<'a> {
    #[inline(always)]
    fn from(selected_sources: &'a SelectedSources) -> Self {
        selected_sources.iter()
    }
}

impl<'a> Clone for SelectedSourcesIter<'a> {
    #[inline(always)]
    fn clone(&self) -> Self {
        SelectedSourcesIter {
            selected_sources: self.selected_sources,
            iter: self.iter.clone(),
        }
    }
}

impl<'a> Iterator for SelectedSourcesIter<'a> {
    type Item = SelectedSource<&'a Stream, BorrowedFd<'a>>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|stream| SelectedSource {
            pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
            stream,
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline(always)]
    fn nth(
        &mut self,
        n: usize,
    ) -> Option<Self::Item> {
        self.iter.nth(n).map(|stream| SelectedSource {
            pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
            stream,
        })
    }

    #[inline(always)]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    #[inline(always)]
    fn fold<B, F>(
        self,
        init: B,
        mut f: F,
    ) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |b, stream| {
            f(
                b,
                SelectedSource {
                    pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                    stream,
                },
            )
        })
    }

    #[inline(always)]
    fn for_each<F>(
        self,
        mut f: F,
    ) where
        F: FnMut(Self::Item),
    {
        self.iter.for_each(|stream| {
            f(SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
        });
    }

    #[inline(always)]
    fn all<F>(
        &mut self,
        mut f: F,
    ) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        self.iter.all(|stream| {
            f(SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
        })
    }

    #[inline(always)]
    fn any<F>(
        &mut self,
        mut f: F,
    ) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        self.iter.any(|stream| {
            f(SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
        })
    }

    #[inline(always)]
    fn find<P>(
        &mut self,
        mut predicate: P,
    ) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        self.iter
            .find(|&stream| {
                predicate(&SelectedSource {
                    pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                    stream,
                })
            })
            .map(|stream| SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
    }

    #[inline(always)]
    fn find_map<B, F>(
        &mut self,
        mut f: F,
    ) -> Option<B>
    where
        F: FnMut(Self::Item) -> Option<B>,
    {
        self.iter.find_map(|stream| {
            f(SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
        })
    }

    #[inline(always)]
    fn position<P>(
        &mut self,
        mut predicate: P,
    ) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        self.iter.position(|stream| {
            predicate(SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
        })
    }

    #[inline(always)]
    fn rposition<P>(
        &mut self,
        mut predicate: P,
    ) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        self.iter.rposition(|stream: &'a Stream| {
            predicate(SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
        })
    }
}

impl<'a> ExactSizeIterator for SelectedSourcesIter<'a> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> DoubleEndedIterator for SelectedSourcesIter<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|stream| SelectedSource {
            pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
            stream,
        })
    }

    #[inline(always)]
    fn nth_back(
        &mut self,
        n: usize,
    ) -> Option<Self::Item> {
        self.iter.nth_back(n).map(|stream| SelectedSource {
            pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
            stream,
        })
    }

    #[inline(always)]
    fn rfind<P>(
        &mut self,
        mut predicate: P,
    ) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        self.iter
            .rfind(|&stream| {
                predicate(&SelectedSource {
                    pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                    stream,
                })
            })
            .map(|stream| SelectedSource {
                pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                stream,
            })
    }

    #[inline(always)]
    fn rfold<B, F>(
        self,
        init: B,
        mut f: F,
    ) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.rfold(init, |accum, stream| {
            f(
                accum,
                SelectedSource {
                    pipewire_remote: self.selected_sources.pipewire_remote.as_fd(),
                    stream,
                },
            )
        })
    }
}

impl<'a> FusedIterator for SelectedSourcesIter<'a> {}

impl<'a> fmt::Debug for SelectedSourcesIter<'a> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("SelectedSourcesIter")
            .field(
                "rest",
                &fmt::from_fn(|f| f.debug_list().entries(self.clone()).finish_non_exhaustive()),
            )
            .finish_non_exhaustive()
    }
}

/// A selected source.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub struct SelectedSource<S, P>
where
    S: Borrow<Stream>,
    P: AsFd,
{
    /// The remote pipewire file descriptor.
    pub pipewire_remote: P,
    /// The stream we selected.
    pub stream: S,
}

impl<S, P> SelectedSource<S, P>
where
    S: Borrow<Stream>,
    P: AsFd,
{
    /// Reborrow the selected source.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub fn reborrow(&self) -> SelectedSource<&Stream, BorrowedFd<'_>> {
        SelectedSource {
            pipewire_remote: self.pipewire_remote.as_fd(),
            stream: self.stream.borrow(),
        }
    }

    /// Attempt to clone the source.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub fn to_owned(&self) -> std::io::Result<SelectedSource<Stream, OwnedFd>> {
        Ok(SelectedSource {
            pipewire_remote: self.pipewire_remote.as_fd().try_clone_to_owned()?,
            stream: self.stream.borrow().to_owned(),
        })
    }
}

impl<S, P> fmt::Debug for SelectedSource<S, P>
where
    S: Borrow<Stream>,
    P: AsFd,
{
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("SelectedSource")
            .field("pipewire_remote", &self.pipewire_remote.as_fd())
            .field("stream", self.stream.borrow())
            .finish()
    }
}

#[cfg(test)]
#[tokio::test]
async fn fuck() {
    let result = SelectSources {
        ..Default::default()
    }
    .await
    .expect("FUCK");

    for stream in result.iter() {
        println!("{stream:?}");
    }
}

pub fn stream_inner(
    stream_info: Stream,
    pipewire_remote: OwnedFd,
    supported_formats: Vec<VideoFormat>,
) -> Result<(), pw::Error> {
    pw::init();

    Ok(())
}
