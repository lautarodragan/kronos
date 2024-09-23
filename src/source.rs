use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, Source, source::{Amplify, Pausable, PeriodicAccess, SamplesConverter, Skippable, Speed, Stoppable, TrackPosition, SeekError}, Sample};

pub type FullSource = Stoppable<Skippable<Amplify<Pausable<TrackPosition<Speed<Decoder<BufReader<File>>>>>>>>;
pub type FullFull<F> = SamplesConverter<PeriodicAccess<FullSource, F>, f32>;

pub struct JolteonSourcePeriodic<'a> {
    src: &'a mut FullSource,
}

impl JolteonSourcePeriodic<'_> {
    pub fn stop(&mut self) {
        self.src.stop();
    }

    pub fn skip(&mut self) {
        self.src.inner_mut().skip();
    }

    pub fn pos(&self) -> Duration {
        self.src.inner().inner().inner().inner().get_pos()
    }

    pub fn set_volume(&mut self, factor: f32) {
        self.src.inner_mut().inner_mut().set_factor(factor)
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.src.inner_mut().inner_mut().inner_mut().set_paused(paused)
    }

    pub fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.src.try_seek(position)
    }
}

pub struct JolteonSource<F> {
    input: FullFull<F>,
}

pub fn from_file(path: PathBuf, mut periodic_access: impl FnMut(&mut JolteonSourcePeriodic) + Send) -> JolteonSource<Box<impl FnMut(&mut FullSource) + Send>>
{
    let pos = Arc::new(Mutex::new(Duration::ZERO));

    let periodic_access_inner = {
        let pos = pos.clone();

        Box::new(move |src: &mut FullSource| {
            *pos.lock().unwrap() = src.inner().inner().inner().inner().get_pos();
            let mut something = JolteonSourcePeriodic { src };
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

    JolteonSource {
        input,
    }
}

impl<F: FnMut(&mut FullSource) + Send> JolteonSource<F>
where
    F: FnMut(&mut FullSource) + Send,
{

    // pub fn from_file(path: PathBuf, mut periodic_access: impl FnMut(&mut FullSource) + Send) -> JolteonSource<Box<impl FnMut(&mut FullSource) + Send>>
    // {
    //     let periodic_access_inner = Box::new(move |src: &mut FullSource| {
    //         periodic_access(src);
    //     });
    //
    //     let file = BufReader::new(File::open(path).unwrap());
    //     let source = Decoder::new(file).unwrap();
    //     let input = source
    //         .speed(1.0)
    //         .track_position()
    //         .pausable(false)
    //         .amplify(1.0)
    //         .skippable()
    //         .stoppable()
    //         .periodic_access(Duration::from_millis(5), periodic_access_inner)
    //         .convert_samples();
    //
    //     JolteonSource {
    //         input,
    //     }
    // }

    // /// Returns a reference to the inner source.
    // #[inline]
    // pub fn inner(&self) -> &FullFull<F> {
    //     &self.input
    // }
    //
    /// Returns a mutable reference to the inner source.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut FullFull<F> {
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

    // pub fn set_paused(&mut self, paused: bool) {
    //     let a = self.inner_mut();
    //
    // }
}

impl<F> Iterator for JolteonSource<F>
where
    F: FnMut(&mut FullSource),
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

impl<F> Source for JolteonSource<F>
where
    F: FnMut(&mut FullSource),
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
