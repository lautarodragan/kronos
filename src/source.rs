use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{
    Decoder,
    Source as RodioSource,
    Sample,
    source::{Amplify, Pausable, PeriodicAccess, SamplesConverter, Skippable, Speed, Stoppable, TrackPosition, SeekError},
};

type FullRodioSource = Stoppable<Skippable<Amplify<Pausable<TrackPosition<Speed<Decoder<BufReader<File>>>>>>>>;
type PeriodicRodioSource<F> = SamplesConverter<PeriodicAccess<FullRodioSource, F>, f32>;

pub struct Controls<'a> {
    src: &'a mut FullRodioSource,
}

impl Controls<'_> {

    #[inline]
    pub fn stop(&mut self) {
        self.src.stop();
    }

    #[inline]
    pub fn skip(&mut self) {
        self.src.inner_mut().skip();
    }

    #[inline]
    pub fn pos(&self) -> Duration {
        self.src.inner().inner().inner().inner().get_pos()
    }

    #[inline]
    pub fn set_volume(&mut self, factor: f32) {
        self.src.inner_mut().inner_mut().set_factor(factor)
    }

    #[inline]
    pub fn set_paused(&mut self, paused: bool) {
        self.src.inner_mut().inner_mut().inner_mut().set_paused(paused)
    }

    #[inline]
    pub fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.src.try_seek(position)
    }
}

pub struct Source<F> {
    input: PeriodicRodioSource<F>,
}

impl Source<()> {
    pub fn from_file(path: PathBuf, mut periodic_access: impl FnMut(&mut Controls) + Send) -> Source<Box<impl FnMut(&mut FullRodioSource) + Send>>
    {
        let pos = Arc::new(Mutex::new(Duration::ZERO));

        let periodic_access_inner = {
            let pos = pos.clone();

            Box::new(move |src: &mut FullRodioSource| {
                *pos.lock().unwrap() = src.inner().inner().inner().inner().get_pos();
                let mut something = Controls { src };
                periodic_access(&mut something);
            })
        };

        let file = BufReader::new(File::open(path).unwrap());
        let source = Decoder::new(file).unwrap();
        let input = source
            .speed(1.0)
            .track_position()
            .pausable(false)
            .amplify(1.0)
            .skippable()
            .stoppable()
            .periodic_access(Duration::from_millis(5), periodic_access_inner)
            .convert_samples();

        Source {
            input,
        }
    }
}

impl<F: FnMut(&mut FullRodioSource) + Send> Source<F>
where
    F: FnMut(&mut FullRodioSource) + Send,
{


    // /// Returns a reference to the inner source.
    // #[inline]
    // pub fn inner(&self) -> &FullFull<F> {
    //     &self.input
    // }
    //
    /// Returns a mutable reference to the inner source.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut PeriodicRodioSource<F> {
        &mut self.input
    }
    //
    // /// Returns the inner source.
    // #[inline]
    // pub fn into_inner(self) -> FullFull<F> {
    //     self.input
    // }

    pub fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        log::debug!("TRY_SEEK?");
        let i = self.input.inner_mut().inner_mut().inner_mut();
        i.try_seek(pos)
    }

    pub fn skip(&mut self) -> () {
        log::debug!("skip?");
        let i = self.input.inner_mut().inner_mut().inner_mut();
        i.skip()
    }
}

impl<F> Iterator for Source<F>
where
    F: FnMut(&mut FullRodioSource),
{
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        self.input.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<F> RodioSource for Source<F>
where
    F: FnMut(&mut FullRodioSource),
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    #[inline]
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)
    }
}
