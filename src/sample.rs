use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;

use rodio::{Decoder, Source, source::{Amplify, Pausable, PeriodicAccess, SamplesConverter, Skippable, Speed, Stoppable, TrackPosition, SeekError}, Sample};

pub type FullSource = Stoppable<Skippable<Amplify<Pausable<TrackPosition<Speed<Decoder<BufReader<File>>>>>>>>;
pub type FullFull<F> = SamplesConverter<PeriodicAccess<FullSource, F>, f32>;

pub struct JolteonSource<F> {
    input: FullFull<F>,
}


pub fn from_file(path: PathBuf, mut periodic_access: impl FnMut(&mut FullSource) + Send) -> JolteonSource<Box<impl FnMut(&mut FullSource) + Send>>
{
    let periodic_access_inner = Box::new(move |src: &mut FullSource| {
        log::debug!("WTF PERIOD ACCESS OMG");
        periodic_access(src);
    });

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
        let i = self.input.inner_mut().inner_mut().inner_mut();
        i.try_seek(pos)
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
